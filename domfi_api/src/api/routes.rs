use actix_web::{Responder, HttpResponse, web, get};
use chrono::{Utc, NaiveDateTime, DateTime};
use sqlx::PgPool;
use uuid::Uuid;
use bigdecimal::BigDecimal;
use crate::{repo, convert};
use crate::api::convert::ToResponse;
use crate::api::models::{
    ResponseStatus,
    PingResponse,
    ErrorResponse,
    ProvenanceResponse,
    TimestampQuery,
    CoinDominanceResponse,
    PricesResponse,
    PriceByIdResponse
};

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
        prices.push((id, convert::round_price_identifier(&x.dominance_percentage)));
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

    let result =
        repo::CoinDominanceRepo::find_by_id_at_timestamp_rounded(&**id, ts, db.get_ref())
            .await;

    let data = match result {
        Ok(x) => x,
        Err(e) => return e.to_response(),
    };

    let response = PriceByIdResponse {
        status: ResponseStatus::Success,
        coin_id: &data.coin_id,
        coin_symbol: &data.coin_symbol,
        price: &convert::round_price_identifier(&data.percentage),
        price_original: &data.percentage,
        timestamp: data.meta.actual_timestamp_utc,
        meta: data.meta.into(),
    };

    HttpResponse::Ok().json(response)
}
