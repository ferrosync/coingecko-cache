use std::collections::HashMap;
use std::env;
use sqlx::PgPool;
use bytes::Bytes;
use uuid::Uuid;
use chrono::Utc;
use reqwest::header::HeaderMap;
use dotenv::dotenv;
use serde::{Serialize, Deserialize};

use crate::model::CoinDominanceResponse;
use std::error::Error;

mod model {
    use chrono::{DateTime, Utc};
    use chrono::serde::{ts_seconds};
    use serde::Deserialize;
    use bigdecimal::BigDecimal;

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

async fn insert_snapshot(
    buffer: &Bytes,
    headers: &HeaderMap,
    json: &CoinDominanceResponse,
    pool: &PgPool) -> Result<(), Box<dyn Error>> {

    let now = Utc::now();

    let header_json: Vec<DataOriginHeader> = headers.iter()
        .flat_map(|(k, v)| {
            let to_str = v.to_str();
            match to_str {
                Ok(v) => Some(DataOriginHeader::new(k.to_string(), v.to_string())),
                _ => None
            }
        })
        .collect();

    let meta = serde_json::to_value(DataOriginMetadata { headers: header_json })?;

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
        now.naive_utc(),
        buffer.as_ref(),
        meta)
        .execute(&mut tx)
        .await?;

    tx.commit().await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL is not set in '.env' file");
    let db_pool = PgPool::new(&database_url).await?;

    let url = "https://api.coingecko.com/api/v3/global/coin_dominance";

    let response = reqwest::get(url).await?;
    let headers = response.headers().clone();
    let buffer = response.bytes().await?;
    let json = serde_json::from_slice::<model::CoinDominanceResponse>(&buffer)?;

    insert_snapshot(&buffer, &headers, &json, &db_pool).await?;

    println!("{:#?}", json);
    Ok(())
}
