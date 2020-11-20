mod api;
mod repo;
mod base64;
mod convert;
mod ext;

#[macro_use]
extern crate log;

use std::env;
use std::error::Error;

use actix_web::middleware::Logger;
use actix_web::{web, App, HttpServer};
use sqlx::PgPool;

use dotenv::dotenv;
use listenfd::ListenFd;

#[actix_web::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();
    init_logging();

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
            .service(web::scope("/api/v0/")
                    .service(api::services()))
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

fn init_logging() {
    let log_env_default = "actix_server=info,actix_web=info,sqlx=warn,coingecko_cache_api=info,warn";
    let log_env_raw = env::var("RUST_LOG");
    let log_env = log_env_raw.clone().ok()
        .filter(|env| !env.is_empty())
        .unwrap_or(log_env_default.into());

    pretty_env_logger::formatted_timed_builder()
        .parse_filters(&log_env)
        .init();

    match &log_env_raw {
        Err(env::VarError::NotUnicode(..)) =>
            error!("Failed to read 'RUST_LOG' due to invalid Unicode. Using default instead: '{}'", log_env_default),

        Err(env::VarError::NotPresent) =>
            warn!("Missing 'RUST_LOG'. Using default instead: '{}'", log_env_default),

        Ok(s) if s.is_empty() =>
            warn!("Got empty 'RUST_LOG'. Using default instead: '{}'", log_env_default),

        Ok(_) => (),
    }
}
