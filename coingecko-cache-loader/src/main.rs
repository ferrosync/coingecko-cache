use std::collections::HashMap;
use std::env;
use sqlx::PgPool;
use bytes::Bytes;
use uuid::Uuid;
use chrono::Utc;
use reqwest::header::HeaderMap;
use dotenv::dotenv;
use serde::{Serialize, Deserialize};
use futures::future::try_join_all;
use tokio::prelude::*;

use crate::model::CoinDominanceResponse;
use std::error::Error;
use sqlx::types::chrono::DateTime;
use reqwest::{IntoUrl, Url, ClientBuilder};
use log::{trace, debug, info, warn, error};
use tokio::time::Duration;
use leaky_bucket::{LeakyBucket, LeakyBuckets};
use rand::prelude::ThreadRng;
use rand::RngCore;

mod model {
    use chrono::{DateTime, Utc};
    use chrono::serde::{ts_seconds};
    use serde::Deserialize;
    use sqlx::types::BigDecimal;

    #[derive(Deserialize, Debug)]
    pub struct CoinDominanceResponse {
        pub data: Vec<CoinDominance>,

        #[serde(with = "ts_seconds")]
        pub timestamp: DateTime<Utc>,
    }

    #[derive(Deserialize, Debug)]
    pub struct CoinDominance {
        pub name: String,
        pub id: String,
        pub market_cap_usd: BigDecimal,
        pub dominance_percentage: BigDecimal,
    }
}

#[derive(Serialize, Deserialize)]
struct DataOriginHeader {
    key: String,
    value: String
}

impl DataOriginHeader {
    fn new(key: String, value: String) -> DataOriginHeader {
        DataOriginHeader { key, value }
    }
}

#[derive(Serialize, Deserialize)]
struct DataOriginMetadata {
    headers: Vec<DataOriginHeader>,
}

struct InsertDataOriginResult {

}

async fn insert_data_origin(
    timestamp: DateTime<Utc>,
    buffer: &Bytes,
    meta: &Option<serde_json::Value>,
    pool: &PgPool,
) -> Result<Uuid, Box<dyn Error>> {

    let mut tx = pool.begin().await?;
    let uuid = Uuid::new_v4();

    let cur = sqlx::query!(r#"
        insert into data_origin (
            uuid,
            agent,
            timestamp_utc,
            data,
            metadata
        )
        values ($1, $2, $3, $4, $5)
    "#,
        uuid,
        "loader_rust",
        timestamp.naive_utc(),
        buffer.as_ref(),
        *meta)
        .execute(&mut tx)
        .await?;

    tx.commit().await?;

    Ok((uuid))
}

async fn insert_snapshot(
    timestamp: DateTime<Utc>,
    buffer: &Bytes,
    headers: &HeaderMap,
    json: &CoinDominanceResponse,
    pool: &PgPool) -> Result<Uuid, Box<dyn Error>> {

    let uuid = insert_data_origin_from_http(timestamp, buffer, headers, pool).await?;

    let mut tx = pool.begin().await?;

    for coin in json.data.iter() {
        sqlx::query!(r#"
        insert into coin_dominance (
            data_origin_uuid,
            agent,
            timestamp_utc,
            coin_id,
            coin_name,
            market_cap_usd,
            market_dominance_percentage
        )
        values ($1, $2, $3, $4, $5, $6, $7)
        "#,
        uuid,
        "loader_rust",
        json.timestamp.naive_utc(),
        coin.id,
        coin.name,
        coin.market_cap_usd,
        coin.dominance_percentage)
        .execute(&mut tx).await?;
    }

    tx.commit().await?;

    Ok(uuid)
}

async fn insert_data_origin_from_http(
    timestamp: DateTime<Utc>,
    buffer: &Bytes,
    headers: &HeaderMap,
    pool: &PgPool
) -> Result<Uuid, Box<dyn Error>> {

    let header_json: Vec<DataOriginHeader> = headers.iter()
        .flat_map(|(k, v)| {
            let to_str = v.to_str();
            match to_str {
                Ok(v) => Some(DataOriginHeader::new(k.to_string(), v.to_string())),
                _ => None
            }
        })
        .collect();

    let meta = serde_json::to_value(DataOriginMetadata { headers: header_json });
    if let Err(ref err) = meta {
        warn!("Failed to serialize snapshot metadata: {}", err);
    }

    let meta = meta.ok();
    let uuid = insert_data_origin(timestamp, buffer, &meta, pool).await?;

    Ok(uuid)
}

struct UrlCacheBuster<'a> {
    rng: ThreadRng,
    url: &'a Url,
}

impl<'a> UrlCacheBuster<'a> {
    fn new(url: &'a Url) -> UrlCacheBuster<'a> {
        UrlCacheBuster { rng: rand::thread_rng(), url }
    }

    fn next(&mut self) -> Url {
        let num = self.rng.next_u64();

        self.url.clone()
            .query_pairs_mut()
            .append_pair("_", num.to_string().as_str())
            .finish()
            .to_owned()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL missing or unset '.env' file");
    let db_pool = PgPool::new(&database_url).await?;

    let url_raw = env::var("FETCH_URL").expect("FETCH_URL missing or unset in '.env' file");
    let url = Url::parse(url_raw.as_str()).expect("FETCH_URL has invalid URL format");
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
    -> Result<(DateTime<Utc>, Uuid), Box<dyn Error>>
{
    let response = http.get(url.clone()).send().await?;
    let now = Utc::now();

    let headers = response.headers().clone();
    let buffer = response.bytes().await?;
    let json = serde_json::from_slice::<model::CoinDominanceResponse>(&buffer)?;

    let uuid = insert_snapshot(
        now,
        &buffer,
        &headers,
        &json,
        &db_pool
    ).await?;

    Ok((now, uuid))
}
