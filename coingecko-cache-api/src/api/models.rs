use std::fmt::{Display, Formatter};
use std::fmt;

use chrono::{DateTime, Utc};
use bigdecimal::BigDecimal;
use uuid::Uuid;

use serde::{Serialize, Deserialize};
use serde_with::{serde_as};
use serde_with::DisplayFromStr;
use chrono::serde::{ts_milliseconds, ts_seconds};
use crate::ext::serde_with::ToStringVerbatim;

#[serde_as]
#[serde_as(as = "DisplayFromStr")]
#[derive(Serialize)]
pub enum ResponseStatus {
    Success,
    Error,
}

impl Display for ResponseStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self {
            ResponseStatus::Success => f.write_str("success"),
            ResponseStatus::Error => f.write_str("error"),
        }
    }
}

#[derive(Serialize)]
pub struct ErrorResponse {
    status: ResponseStatus,
    reason: String,
}

impl ErrorResponse {
    pub fn new(reason: String) -> ErrorResponse {
        ErrorResponse {
            status: ResponseStatus::Error,
            reason,
        }
    }
}

//

#[serde_as]
#[derive(Serialize)]
pub struct PingResponse {
    #[serde_as(as = "DisplayFromStr")]
    pub status: ResponseStatus,

    #[serde(with = "ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
}

//

#[derive(Deserialize)]
pub struct CoinDominanceQuery {
    pub timestamp: Option<u64>,
}

#[serde_as]
#[derive(Serialize)]
pub struct CoinDominanceElement {
    pub name: String,
    pub id: String,

    #[serde_as(as = "ToStringVerbatim")]
    pub market_cap_usd: BigDecimal,

    #[serde_as(as = "ToStringVerbatim")]
    pub dominance_percentage: BigDecimal,

    #[serde_as(as = "ToStringVerbatim")]
    pub price_identifier: BigDecimal,
}

//

#[serde_as]
#[derive(Serialize)]
pub struct CoinDominanceMeta {
    pub provenance_uuid: Uuid,

    #[serde_as(as = "serde_with::hex::Hex")]
    pub blob_sha256: Vec<u8>,

    #[serde(with = "ts_milliseconds")]
    pub imported_at_timestamp: DateTime<Utc>,

    #[serde(with = "ts_milliseconds")]
    pub requested_timestamp: DateTime<Utc>,

    #[serde(with = "ts_milliseconds")]
    pub actual_timestamp: DateTime<Utc>,
}

//

#[serde_as]
#[derive(Serialize)]
pub struct CoinDominanceResponse {
    #[serde_as(as = "DisplayFromStr")]
    pub status: ResponseStatus,

    pub data: Vec<CoinDominanceElement>,

    #[serde(with = "ts_seconds")]
    pub timestamp: DateTime<Utc>,

    pub meta: CoinDominanceMeta,
}

#[serde_as]
#[derive(Serialize)]
pub struct ProvenanceResponse {
    pub uuid: Uuid,
    pub agent: String,
    pub imported_at: DateTime<Utc>,

    #[serde_as(as = "crate::base64::Base64")]
    pub data: Vec<u8>,

    #[serde_as(as = "serde_with::hex::Hex")]
    pub sha256: Vec<u8>,

    pub request_metadata: Option<serde_json::Value>,
    pub response_metadata: Option<serde_json::Value>,
}

