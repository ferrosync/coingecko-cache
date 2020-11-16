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
