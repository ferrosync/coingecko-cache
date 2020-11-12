mod repo;

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
use sqlx::Cursor;

use crate::repo::{OriginMetadata, FindByTimestampResult, RepositoryError};

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

#[derive(Serialize)]
struct CoinDominanceMeta {
    origin: Uuid,

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
            origin: data.origin_uuid,
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

#[derive(Serialize)]
struct DataOriginResponse {
    pub uuid: Uuid,
    pub agent: String,
    pub imported_at: DateTime<Utc>,
    pub data: String,
}

impl DataOriginResponse {
    fn from_repo(x: repo::DataOrigin) -> DataOriginResponse {
        Self {
            uuid: x.uuid,
            agent: x.agent,
            imported_at: x.timestamp_utc,
            data: x.data,
        }
    }
}

#[get("/api/v0/origin/{id}")]
async fn get_data_origin(id: web::Path<Uuid>, db: web::Data<PgPool>) -> impl Responder {

    let result =
        repo::DataOriginRepo::get_by_uuid(*id, db.get_ref())
            .await;

    let data = match result {
        Ok(x) => x,
        Err(e) => {
            return match e {
                RepositoryError::SqlError { source: sqlx::Error::RowNotFound } => {
                    HttpResponse::NotFound().json(ErrorResponse {
                        status: "error".into(),
                        reason: "Unable to find data origin requested".into(),
                    })
                }
                RepositoryError::SqlError { source } => {
                    error!("Database error: {}", source);
                    HttpResponse::InternalServerError().json(ErrorResponse {
                        status: "error".into(),
                        reason: "Invalid database connection error".into(),
                    })
                },
                RepositoryError::NoRecordsFound =>
                    HttpResponse::NotFound().json(ErrorResponse {
                        status: "error".into(),
                        reason: "Unable to find data origin requested".into(),
                    })
            }
        }
    };

    HttpResponse::Ok().json(DataOriginResponse::from_repo(data))
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
        Err(e) => {
            return match e {
                RepositoryError::SqlError { source } => {
                    error!("Database error: {}", source);
                    HttpResponse::InternalServerError().json(ErrorResponse {
                        status: "error".into(),
                        reason: "Invalid database connection error".into(),
                    })
                }
                RepositoryError::NoRecordsFound =>
                    HttpResponse::NotFound().json(ErrorResponse {
                        status: "error".into(),
                        reason: "Unable to find timestamp requested".into(),
                    })
            }
        }
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
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL is not set in .env file");
    let db_pool = PgPool::new(&database_url).await?;

    // HTTP Server
    let mut server = HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .data(db_pool.clone())
            .service(hello)
            .service(get_coingecko_coin_dominance)
            .service(get_data_origin)
            .route("/hey", web::get().to(manual_hello))
    });

    // Launch server from listenfd
    server = match listenfd.take_tcp_listener(0)? {
        Some(listener) => server.listen(listener)?,
        None => {
            let host = env::var("HOST").expect("HOST is not set in .env file");
            let port = env::var("PORT").expect("PORT is not set in .env file");
            server.bind(format!("{}:{}", host, port))?
        }
    };

    info!("Starting server");
    server.run().await?;

    Ok(())
}
