mod api;
mod repo;
mod base64;
mod convert;
mod historical;

#[macro_use]
extern crate log;

#[macro_use]
extern crate lazy_static;

use std::env;
use std::error::Error;

use actix_web::middleware::Logger;
use actix_web::{web, App, HttpServer, middleware};
use sqlx::PgPool;
use dotenv::dotenv;
use listenfd::ListenFd;

use domfi_util::init_logging;
use crate::historical::HistoricalCacheService;

const DEFAULT_LOG_FILTERS: &'static str =
    "actix_server=info,actix_web=info,sqlx=warn,coingecko_cache_api=info,warn";

#[actix_web::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();
    init_logging(DEFAULT_LOG_FILTERS);

    // Enable receiving passed file descriptors
    // Launch using `systemfd --no-pid -s http::PORT -- cargo watch -x run` to leverage this
    //
    let mut listenfd = ListenFd::from_env();

    // Database
    let database_url = env::var("DOMFI_API_DATABASE_URL").expect("DATABASE_URL is not set in .env file");
    let db_pool = PgPool::connect(&database_url).await?;

    let (historical_cache_service, history_rx) =
        HistoricalCacheService::new(
            db_pool.clone(),
            1024,
            std::time::Duration::from_secs(5 * 60),
            std::time::Duration::from_secs(45));

    tokio::spawn(historical_cache_service.into_run());

    // HTTP Server
    let mut server = HttpServer::new(move || {
        App::new()
            .wrap(Logger::new(r#"%{r}a [%a] "%r" %s %b "%{Referer}i" "%{User-Agent}i" %Dms"#))
            .wrap(middleware::Compress::default())
            .data(history_rx.clone())
            .data(db_pool.clone())
            .service(web::scope("/api/v0/")
                .service(api::services()))
    });

    // Launch server from listenfd
    server = match listenfd.take_tcp_listener(0)? {
        Some(listener) => {
            info!("Using listenfd");
            server.listen(listener)?
        },
        None => {
            info!("Binding to listen address");

            let host = env::var("DOMFI_API_HOST").expect("DOMFI_API_HOST is not set in .env file");
            let port = env::var("DOMFI_API_PORT").expect("DOMFI_API_PORT is not set in .env file");

            let addr_input = format!("{}:{}", host, port);
            let sockets = domfi_ext_tcp::bind_to(addr_input, 2048)?;
            for s in sockets {
                server = server.listen(s)?;
            }

            server
        }
    };

    info!("Starting server");
    server.run().await?;
    Ok(())
}
