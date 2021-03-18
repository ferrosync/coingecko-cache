mod config;
mod coingecko;

#[macro_use]
extern crate lazy_static;

use std::collections::HashMap;
use std::error::Error;
use sqlx::PgPool;
use log::{info, error};

use domfi_util::init_logging;
use domfi_data::pg;
use crate::config::config_with_prefix;
use crate::coingecko::MarketDominanceData;
use bigdecimal::{BigDecimal, Zero};
use domfi_data::pg::models::{CoinDominanceEntry};
use std::borrow::Cow;

const DEFAULT_LOG_FILTERS: &'static str = "info,domfi_loader_historical=debug,sqlx=warn";

lazy_static! {
    static ref KNOWN_COINS_BY_SYMBOL: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert("Others", "others-coingecko-global");
        m.insert("XLM"   , "stellar");
        m.insert("XMR"   , "monero");
        m.insert("NEO"   , "neo");
        m.insert("EOS"   , "eos");

        m.insert("BSV"   , "bitcoin-cash-sv");
        m.insert("LINK"  , "chainlink");
        m.insert("BNB"   , "binancecoin");
        m.insert("BCH"   , "bitcoin-cash");
        m.insert("DOT"   , "polkadot");
        m.insert("LTC"   , "litecoin");
        m.insert("XRP"   , "ripple");
        m.insert("USDT"  , "tether");
        m.insert("ETH"   , "ethereum");
        m.insert("BTC"   , "bitcoin");
        m
    };
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let env_result = dotenv::dotenv();
    init_logging(DEFAULT_LOG_FILTERS);

    if let Err(err) = env_result {
        error!("Failed to load .env file: {}", err);
    }

    let config = config_with_prefix("DOMFI_LOADER_HIST").await?;
    let db_pool = PgPool::connect(&config.postgres_url).await?;

    // Fetch and insert into provenance
    let url = "https://www.coingecko.com/global_charts/market_dominance_data?locale=en";
    let fetch =
        pg::ops::provenance::insert_from_json_url_with_client::<MarketDominanceData, _, _>(
            &config.agent_name,
            url,
            &db_pool)
            .await?;

    let x: Vec<_> =
        fetch.json.series_data_array.into_iter()
            .filter_map(|x| {
                let coin_id = (&*KNOWN_COINS_BY_SYMBOL).get(x.name.as_str());
                coin_id.map(|id| (id, x))
            })
            .flat_map(|(coin_id, series)| {
                let coin_name = series.name.clone();
                series.data.into_iter()
                    .flat_map(move |row| {
                        let coin_name = coin_name.clone();
                        let timestamp = row.0;
                        let value_opt = row.1;
                        value_opt.map(|dom_perc| CoinDominanceEntry {
                            name: Cow::Owned(coin_name),
                            id: Cow::Borrowed(coin_id),
                            market_cap_usd: Cow::Owned(BigDecimal::zero()),
                            dominance_percentage: Cow::Owned(dom_perc),
                            timestamp: Cow::Owned(timestamp),
                        })
                    })
            })
            .collect();

    info!("Got {} rows", x.len());

    pg::ops::coin_dominance_entry::insert(
        &config.agent_name,
        &fetch.provenance,
        x.as_slice(),
        &db_pool)
        .await?;

    info!("Done.");

    Ok(())
}
