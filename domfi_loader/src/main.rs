mod coingecko;
mod util;

use std::env;
use std::error::Error;
use std::borrow::Cow;

use log::{info, warn, error};
use leaky_bucket::LeakyBuckets;
use tokio::time::Duration;
use reqwest::Url;
use sqlx::PgPool;

use domfi_util::init_logging;
use domfi_data::pg;
use domfi_data::pg::models::CoinDominanceEntry;
use crate::util::{UrlCacheBuster, AtomicCancellation};

const DEFAULT_LOG_FILTERS: &'static str = "info,domfi_loader=debug";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let env_result = dotenv::dotenv();
    init_logging(DEFAULT_LOG_FILTERS);

    if let Err(err) = env_result {
        error!("Failed to load .env file: {}", err);
    }

    let config = get_config().await?;
    let db_pool = PgPool::connect(&config.database_url).await?;

    info!("Rate limit interval set to 1 req/{:#?}", config.rate_limit_interval);

    //

    let http = reqwest::ClientBuilder::new().build().expect("Failed to build HTTP client");

    let mut buckets = LeakyBuckets::new();

    // spawn the coordinate thread to refill the rate limiter.
    let coordinator = buckets.coordinate()?;
    tokio::spawn(async move { coordinator.await.expect("Failed to start rate limiter coordinator") });

    let rate_limiter = buckets.rate_limiter()
        .max(5)
        .tokens(1)
        .refill_amount(1)
        .refill_interval(config.rate_limit_interval)
        .build()
        .expect("Failed to build request rate limiter");

    let stop_signal_source = AtomicCancellation::new();

    let h = stop_signal_source.clone();
    ctrlc::set_handler(move || {
        let handle = &h;
        handle.cancel();
        info!("Shutting down from Ctrl-C signal...");
    })
    .expect("Error setting Ctrl-C handler");

    let stop_handle = stop_signal_source.clone();
    let mut url_gen = config.url_gen();
    while stop_handle.can_continue() {
        rate_limiter.acquire_one().await?;

        let url = url_gen.next();
        let result =
            pg::ops::provenance::insert_from_json_url::<coingecko::CoinDominanceResponse, _, _>(
                &config.agent_name,
                url,
                &http,
                &db_pool
            ).await;

        let ctx = match result {
            Err(err) => {
                error!("Failed to insert provenance! Cause: {}", err);
                continue;
            },
            Ok(x) => x,
        };

        let dataset_timestamp = ctx.json.timestamp;
        let rows: Vec<_> = ctx.json.data.into_iter()
            .map(|r| {
                CoinDominanceEntry {
                    name: Cow::Owned(r.name),
                    id: Cow::Owned(r.id),
                    market_cap_usd: Cow::Owned(r.market_cap_usd),
                    dominance_percentage: Cow::Owned(r.dominance_percentage),
                    timestamp: Cow::Borrowed(&dataset_timestamp)
                }
            })
            .collect();

        let result =
            pg::ops::coin_dominance_entry::insert(
                &config.agent_name,
                &ctx.provenance,
                rows.as_slice(),
                &db_pool,
            ).await;

        let provenance_uuid = &ctx.provenance.uuid;
        match result {
            Err(e) => {
                error!("[{}] Failed to insert coin dominance! Cause: {}", provenance_uuid, e);
                continue;
            },
            Ok(_) => {
                info!("[{}] Committed snapshot at {}", provenance_uuid, &ctx.imported_at);
            }
        }
    }

    info!("Quitting");
    Ok(())
}

struct Config {
    agent_name: String,
    database_url: String,
    url: Url,
    rate_limit_interval: Duration,
}

impl Config {
    fn url_gen(&self) -> UrlCacheBuster {
        UrlCacheBuster::new(&self.url)
    }
}

async fn get_config() -> Result<Config, Box<dyn Error>> {
    let agent_name = env::var("DOMFI_LOADER_AGENT_NAME").unwrap_or("loader_rust".into());
    info!("Agent name: '{}'", agent_name);

    let database_url = env::var("DOMFI_LOADER_DATABASE_URL").expect("DOMFI_LOADER_DATABASE_URL missing or unset '.env' file");

    let url_raw = env::var("DOMFI_LOADER_URL").expect("DOMFI_LOADER_URL missing or unset in '.env' file");
    let url = Url::parse(url_raw.as_str()).expect("DOMFI_LOADER_URL has invalid URL format");

    let rate_limit_interval_default = Duration::from_millis(1250);
    let rate_limit_interval = env::var("DOMFI_LOADER_INTERVAL").ok()
        .and_then(|s| match parse_duration::parse(&s) {
            Err(err) => {
                warn!("Failed to parse 'DOMFI_LOADER_INTERVAL'. Using default value instead of '{:#?}'. Cause: {}",
                      rate_limit_interval_default, err);
                None
            }
            Ok(dur) => Some(dur)
        })
        .unwrap_or(rate_limit_interval_default);

    Ok(Config {
        agent_name,
        database_url,
        url,
        rate_limit_interval,
    })
}

