use crate::repo;
use crate::api::models::{CoinDominanceElement, ProvenanceResponse, CoinDominanceMeta};
use bigdecimal::BigDecimal;

fn round_price_identifier(value: BigDecimal) -> BigDecimal {
    value.round(2).with_scale(2)
}

impl CoinDominanceElement {
    pub fn from_repo(data: repo::CoinDominanceRecord) -> CoinDominanceElement {
        Self {
            name: data.name,
            id: data.id,
            market_cap_usd: data.market_cap_usd,
            dominance_percentage: data.dominance_percentage.clone(),
            price_identifier: round_price_identifier(data.dominance_percentage.clone()),
        }
    }
}

impl ProvenanceResponse {
    pub fn from_repo(x: repo::Provenance) -> ProvenanceResponse {
        Self {
            uuid: x.uuid,
            agent: x.agent,
            imported_at: x.timestamp_utc,
            data: x.data,
            sha256: x.object_sha256,
            request_metadata: x.request_metadata,
            response_metadata: x.response_metadata,
        }
    }
}

impl CoinDominanceMeta {
    pub fn from_repo(data: repo::OriginMetadata) -> CoinDominanceMeta {
        Self {
            provenance_uuid: data.provenance_uuid,
            blob_sha256: data.blob_sha256,
            imported_at_timestamp: data.imported_at_utc,
            requested_timestamp: data.requested_timestamp_utc,
            actual_timestamp: data.actual_timestamp_utc,
        }
    }
}
