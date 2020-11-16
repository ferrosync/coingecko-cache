mod repo;
mod base64;

#[macro_use]
extern crate log;

use std::env;
use std::str::FromStr;
use std::error::Error;
use std::ops::Add;

use serde::{self, Serialize, Deserialize};
use serde_with::{serde_as, SerializeAs};
use serde_with::serde::Serializer;
use serde_json::value::RawValue;

use actix_web::middleware::Logger;
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};

use sqlx::types::BigDecimal;
use chrono::serde::{ts_milliseconds, ts_seconds};
use chrono::{DateTime, Utc, DurationRound, SubsecRound, Timelike, Duration};

use dotenv::dotenv;
use listenfd::ListenFd;
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::chrono::NaiveDateTime;

use crate::repo::{OriginMetadata, FindByTimestampResult, RepositoryError};
use actix_web::body::Body;

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

#[derive(Deserialize)]
struct CoinDominanceQuery {
    timestamp: Option<u64>,
}

#[serde_as]
#[derive(Serialize)]
struct CoinDominanceElement {
    name: String,
    id: String,

    #[serde_as(as = "ToStringVerbatim")]
    market_cap_usd: BigDecimal,

    #[serde_as(as = "ToStringVerbatim")]
    dominance_percentage: BigDecimal,
}

impl CoinDominanceElement {
    fn from_repo(data: repo::CoinDominanceRecord) -> CoinDominanceElement {
        Self {
            name: data.name,
            id: data.id,
            market_cap_usd: data.market_cap_usd,
            dominance_percentage: data.dominance_percentage,
        }
    }
}

struct ToStringVerbatim { }

impl<T> SerializeAs<T> for ToStringVerbatim
    where
        T: ToString,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
    {
        let raw_value = RawValue::from_string(source.to_string()).unwrap(); // HACK!
        raw_value.serialize(serializer)
    }
}

#[serde_as]
#[derive(Serialize)]
struct CoinDominanceMeta {
    provenance_uuid: Uuid,

    #[serde_as(as = "serde_with::hex::Hex")]
    blob_sha256: Vec<u8>,

    #[serde(with = "ts_milliseconds")]
    imported_at_timestamp: DateTime<Utc>,

    #[serde(with = "ts_milliseconds")]
    requested_timestamp: DateTime<Utc>,

    #[serde(with = "ts_milliseconds")]
    actual_timestamp: DateTime<Utc>,
}

impl CoinDominanceMeta {
    fn from_repo(data: OriginMetadata) -> CoinDominanceMeta {
        Self {
            provenance_uuid: data.provenance_uuid,
            blob_sha256: data.blob_sha256,
            imported_at_timestamp: data.imported_at_utc,
            requested_timestamp: data.requested_timestamp_utc,
            actual_timestamp: data.actual_timestamp_utc,
        }
    }
}

#[derive(Serialize)]
struct CoinDominanceResponse {
    data: Vec<CoinDominanceElement>,

    #[serde(with = "ts_seconds")]
    timestamp: DateTime<Utc>,

    meta: CoinDominanceMeta,
}

#[derive(Serialize)]
struct ErrorResponse {
    status: String,
    reason: String,
}

