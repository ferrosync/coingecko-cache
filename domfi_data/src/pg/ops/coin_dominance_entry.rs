use log::info;
use sqlx::PgPool;
use crate::pg::models::{
    ProvenanceId,
    CoinDominanceEntry
};
use std::ops::Deref;

pub async fn insert<'a>(
    agent_name: &str,
    pid: &ProvenanceId,
    entries: &'a [CoinDominanceEntry<'a>],
    pool: &PgPool)
    -> Result<(), sqlx::Error>
{
    let mut tx = pool.begin().await?;

    let mut i = 0usize;
    for coin in entries.iter() {
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
        agent_name,
        coin.timestamp.naive_utc(),
        coin.id.deref(),
        coin.name.deref(),
        coin.market_cap_usd.deref(),
        coin.dominance_percentage.deref())
            .execute(&mut tx)
            .await?;

        i += 1;
        if i % 1000 == 0 {
            info!("INSERT {}/{} ({:.2}%)", i, entries.len(), (i as f64) / (entries.len() as f64) * 100f64)
        }
    }

    if entries.len() >= 1000 {
        info!("INSERT {}/{} rows ({:.2}%)", i, entries.len(), (i as f64) / (entries.len() as f64) * 100f64);
    }

    tx.commit().await?;
    Ok(())
}

