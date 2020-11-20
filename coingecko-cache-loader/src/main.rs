
mod coingecko;
mod util;
mod db;

use std::env;
use std::error::Error;
use std::io::Write;

use chrono::{Utc, Local};
use dotenv::dotenv;
use log::{trace, debug, info, warn, error};
use leaky_bucket::LeakyBuckets;
use reqwest::header::{CONTENT_TYPE};
use reqwest::Url;
use sqlx::PgPool;
use sqlx::types::chrono::DateTime;
use tokio::time::Duration;

use crate::util::{UrlCacheBuster, AtomicCancellation};
use crate::db::models::ProvenanceId;
use crate::db::convert::ToMetadata;
use std::str::FromStr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let log_env = env::var("RUST_LOG").unwrap_or("info".into());
    pretty_env_logger::formatted_timed_builder()
        .parse_filters(&log_env)
        .init();

    let database_url = env::var("DOMFI_LOADER_DATABASE_URL").expect("DOMFI_LOADER_DATABASE_URL missing or unset '.env' file");
    let db_pool = PgPool::connect(&database_url).await?;

    let url_raw = env::var("DOMFI_LOADER_URL").expect("DOMFI_LOADER_URL missing or unset in '.env' file");
    let url = Url::parse(url_raw.as_str()).expect("DOMFI_LOADER_URL has invalid URL format");
    let mut url_gen = UrlCacheBuster::new(&url);

    let rate_limit_interval_default = Duration::from_millis(1250);
    let rate_limit_interval = env::var("DOMFI_LOADER_INTERVAL").ok()
        .and_then(|s| match parse_duration::parse(&s) {
            Err(err) => {
                warn!("Failed to parse 'DOMFI_LOADER_INTERVAL'. Using default value instead of '{:#?}'. Cause: {}",
                      rate_limit_interval_default, err);
                None
            }
            Ok(dur) => Some(dur)
        })
        .unwrap_or(rate_limit_interval_default);

    //

    let http = reqwest::ClientBuilder::new().build().expect("Failed to build HTTP client");

    let mut buckets = LeakyBuckets::new();

    // spawn the coordinate thread to refill the rate limiter.
    let coordinator = buckets.coordinate()?;
    tokio::spawn(async move { coordinator.await.expect("Failed to start rate limiter coordinator") });

    let rate_limiter = buckets.rate_limiter()
        .max(1)
        .tokens(1)
        .refill_amount(1)
        .refill_interval(rate_limit_interval)
        .build()
        .expect("Failed to build request rate limiter");

    let stop_signal_source = AtomicCancellation::new();

    let h = stop_signal_source.clone();
    ctrlc::set_handler(move || {
        let handle = &h;
        handle.cancel();
        info!("Shutting down from Ctrl-C signal...");
    })
    .expect("Error setting Ctrl-C handler");

    let stop_handle = stop_signal_source.clone();
    while stop_handle.can_continue() {
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

    info!("Quitting");
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
    let json = serde_json::from_slice::<coingecko::CoinDominanceResponse>(&buffer);

    let pid = db::ops::insert_snapshot(
        now,
        &buffer,
        mime,
        &request_meta,
        &response_meta,
        json,
        &db_pool
    ).await?;

    Ok((now, pid))
}
