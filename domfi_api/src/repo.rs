use std::ops::Add;
use futures::prelude::*;
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::BigDecimal;
use chrono::{DateTime, Utc, SubsecRound, Timelike, Duration, TimeZone};
use snafu::{Snafu, ResultExt};
use domfi_domain::models::FinancialDominanceAsset;

pub struct OriginMetadata {
    pub requested_timestamp_utc: DateTime<Utc>,
    pub actual_timestamp_utc: DateTime<Utc>,
    pub imported_at_utc: DateTime<Utc>,
    pub provenance_uuid: Uuid,
    pub agent: String,
    pub blob_sha256: Vec<u8>,
}

pub struct OriginMetadataSlim {
    pub requested_timestamp_utc: DateTime<Utc>,
    pub actual_timestamp_utc: DateTime<Utc>,
    pub provenance_uuid: Uuid,
}

pub struct CoinDominanceRecord {
    pub name: String,
    pub id: String,
    pub market_cap_usd: BigDecimal,
    pub dominance_percentage: BigDecimal,
}

pub struct PricingResult {
    pub meta: OriginMetadataSlim,
    pub coin_id: String,
    pub coin_symbol: String,
    pub percentage: BigDecimal,
}

pub struct FindByTimestampResult {
    pub meta: OriginMetadata,
    pub elements: Vec<CoinDominanceRecord>,
}

#[derive(Debug, Snafu)]
pub enum RepositoryError {
    #[snafu(display("Failed to access database: {}", source))]
    SqlError { source: sqlx::Error, },
}

pub struct DataOriginRepo { }

pub struct Provenance {
    pub uuid: Uuid,
    pub agent: String,
    pub timestamp_utc: DateTime<Utc>,
    pub data: Vec<u8>,
    pub object_id: i64,
    pub object_sha256: Vec<u8>,
    pub request_metadata: Option<serde_json::Value>,
    pub response_metadata: Option<serde_json::Value>,
}

pub struct ObjectStorageRepo { }

pub struct StorageBlob {
    pub id: i64,
    pub sha256: Vec<u8>,
    pub data: Vec<u8>,
    pub mime: Option<String>,
}

pub struct FindByIdHistoryRow {
    pub timestamp_utc_minutely: DateTime<Utc>,
    pub timestamp_utc_exact: DateTime<Utc>,
    pub provenance_uuid: Uuid,
    pub dominance_percentage: BigDecimal,
}

pub struct FindByIdHistoryDataset {
    pub asset: FinancialDominanceAsset,
    pub rows: Vec<FindByIdHistoryRow>,
}

impl ObjectStorageRepo {
    pub async fn get_by_sha256(hash: &[u8], pool: &PgPool) -> Result<StorageBlob, RepositoryError> {

        let row = sqlx::query!(r#"
            select
                obj.id,
                obj.sha256,
                obj.data,
                obj.mime
            from
                object_storage obj
            where
                obj.sha256 = $1
        "#, hash)
            .fetch_one(pool)
            .await
            .context(SqlError);

        row.map(|x| StorageBlob {
            id: x.id,
            data: x.data,
            sha256: x.sha256,
            mime: x.mime,
        })
    }
}

impl DataOriginRepo {

    pub async fn get_by_uuid(uuid: Uuid, pool: &PgPool) -> Result<Provenance, RepositoryError> {

        let row = sqlx::query!(r#"
            select
                data.uuid,
                data.agent,
                data.timestamp_utc,
                data.object_id,
                data.request_metadata,
                data.response_metadata,
                obj.data,
                obj.sha256
            from
                provenance data
                inner join object_storage obj
                    on obj.id = data.object_id
            where
                data.uuid = $1
        "#, uuid)
            .fetch_one(pool)
            .await
            .context(SqlError);

        row.map(|x| Provenance {
            uuid: x.uuid,
            agent: x.agent,
            timestamp_utc: Utc.from_utc_datetime(&x.timestamp_utc),
            object_id: x.object_id,
            object_sha256: x.sha256,
            data: x.data,
            response_metadata: x.response_metadata,
            request_metadata: x.request_metadata,
        })
    }
}

pub struct CoinDominanceRepo { }

pub struct TimestampAgent {
    timestamp: DateTime<Utc>,
    agent: String,
}

impl CoinDominanceRepo {

    pub fn round_timestamp(ts: DateTime<Utc>) -> DateTime<Utc> {
        ts.trunc_subsecs(0).with_second(0).unwrap()
    }

