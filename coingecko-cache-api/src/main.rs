mod api;
mod repo;
mod base64;
mod convert;
mod ext;

#[macro_use]
extern crate log;

use std::{env, net, io};
use std::error::Error;

use actix_web::middleware::Logger;
use actix_web::{web, App, HttpServer};
use sqlx::PgPool;

use dotenv::dotenv;
use listenfd::ListenFd;
use net2::TcpBuilder;
use net2::unix::UnixTcpBuilderExt;

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
        Some(listener) => {
            info!("Using listenfd");
            server.listen(listener)?
        },
        None => {
            info!("Binding to listen address");

            let host = env::var("DOMFI_API_HOST").expect("DOMFI_API_HOST is not set in .env file");
            let port = env::var("DOMFI_API_PORT").expect("DOMFI_API_PORT is not set in .env file");

            let addr_input = format!("{}:{}", host, port);
            let sockets = bind_to(addr_input, 2048)?;
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

fn create_tcp_listener(
    addr: net::SocketAddr,
    backlog: i32,
) -> io::Result<net::TcpListener> {
    let builder = match addr {
        net::SocketAddr::V4(_) => TcpBuilder::new_v4()?,
        net::SocketAddr::V6(_) => TcpBuilder::new_v6()?,
    };

    let socket = builder
        .reuse_address(true)?
        .reuse_port(true)?
        .bind(addr)?
        .listen(backlog)?;

    Ok(socket)
}

fn bind_to<A: net::ToSocketAddrs>(
    addr: A,
    backlog: i32,
) -> io::Result<Vec<net::TcpListener>> {
    let mut err = None;
    let mut success = false;
    let mut sockets = Vec::new();

    for addr in addr.to_socket_addrs()? {
        match create_tcp_listener(addr, backlog) {
            Ok(lst) => {
                success = true;
                sockets.push(lst);
            }
            Err(e) => err = Some(e),
        }
    }

    if !success {
        if let Some(e) = err.take() {
            Err(e)
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "Can not bind to address.",
            ))
        }
    } else {
        Ok(sockets)
    }
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
