use actix_web::{Responder, HttpResponse, web, get};
use chrono::{Utc, NaiveDateTime, DateTime};
use sqlx::PgPool;
use uuid::Uuid;
use crate::repo;
use crate::api::convert::ToResponse;
use crate::api::models::{
    ResponseStatus,
    PingResponse,
    ErrorResponse,
    ProvenanceResponse,
    CoinDominanceQuery,
    CoinDominanceResponse,
    CoinDominanceElement,
    CoinDominanceMeta
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

    HttpResponse::Ok().json(ProvenanceResponse::from_repo(data))
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
pub async fn get_coingecko_coin_dominance(query: web::Query<CoinDominanceQuery>, db: web::Data<PgPool>) -> impl Responder {
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
            .map(|x| CoinDominanceElement::from_repo(x))
            .collect(),
        timestamp: data.meta.actual_timestamp_utc,
        meta: CoinDominanceMeta::from_repo(data.meta),
    };

    HttpResponse::Ok().json(response)
}