    pub async fn latest_timestamp_agent(pool: &PgPool) -> Result<TimestampAgent, RepositoryError> {

        let row = sqlx::query!(r#"
            select
                timestamp_utc, agent
            from
                coin_dominance
            where
                timestamp_utc = (
                    select max(timestamp_utc) from coin_dominance
                )
            limit 1
        "#)
            .fetch_one(pool)
            .await
            .context(SqlError)?;

        Ok(TimestampAgent {
            timestamp: Utc.from_utc_datetime(&row.timestamp_utc),
            agent: row.agent,
        })
    }

    pub async fn timestamp_from_range(ts: DateTime<Utc>, pool: &PgPool) -> Result<TimestampAgent, RepositoryError> {

        let actual_ts = Self::round_timestamp(ts);
        let actual_ts_plus_1 = actual_ts.add(Duration::minutes(1));

        let row = sqlx::query!(r#"
            select
                timestamp_utc, agent
            from
                coin_dominance
            where
                timestamp_utc between $1 and $2
            order by timestamp_utc asc
            limit 1
        "#,
            actual_ts.naive_utc(),
            actual_ts_plus_1.naive_utc(),
            )
            .fetch_one(pool)
            .await
            .context(SqlError)?;

        Ok(TimestampAgent {
            timestamp: Utc.from_utc_datetime(&row.timestamp_utc),
            agent: row.agent,
        })
    }

    pub async fn find_by_timestamp_rounded(
        ts: Option<DateTime<Utc>>,
        pool: &PgPool
    ) -> Result<FindByTimestampResult, RepositoryError> {

        let timestamp_agent = match ts {
            Some(ts) => Self::timestamp_from_range(ts, pool).await?,
            None => Self::latest_timestamp_agent(pool).await?,
        };

        let mut cursor =
            sqlx::query!(r#"
                select
                    data.provenance_uuid,
                    obj.id,
                    obj.sha256,
                    data.timestamp_utc,
                    data.imported_at_utc,
                    data.agent,
                    data.coin_id,
                    data.coin_name,
                    data.market_cap_usd,
                    data.market_dominance_percentage
                from
                    coin_dominance as data
                    inner join object_storage obj
                        on obj.id = data.object_id
                where
                    data.timestamp_utc = $1
                    and data.agent = $2
                order by
                    -- note: force pushing the "others" to the bottom of the list
                    case when ((data.coin_id <> '') is not true) then 1 else 0 end,

                    -- then sort by market cap descending
                    data.market_cap_usd desc
                "#,
                timestamp_agent.timestamp.naive_utc(),
                timestamp_agent.agent)
                .fetch(pool);

        let mut elements = Vec::new();
        let meta = if let Some(x) = cursor.try_next().await.context(SqlError)? {
            elements.push(CoinDominanceRecord {
                name: x.coin_name,
                id: x.coin_id,
                market_cap_usd: x.market_cap_usd,
                dominance_percentage: x.market_dominance_percentage
            });

            let actual_timestamp = Utc.from_utc_datetime(&x.timestamp_utc);
            let requested_timestamp = ts.unwrap_or(actual_timestamp);
            let m = OriginMetadata {
                requested_timestamp_utc: requested_timestamp,
                provenance_uuid: x.provenance_uuid,
                actual_timestamp_utc: actual_timestamp,
                imported_at_utc: Utc.from_utc_datetime(&x.imported_at_utc),
                agent: x.agent,
                blob_sha256: x.sha256,
            };

            m
        } else {
            return Err(sqlx::Error::RowNotFound).context(SqlError);
        };

        while let Some(x) = cursor.try_next().await.context(SqlError)? {
            elements.push(CoinDominanceRecord {
                name: x.coin_name,
                id: x.coin_id,
                market_cap_usd: x.market_cap_usd,
                dominance_percentage: x.market_dominance_percentage
            })
        };

        Ok(FindByTimestampResult {
            meta,
            elements
        })
    }

    pub async fn find_by_id_at_timestamp_rounded(
        asset: &FinancialDominanceAsset,
        ts: Option<DateTime<Utc>>,
        pool: &PgPool
    ) -> Result<PricingResult, RepositoryError> {

        let timestamp_agent = match ts {
            Some(ts) => Self::timestamp_from_range(ts, pool).await?,
            None => Self::latest_timestamp_agent(pool).await?,
        };

        let x =
            sqlx::query!(r#"
                select
                    data.provenance_uuid,
                    data.timestamp_utc,
                    data.coin_id,
                    data.coin_name,
                    data.market_dominance_percentage
                from
                    coin_dominance as data
                where
                    data.timestamp_utc = $1
                    and data.agent = $2
                    and data.coin_id = $3
                limit 1
                "#,
                timestamp_agent.timestamp.naive_utc(),
                timestamp_agent.agent,
                asset.underlying().symbol().id())
                .fetch_one(pool)
                .await
                .context(SqlError)?;

        let actual_timestamp = Utc.from_utc_datetime(&x.timestamp_utc);
        let requested_timestamp = ts.unwrap_or(actual_timestamp);

        Ok(PricingResult {
            coin_id: x.coin_id,
            coin_symbol: x.coin_name,
            percentage: x.market_dominance_percentage,
            meta: OriginMetadataSlim {
                requested_timestamp_utc: requested_timestamp,
                actual_timestamp_utc: actual_timestamp,
                provenance_uuid: x.provenance_uuid,
            },
        })
    }

    pub async fn find_by_id_history(
        asset: &FinancialDominanceAsset,
        pool: &PgPool
    ) -> Result<FindByIdHistoryDataset, RepositoryError> {

        let rows =
            sqlx::query!(r#"
                select distinct on (date_trunc('minute', timestamp_utc))
                    date_trunc('minute', timestamp_utc) as timestamp_utc_minutely,
                    timestamp_utc,
                    provenance_uuid,
                    market_dominance_percentage
                from
                    coin_dominance
                where
                    coin_id = $1
                    and timestamp_utc >= now() at time zone 'utc' - '72 hours'::interval
                    and timestamp_utc < date_trunc('minute', now() at time zone 'utc')
                order by
                    date_trunc('minute', timestamp_utc),
                    timestamp_utc asc
                "#,
                asset.underlying().symbol().id())
                .fetch_all(pool)
                .await
                .context(SqlError)?
                .into_iter()
                .map(|r| FindByIdHistoryRow {
                    timestamp_utc_minutely: Utc.from_utc_datetime(&r.timestamp_utc_minutely.unwrap()),
                    timestamp_utc_exact: Utc.from_utc_datetime(&r.timestamp_utc),
                    provenance_uuid: r.provenance_uuid,
                    dominance_percentage: r.market_dominance_percentage,
                })
                .collect();

        Ok(FindByIdHistoryDataset {
            asset: asset.clone(),
            rows,
        })
    }
}