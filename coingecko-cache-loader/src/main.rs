
mod coingecko;
mod util;
mod db;

use std::env;
use std::error::Error;

use chrono::Utc;
use dotenv::dotenv;
use log::{trace, debug, info, warn, error};
use leaky_bucket::LeakyBuckets;
use reqwest::header::{CONTENT_TYPE};
use reqwest::Url;
use sqlx::PgPool;
use sqlx::types::chrono::DateTime;
use tokio::time::Duration;

use crate::util::UrlCacheBuster;
use crate::db::models::ProvenanceId;
use crate::db::convert::ToMetadata;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();
    dotenv().ok();

    let database_url = env::var("DOMFI_LOADER_DATABASE_URL").expect("DOMFI_LOADER_DATABASE_URL missing or unset '.env' file");
    let db_pool = PgPool::connect(&database_url).await?;

    let url_raw = env::var("DOMFI_LOADER_URL").expect("DOMFI_LOADER_URL missing or unset in '.env' file");
    let url = Url::parse(url_raw.as_str()).expect("DOMFI_LOADER_URL has invalid URL format");
    let mut url_gen = UrlCacheBuster::new(&url);

    let http = reqwest::ClientBuilder::new().build().expect("Failed to build HTTP client");

    let mut buckets = LeakyBuckets::new();

    // spawn the coordinate thread to refill the rate limiter.
    let coordinator = buckets.coordinate()?;
    tokio::spawn(async move { coordinator.await.expect("Failed to start rate limiter coordinator") });

    let rate_limiter = buckets.rate_limiter()
        .max(1)
        .tokens(1)
        .refill_amount(1)
        .refill_interval(Duration::from_secs(1))
        .build()
        .expect("Failed to build request rate limiter");

    loop {
        let url = url_gen.next();
        let result = fetch_to_insert_snapshot(&url, &http, &db_pool).await;
        if let Err(err) = result {
            error!("Failed to insert snapshot! Cause: {}", err);
        }
        else if let Ok((ts, uuid)) = result {
            info!("[{}] Committed snapshot at {}", uuid, ts);
        }

        rate_limiter.acquire_one().await?;
    }

    Ok(())
}


async fn fetch_to_insert_snapshot(url: &Url, http: &reqwest::Client, db_pool: &PgPool)
    -> Result<(DateTime<Utc>, ProvenanceId), Box<dyn Error>>
{
    let request_builder = http.get(url.clone());

    let request = request_builder.build()?;
    let request_meta = request.to_metadata();

    let response = http.execute(request).await?;
    let now = Utc::now();

    let mime = response.headers()
        .get(CONTENT_TYPE)
        .and_then(|x| x.to_str().map(|s| s.to_string()).ok());

    let response_meta = response.to_metadata();
    let buffer = response.bytes().await?;
    let json = serde_json::from_slice::<coingecko::CoinDominanceResponse>(&buffer)?;

    let pid = db::ops::insert_snapshot(
        now,
        &buffer,
        mime,
        &request_meta,
        &response_meta,
        &json,
        &db_pool
    ).await?;

    Ok((now, pid))
}