#[serde_as]
#[derive(Serialize)]
struct ProvenanceResponse {
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

impl ProvenanceResponse {
    fn from_repo(x: repo::Provenance) -> ProvenanceResponse {
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

trait ToResponse {
    type Output : Responder;
    fn to_response(&self) -> Self::Output;
}

impl ToResponse for RepositoryError {
    type Output = HttpResponse<Body>;
    fn to_response(&self) -> Self::Output {
        match self {
            RepositoryError::SqlError { source: sqlx::Error::RowNotFound } => {
                HttpResponse::NotFound().json(ErrorResponse {
                    status: "error".into(),
                    reason: "Unable to find data origin requested".into(),
                })
            },
            RepositoryError::SqlError { source } => {
                error!("Database error: {}", source);
                HttpResponse::InternalServerError().json(ErrorResponse {
                    status: "error".into(),
                    reason: "Invalid database connection error".into(),
                })
            },
        }
    }
}

#[get("/api/v0/provenance/{id}")]
async fn get_data_origin(id: web::Path<Uuid>, db: web::Data<PgPool>) -> impl Responder {

    let result =
        repo::DataOriginRepo::get_by_uuid(*id, db.get_ref())
            .await;

    let data = match result {
        Ok(x) => x,
        Err(e) => return e.to_response(),
    };

    HttpResponse::Ok().json(ProvenanceResponse::from_repo(data))
}

#[get("/api/v0/blob/{hash}")]
async fn get_blob(hash: web::Path<String>, db: web::Data<PgPool>) -> impl Responder {

    let hex = hex::decode(hash.into_inner());
    let hex = match hex {
        Err(e) => return HttpResponse::BadRequest().json(ErrorResponse {
            status: "error".into(),
            reason: format!("Invalid SHA256 hash: {}", e),
        }),
        Ok(buf) => buf,
    };

    if hex.len() != 32 {
        return HttpResponse::BadRequest().json(ErrorResponse {
            status: "error".into(),
            reason: "Invalid SHA256 hash: Expected length to be 32 bytes".into(),
        })
    }

    let result =
        repo::ObjectStorageRepo::get_by_sha256(hex.as_slice(), db.get_ref())
            .await;

    let data = match result {
        Ok(x) => x,
        Err(e) => return e.to_response(),
    };

    HttpResponse::Ok()
        .content_type(data.mime.unwrap_or("application/octet-stream".into()))
        .body(data.data)
}

#[get("/api/v0/coingecko/coin_dominance")]
async fn get_coingecko_coin_dominance(query: web::Query<CoinDominanceQuery>, db: web::Data<PgPool>) -> impl Responder {
    let ts = query.timestamp
        .map(|x| DateTime::from_utc(NaiveDateTime::from_timestamp(x as i64, 0), Utc));

    let result =
        repo::CoinDominanceRepo::find_by_timestamp_rounded(ts, db.get_ref())
            .await;

    let data = match result {
        Ok(x) => x,
        Err(e) => return e.to_response(),
    };

    let response = CoinDominanceResponse {
        data: data.elements.into_iter()
            .map(|x| CoinDominanceElement::from_repo(x))
            .collect(),
        timestamp: data.meta.actual_timestamp_utc,
        meta: CoinDominanceMeta::from_repo(data.meta),
    };

    HttpResponse::Ok().json(response)
}

async fn manual_hello() -> impl Responder {
    HttpResponse::Ok().body("Hey there!")
}

#[actix_web::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    dotenv().ok();

    // Enable receiving passed file descriptors
    // Launch using `systemfd --no-pid -s http::PORT -- cargo watch -x run` to leverage this
    //
    let mut listenfd = ListenFd::from_env();

    // Database
    let database_url = env::var("DOMFI_API_DATABASE_URL").expect("DATABASE_URL is not set in .env file");
    let db_pool = PgPool::connect(&database_url).await?;

    // HTTP Server
    let mut server = HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .data(db_pool.clone())
            .service(hello)
            .service(get_coingecko_coin_dominance)
            .service(get_data_origin)
            .service(get_blob)
            .route("/hey", web::get().to(manual_hello))
    });

    // Launch server from listenfd
    server = match listenfd.take_tcp_listener(0)? {
        Some(listener) => server.listen(listener)?,
        None => {
            let host = env::var("DOMFI_API_HOST").expect("DOMFI_API_HOST is not set in .env file");
            let port = env::var("DOMFI_API_PORT").expect("DOMFI_API_PORT is not set in .env file");
            server.bind(format!("{}:{}", host, port))?
        }
    };

    info!("Starting server");
    server.run().await?;

    Ok(())
}
