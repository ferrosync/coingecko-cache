pub mod models;
pub mod routes;
mod convert;

use actix_web::{web, Scope};

pub fn services() -> Scope {
    web::scope("/")
        .service(routes::ping)
        .service(routes::get_blob)
        .service(routes::get_coingecko_coin_dominance)
        .service(routes::get_data_origin)
        .service(routes::get_prices)
        .service(routes::get_price_by_id)
}
