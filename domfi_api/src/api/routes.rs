use std::ops::Deref;

use actix_web::{Responder, HttpResponse, web, get};
use sqlx::PgPool;
use log::error;
use snafu::Snafu;

use serde::Deserialize;
use chrono::{Utc, NaiveDateTime, DateTime};
use uuid::Uuid;
use bigdecimal::BigDecimal;
use qstring::QString;

use domfi_domain::round_price_identifier;
use crate::repo;
use crate::api::convert::ToResponse;
use crate::api::models::{ResponseStatus, PingResponse, ErrorResponse, ProvenanceResponse, TimestampQuery, CoinDominanceResponse, PricesResponse, PriceByIdResponse, HistoryResponse, HistoryResponseSlim};
use crate::historical::{HistoricalCacheServiceRef, HistoryFetchRequest, ClientFindByIdHistoryError};
use domfi_domain::models::FinancialAssetValueOf;
use domfi_domain::models::FinancialAssetRawValueOf;
use domfi_domain::models::financial_assets::get_canonical_default_asset;

#[get("/ping")]
pub async fn ping() -> impl Responder {
    let now = Utc::now();
    HttpResponse::Ok().json(PingResponse {
        status: ResponseStatus::Success,
        timestamp: now,
    })
}


#[get("/provenance/{id}")]
pub async fn get_data_origin(id: web::Path<Uuid>, db: web::Data<PgPool>) -> impl Responder {

    let result =
        repo::DataOriginRepo::get_by_uuid(*id, db.get_ref())
            .await;

    let data = match result {
        Ok(x) => x,
        Err(e) => return e.to_response(),
    };

    HttpResponse::Ok().json(ProvenanceResponse::from(data))
}

#[get("/blob/{hash}")]
pub async fn get_blob(hash: web::Path<String>, db: web::Data<PgPool>) -> impl Responder {

    let hex = hex::decode(hash.into_inner());
    let hex = match hex {
        Err(e) =>
            return HttpResponse::BadRequest().json(ErrorResponse::new(format!("Invalid SHA256 hash: {}", e))),
        Ok(buf) => buf,
    };

    if hex.len() != 32 {
        return HttpResponse::BadRequest().json(
            ErrorResponse::new("Invalid SHA256 hash: Expected length to be 32 bytes".into()));
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

#[get("/coingecko/coin_dominance")]
pub async fn get_coingecko_coin_dominance(query: web::Query<TimestampQuery>, db: web::Data<PgPool>) -> impl Responder {
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
        status: ResponseStatus::Success,
        data: data.elements.into_iter()
            .map(|x| x.into())
            .collect(),
        timestamp: data.meta.actual_timestamp_utc,
        meta: data.meta.into(),
    };

    HttpResponse::Ok().json(response)
}

#[get("/price")]
pub async fn get_prices(query: web::Query<TimestampQuery>, db: web::Data<PgPool>) -> impl Responder {
    let ts = query.timestamp
        .map(|x| DateTime::from_utc(NaiveDateTime::from_timestamp(x as i64, 0), Utc));

    let result =
        repo::CoinDominanceRepo::find_by_timestamp_rounded(ts, db.get_ref())
            .await;

    let data = match result {
        Ok(x) => x,
        Err(e) => return e.to_response(),
    };

    let mut prices: Vec<(&str, BigDecimal)> = Vec::new();

    for x in data.elements.iter() {
        let id = if x.id.is_empty() { "others" } else { &x.id };
        prices.push((id, round_price_identifier(&x.dominance_percentage)));
    }

    let response = PricesResponse {
        status: ResponseStatus::Success,
        data: prices,
        timestamp: data.meta.actual_timestamp_utc,
        meta: data.meta.into(),
    };

    HttpResponse::Ok().json(response)
}

#[get("/price/{id}")]
pub async fn get_price_by_id(id: web::Path<String>, query: web::Query<TimestampQuery>, db: web::Data<PgPool>) -> impl Responder {
    let ts = query.timestamp
        .map(|x| DateTime::from_utc(NaiveDateTime::from_timestamp(x as i64, 0), Utc));

    let asset_meta = match get_canonical_default_asset(id.as_str()) {
        None => return ClientFindByIdHistoryError::CoinUnknownOrNotAllowed.to_response(),
        Some(x) => x
    };

    let result =
        repo::CoinDominanceRepo::find_by_id_at_timestamp_rounded(asset_meta.asset(), ts, db.get_ref())
            .await;

    let data = match result {
        Ok(x) => x,
        Err(e) => return e.to_response(),
    };

    let response = PriceByIdResponse {
        status: ResponseStatus::Success,
        coin_id: &data.coin_id,
        coin_symbol: &data.coin_symbol,
        price: &asset_meta.value_of(&data.percentage),
        price_original: &asset_meta.raw_value_of(&data.percentage),
        timestamp: data.meta.actual_timestamp_utc,
        meta: data.meta.into(),
    };

    HttpResponse::Ok().json(response)
}

fn default_as_false() -> bool {
    false
}

#[derive(Deserialize, Debug)]
pub struct GetPriceHistoryByIdQuery {
    #[serde(default="default_as_false")]
    full: bool
}

#[get("/price/{id}/history")]
pub async fn get_price_historical_by_id(
    id: web::Path<String>,
    req: web::HttpRequest,
    history_service: web::Data<HistoricalCacheServiceRef>
) -> impl Responder {

    let history_rx = history_service.get_ref();

    let (msg, rx) = HistoryFetchRequest::new_with_receiver(id.deref().to_owned());
    if let Err(e) = history_rx.clone().send(msg).await {
        error!("Failed to send message to history fetch service: {}", e);
        return e.to_response()
    }

    let dataset_result = match rx.await {
        Err(e) => {
            error!("Failed to receive from history fetch service: {}", e);
            return e.to_response();
        },
        Ok(x) => x
    };

    let dataset = match dataset_result {
        Err(e) => {
            error!("Failed to fetch history dataset: {}", e);
            return e.to_response();
        }
        Ok(x) => x,
    };

    let use_full = match is_query_flag_set(req.query_string(), "full") {
        Err(e) => return e.to_response(),
        Ok(x) => x,
    };

    return if use_full {
        HttpResponse::Ok().json(HistoryResponse {
            status: ResponseStatus::Success,
            data: dataset
        })
    } else {
        HttpResponse::Ok().json(HistoryResponseSlim {
            status: ResponseStatus::Success,
            data: dataset.deref().into(),
        })
    };
}

#[derive(Snafu, Debug)]
pub enum QueryFlagError {
    #[snafu(display("Unrecognized input for query parameter '{}'. Expected boolean. Got '{}'", param, input))]
    UnrecognizedValue {
        param: String,
        input: String,
    },

    #[snafu(display("Unrecognized input for query parameter '{}'. Expected boolean.", param))]
    InputTooLong {
        param: String,
    }
}

fn is_query_flag_set(query: &str, param: &str) -> Result<bool, QueryFlagError> {
    let arg = match QString::from(query)
        .get(param)
        .map(|x| x.to_ascii_lowercase())
    {
        None => return Ok(false),
        Some(x) => x,
    };

    if arg.len() > 10 {
        return Err(QueryFlagError::InputTooLong {
            param: param.to_owned(),
        })
    }

    match arg.as_str() {
        ""
        | "t"
        | "true"
        | "y"
        | "yes" => Ok(true),

        "f"
        | "false"
        | "n"
        | "no" => Ok(false),

        _ => Err(QueryFlagError::UnrecognizedValue {
            param: param.to_owned(),
            input: arg,
        }),
    }
}
