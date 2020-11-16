use std::collections::HashMap;
use std::{env, fmt};
use sqlx::PgPool;
use bytes::Bytes;
use uuid::Uuid;
use chrono::Utc;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use dotenv::dotenv;
use serde::{Serialize, Deserialize};
use futures::future::try_join_all;
use tokio::prelude::*;

use crate::model::CoinDominanceResponse;
use std::error::Error;
use sqlx::types::chrono::DateTime;
use reqwest::{IntoUrl, Url, ClientBuilder, Method, StatusCode};
use log::{trace, debug, info, warn, error};
use tokio::time::Duration;
use leaky_bucket::{LeakyBucket, LeakyBuckets};
use rand::prelude::ThreadRng;
use rand::RngCore;
use sha2::{Sha256, Digest};
use serde_json::Value;
use futures::StreamExt;
use std::fmt::Display;
use serde::export::Formatter;

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
struct HeaderMapEntry {
    key: String,
    value: String
}

impl HeaderMapEntry {
    fn new(key: String, value: String) -> HeaderMapEntry {
        HeaderMapEntry { key, value }
    }
}

trait ToMetadata {
    type Output: Serialize;
    fn to_metadata(&self) -> Self::Output;
}

impl ToMetadata for HeaderMap {
    type Output = Vec<HeaderMapEntry>;
    fn to_metadata(&self) -> Self::Output {
        self.iter()
            .map(|(k, v)| {
                let k = k.to_string();
                match v.to_str() {
                    Ok(v) => HeaderMapEntry::new(k, v.to_string()),
                    _ => HeaderMapEntry::new(k + ":b64", base64::encode(v.as_bytes())),
                }
            })
            .collect()
    }
}

impl ToMetadata for reqwest::Request {
    type Output = RequestMetadata;
    fn to_metadata(&self) -> Self::Output {
        RequestMetadata {
            headers: self.headers().clone().to_metadata(),
            method: self.method().to_string(),
            url: self.url().to_string(),
        }
    }
}

impl ToMetadata for reqwest::Response {
    type Output = ResponseMetadata;
    fn to_metadata(&self) -> Self::Output {
        ResponseMetadata {
            headers: self.headers().clone().to_metadata(),
            url: self.url().to_string(),
            status: self.status().as_u16()
        }
    }
}

struct InsertDataOriginResult {

}

#[derive(Eq, PartialEq, Debug)]
struct ProvenanceId {
    uuid: Uuid,
    object_id: i64,
}

impl Display for ProvenanceId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "[uuid = {}, obj #{}]", self.uuid, self.object_id)
    }
}

async fn insert_provenance(
    timestamp: DateTime<Utc>,
    buffer: &Bytes,
    mime: Option<String>,
    request_meta: &Option<serde_json::Value>,
    response_meta: &Option<serde_json::Value>,
    pool: &PgPool,
) -> Result<ProvenanceId, Box<dyn Error>> {

    let mut tx = pool.begin().await?;

    let mut hasher = Sha256::new();
    hasher.update(buffer.as_ref());
    let hash = hasher.finalize();

    let storage = sqlx::query!(r#"
        with new_obj as (
            insert into object_storage (sha256, data, mime)
            values ($1, $2, $3)
            on conflict (sha256) do update
                set mime = $3
            returning id
        )
        select id from new_obj
        union
        select id from object_storage where sha256 = $1
        "#,
        hash.as_slice(),
        buffer.as_ref(),
        mime)
        .fetch_one(&mut tx)
        .await?;

    let object_id_opt: Option<i64> = storage.id;
    let object_id = object_id_opt.ok_or(sqlx::Error::RowNotFound)?;
    let uuid = Uuid::new_v4();

    let cur = sqlx::query!(r#"
        insert into provenance (
            uuid,
            object_id,
            agent,
            timestamp_utc,
            request_metadata,
            response_metadata
        )
        values ($1, $2, $3, $4, $5, $6)
    "#,
        uuid,
        storage.id,
        "loader_rust",
        timestamp.naive_utc(),
        *request_meta,
        *response_meta)
        .execute(&mut tx)
        .await?;

    tx.commit().await?;
    Ok(ProvenanceId { uuid, object_id })
}

async fn insert_snapshot(
    timestamp: DateTime<Utc>,
    buffer: &Bytes,
    mime: Option<String>,
    request_meta: &RequestMetadata,
    response_meta: &ResponseMetadata,
    json: &CoinDominanceResponse,
    pool: &PgPool) -> Result<ProvenanceId, Box<dyn Error>> {

    let pid = insert_data_origin_from_http(
        timestamp,
        buffer,
        mime,
        request_meta,
        response_meta,
        pool,
    ).await?;

    let mut tx = pool.begin().await?;

    for coin in json.data.iter() {
        sqlx::query!(r#"
        insert into coin_dominance (
            provenance_uuid,
            object_id,
            agent,
            timestamp_utc,
            coin_id,
            coin_name,
            market_cap_usd,
            market_dominance_percentage
        )
        values ($1, $2, $3, $4, $5, $6, $7, $8)
        on conflict do nothing
        "#,
        pid.uuid,
        pid.object_id,
        "loader_rust",
        json.timestamp.naive_utc(),
        coin.id,
        coin.name,
        coin.market_cap_usd,
        coin.dominance_percentage)
        .execute(&mut tx)
        .await?;
    }

    tx.commit().await?;
    Ok(pid)
}

async fn insert_data_origin_from_http(
    timestamp: DateTime<Utc>,
    buffer: &Bytes,
    mime: Option<String>,
    request_meta: &RequestMetadata,
    response_meta: &ResponseMetadata,
    pool: &PgPool
) -> Result<ProvenanceId, Box<dyn Error>> {

    fn convert<T: Serialize>(value: T) -> Option<Value> {
        let json = serde_json::to_value(value);
        if let Err(ref err) = json {
            warn!("Failed to serialize snapshot metadata: {}", err);
        }
        json.ok()
    }

    let request_meta = convert(request_meta);
    let response_meta = convert(response_meta);

    let pid = insert_provenance(
        timestamp,
        buffer,
        mime,
        &request_meta,
        &response_meta,
        pool
    ).await?;

    Ok(pid)
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

type HeaderMapSerializable = Vec<HeaderMapEntry>;

#[derive(Serialize)]
struct RequestMetadata {
    method: String,
    url: String,
    headers: HeaderMapSerializable,
}

#[derive(Serialize)]
struct ResponseMetadata {
    url: String,
    status: u16,
    headers: HeaderMapSerializable,
}

async fn fetch_to_insert_snapshot(url: &Url, http: &reqwest::Client, db_pool: &PgPool)
    -> Result<(DateTime<Utc>, ProvenanceId), Box<dyn Error>>
{
    let mut request_builder = http.get(url.clone());

    let request = request_builder.build()?;
    let request_meta = request.to_metadata();

    let response = http.execute(request).await?;
    let now = Utc::now();

    let mime = response.headers()
        .get(CONTENT_TYPE)
        .and_then(|x| x.to_str().map(|s| s.to_string()).ok());

    let response_meta = response.to_metadata();
    let buffer = response.bytes().await?;
    let json = serde_json::from_slice::<model::CoinDominanceResponse>(&buffer)?;

    let pid = insert_snapshot(
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
