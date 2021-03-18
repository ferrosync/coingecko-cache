use std::io::Cursor;

use log::{warn};
use bytes::Bytes;
use sha2::{Sha256, Digest};
use snafu::{Snafu, ResultExt};
use sqlx::PgPool;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use serde::Serialize;
use serde_json::Value;
use serde::de::DeserializeOwned;

use reqwest::IntoUrl;
use reqwest::header::CONTENT_TYPE;

use crate::pg::models::{ProvenanceId, RequestMetadata, ResponseMetadata};
use crate::pg::convert::ToMetadata;

pub async fn insert(
    timestamp: &DateTime<Utc>,
    agent_name: impl AsRef<str>,
    buffer: &Bytes,
    mime: Option<String>,
    request_meta: &Option<serde_json::Value>,
    response_meta: &Option<serde_json::Value>,
    pool: &PgPool,
) -> Result<ProvenanceId, sqlx::Error> {

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

    sqlx::query!(r#"
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
        agent_name.as_ref(),
        timestamp.naive_utc(),
        *request_meta,
        *response_meta)
        .execute(&mut tx)
        .await?;

    tx.commit().await?;
    Ok(ProvenanceId { uuid, object_id })
}


pub async fn insert_from_http<S: AsRef<str>>(
    timestamp: &DateTime<Utc>,
    agent_name: S,
    buffer: &Bytes,
    mime: Option<String>,
    request_meta: &RequestMetadata,
    response_meta: &ResponseMetadata,
    pool: &PgPool
) -> Result<ProvenanceId, sqlx::Error> {

    fn convert<T: Serialize>(value: T) -> Option<Value> {
        let json = serde_json::to_value(value);
        if let Err(ref err) = json {
            warn!("Failed to serialize snapshot metadata: {}", err);
        }
        json.ok()
    }

    let request_meta = convert(request_meta);
    let response_meta = convert(response_meta);

    let pid = insert(
        &timestamp,
        agent_name,
        buffer,
        mime,
        &request_meta,
        &response_meta,
        &pool
    ).await?;

    Ok(pid)
}

pub struct FetchIntoProvenanceOutput<Json> {
    pub imported_at: DateTime<Utc>,
    pub provenance: ProvenanceId,
    pub json: Json,
}


#[derive(Snafu, Debug)]
pub enum FetchToInsertError {
    #[snafu(display("Failed to complete HTTP request: {}", source))]
    HttpError {
        source: reqwest::Error,
    },

    #[snafu(display("Failed to deserialize response: {}", source))]
    DeserializationError {
        source: serde_json::Error,
    },

    #[snafu(display("Failed to insert into database: {}", source))]
    DbError {
        source: sqlx::Error,
    }
}

pub async fn insert_from_json_url_with_client<Json, S, U>(
    agent_name: S,
    url: U,
    pool: &PgPool)
    -> Result<FetchIntoProvenanceOutput<Json>, FetchToInsertError>
where
    Json: DeserializeOwned,
    S: AsRef<str>,
    U: IntoUrl,
{
    let http = reqwest::ClientBuilder::new().build()
        .context(HttpError)?;

    insert_from_json_url::<Json, S, U>(
        agent_name,
        url,
        &http,
        pool).await
}

pub async fn insert_from_json_url<Json, S, U>(
    agent_name: S,
    url: U,
    http: &reqwest::Client,
    pool: &PgPool)
    -> Result<FetchIntoProvenanceOutput<Json>, FetchToInsertError>
where
    Json: DeserializeOwned,
    S: AsRef<str>,
    U: IntoUrl,
{
    let request_builder = http.get(url);
    let request = request_builder.build().context(HttpError)?;
    let request_meta = request.to_metadata();

    let response = http.execute(request).await.context(HttpError)?;
    let now = Utc::now();

    let mime = response.headers()
        .get(CONTENT_TYPE)
        .and_then(|x| x.to_str().map(|s| s.to_string()).ok());

    let response_meta = response.to_metadata();
    let buffer = response.bytes().await.context(HttpError)?;

    let pid = super::provenance::insert_from_http(
        &now,
        agent_name,
        &buffer,
        mime,
        &request_meta,
        &response_meta,
        &pool,
    ).await.context(DbError)?;

    let cursor = Cursor::new(buffer);
    let json = serde_json::from_reader::<_, Json>(cursor).context(DeserializationError)?;

    Ok(FetchIntoProvenanceOutput {
        imported_at: now,
        provenance: pid,
        json,
    })
}
