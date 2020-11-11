use chrono::{DateTime, Utc, SubsecRound, Timelike, Duration, TimeZone};
use sqlx::PgPool;
use std::ops::Add;
use sqlx::types::Uuid;
use snafu::{ensure, Snafu, ResultExt};
use sqlx::types::BigDecimal;
use sqlx::Cursor;
use sqlx::postgres::PgCursor;
use std::iter::Iterator;
use futures::prelude::*;

pub struct OriginMetadata {
    pub timestamp_utc: DateTime<Utc>,
    pub imported_at_utc: DateTime<Utc>,
    pub origin_uuid: Uuid,
    pub agent: String,
}

pub struct CoinDominanceRecord {
    pub name: String,
    pub id: String,
    pub market_cap_usd: BigDecimal,
    pub dominance_percentage: BigDecimal,
}

pub struct FindByTimestampResult {
    pub meta: OriginMetadata,
    pub elements: Vec<CoinDominanceRecord>,
}

#[derive(Debug, Snafu)]
pub enum RepositoryError {
    #[snafu(display("Failed to access database: {}", source))]
    SqlError { source: sqlx::Error, },

    #[snafu(display("Query did not return any records"))]
    NoRecordsFound,
}

pub struct DataOriginRepo { }

pub struct DataOrigin {
    pub uuid: Uuid,
    pub agent: String,
    pub timestamp_utc: DateTime<Utc>,
    pub data: String,
    pub metadata: Option<Vec<String>>,
}

impl DataOriginRepo {

    pub async fn get_by_uuid(uuid: Uuid, pool: &PgPool) -> Result<DataOrigin, RepositoryError> {

        let row = sqlx::query!(r#"
            select
                uuid,
                agent,
                timestamp_utc,
                data,
                metadata
            from
                data_origin
            where
                uuid = $1
        "#, uuid)
            .fetch_one(pool)
            .await
            .context(SqlError);

        row.map(|x| DataOrigin {
            uuid: x.uuid,
            agent: x.agent,
            timestamp_utc: Utc.from_utc_datetime(&x.timestamp_utc),
            data: x.data,
            metadata: x.metadata,
        })
    }
}

pub struct CoinDominanceRepo { }

impl CoinDominanceRepo {

    pub fn round_timestamp(ts: DateTime<Utc>) -> DateTime<Utc> {
        ts.trunc_subsecs(0).with_second(0).unwrap()
    }

    pub async fn find_by_timestamp_rounded(ts: DateTime<Utc>, pool: &PgPool) -> Result<FindByTimestampResult, RepositoryError> {

        let actual_ts = Self::round_timestamp(ts);
        let actual_ts_plus_1 = actual_ts.add(Duration::minutes(1));

        let mut cursor = sqlx::query!(r#"
            with ts as (
                select
                   timestamp_utc, agent
                from coin_dominance
                where
                      timestamp_utc between $1 and $2
                order by timestamp_utc asc
                limit 1
            )
            select
                data.id,
                data.data_origin_uuid,
                data.timestamp_utc,
                data.imported_at_utc,
                data.agent,
                data.coin_id,
                data.coin_name,
                data.market_cap_usd,
                data.market_dominance_percentage
            from
                coin_dominance as data,
                ts
            where
                  data.timestamp_utc = ts.timestamp_utc
                  and data.agent = ts.agent
            "#,
            actual_ts.naive_utc(),
            actual_ts_plus_1.naive_utc())
            .fetch(pool);

        let mut elements = Vec::new();
        let meta = if let Some(x) = cursor.try_next().await.context(SqlError)? {
            elements.push(CoinDominanceRecord {
                name: x.coin_name,
                id: x.coin_id,
                market_cap_usd: x.market_cap_usd,
                dominance_percentage: x.market_dominance_percentage
            });

            let m = OriginMetadata {
                origin_uuid: x.data_origin_uuid,
                timestamp_utc: Utc.from_utc_datetime(&x.timestamp_utc),
                imported_at_utc: Utc.from_utc_datetime(&x.imported_at_utc),
                agent: x.agent,
            };

            m
        } else {
            return NoRecordsFound.fail();
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
}