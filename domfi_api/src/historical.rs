use std::sync::Arc;

use tokio::sync::{oneshot, mpsc};
use tokio::time::Duration;
use tokio::sync::mpsc::Sender;
use tokio::select;

use sqlx::PgPool;
use chrono::{DateTime, Utc};
use chrono::serde::ts_seconds;
use bigdecimal::BigDecimal;
use uuid::Uuid;
use log::{error};
use snafu::{Snafu};
use serde::{Serialize};
use ttl_cache::TtlCache;

use domfi_domain::models::{FinancialAssetWithMetadata, FinancialAssetWithMetadataOfAny, FinancialAssetValueOf, FinancialDominanceAsset, FinancialAssetRawValueOf};
use crate::repo::{RepositoryError, CoinDominanceRepo};
use domfi_domain::models::financial_assets::get_canonical_default_asset;

#[derive(Debug)]
pub struct HistoryFetchRequest {
    pub coin_id: String,
    pub sender: oneshot::Sender<HistoryFetchResponse>,
}

impl HistoryFetchRequest {
    pub fn new_with_receiver(asset_id: String) -> (HistoryFetchRequest, oneshot::Receiver<HistoryFetchResponse>) {
        let (tx, rx) = oneshot::channel();
        let msg = HistoryFetchRequest {
            coin_id: asset_id,
            sender: tx
        };
        (msg, rx)
    }
}

pub type HistoryFetchResponse = Result<Arc<ClientFindByIdHistoryDataset>, ClientFindByIdHistoryError>;

#[derive(Snafu, Debug)]
pub enum ClientFindByIdHistoryError {
    #[snafu(display("Unknown instrument or not allowed."))]
    CoinUnknownOrNotAllowed,

    #[snafu(display("Failed to fetch coin dominance history."))]
    DbError,

    #[snafu(display("Failed to locate the service from the server context."))]
    FailedToLocateService,
}

#[derive(Serialize, Debug)]
pub struct ClientFindByIdHistoryEntry {
    #[serde(with = "ts_seconds")]
    pub tick: DateTime<Utc>,
    #[serde(with = "ts_seconds")]
    pub timestamp_original: DateTime<Utc>,
    pub provenance_uuid: Uuid,
    pub price: BigDecimal,
    pub price_original: BigDecimal,
}

#[derive(Serialize, Debug)]
pub struct ClientFindByIdHistoryEntrySlim<'a>(
    /// Timestamp original
    #[serde(with = "ts_seconds")]
    &'a DateTime<Utc>,

    /// Rounded price
    &'a BigDecimal
);

impl<'a> From<&'a ClientFindByIdHistoryEntry> for ClientFindByIdHistoryEntrySlim<'a> {
    fn from(x: &'a ClientFindByIdHistoryEntry) -> Self {
        ClientFindByIdHistoryEntrySlim(&x.tick, &x.price)
    }
}

type AssetId = FinancialDominanceAsset;

#[derive(Serialize, Clone, Debug)]
pub struct ClientFindByIdHistoryDataset {
    pub asset: FinancialAssetWithMetadataOfAny,
    pub rows: Arc<Vec<ClientFindByIdHistoryEntry>>,
}

#[derive(Serialize, Debug)]
pub struct ClientFindByIdHistoryDatasetSlim<'a> {
    pub asset: &'a FinancialAssetWithMetadataOfAny,
    pub rows: Vec<ClientFindByIdHistoryEntrySlim<'a>>,
}

impl<'a> From<&'a ClientFindByIdHistoryDataset> for ClientFindByIdHistoryDatasetSlim<'a> {
    fn from(x: &'a ClientFindByIdHistoryDataset) -> Self {
        ClientFindByIdHistoryDatasetSlim {
            asset: &x.asset,
            rows: x.rows.iter().map(|r| r.into()).collect(),
        }
    }
}

pub struct HistoricalCacheService {
    rx: mpsc::Receiver<HistoryFetchRequest>,
    pool: PgPool,
    cache: TtlCache<AssetId, Arc<ClientFindByIdHistoryDataset>>,
    default_ttl: Duration,
    update_interval: Duration
}

#[derive(Debug)]
enum HistoricalCoinMonitorMessage {
    ShouldUpdate(AssetId, oneshot::Sender<bool>),
    UpdatedDataset(AssetId, Arc<ClientFindByIdHistoryDataset>),
}

struct HistoricalCoinMonitor {
    asset_meta: FinancialAssetWithMetadata<FinancialDominanceAsset>,
    update_interval: Duration,
    pool: PgPool,
    parent: mpsc::Sender<HistoricalCoinMonitorMessage>,
}

impl HistoricalCoinMonitor {
    pub fn make_parent_channel(buffer_size: usize) -> (Sender<HistoricalCoinMonitorMessage>, tokio::sync::mpsc::Receiver<HistoricalCoinMonitorMessage>) {
        mpsc::channel(buffer_size)
    }

    pub async fn into_run(mut self) {
        info!("Started monitor for coin '{:?}' (update interval = {:?})", self.asset_meta.asset(), self.update_interval);
        let mut interval = tokio::time::interval(self.update_interval);
        loop {
            interval.tick().await;

            let (should_fetch_tx, should_fetch_rx) = oneshot::channel();
            if let Err(e) =
                self.parent.send(HistoricalCoinMonitorMessage::ShouldUpdate(
                    self.asset_meta.asset().clone(),
                    should_fetch_tx)).await {

                warn!("Failed to determine coin historical liveliness for '{:?}': {}", self.asset_meta.asset(), e);
                break;
            }

            match should_fetch_rx.await {
                Err(e) => {
                    warn!("Failed to determine coin historical liveliness for '{:?}': {}", self.asset_meta.asset(), e);
                    break;
                }
                Ok(false) => {
                    info!("Stopping monitoring for coin '{:?}'", self.asset_meta.asset());
                    break;
                },
                _ => {}
            };

            let result = fetch(&self.pool, &self.asset_meta).await;
            let dataset = match result {
                Err(e) => {
                    warn!("Failed to fetch coin historical data for '{:?}': {}", self.asset_meta.asset(), e);
                    continue;
                }
                Ok(x) => x
            };

            if let Err(e) = self.parent.send(HistoricalCoinMonitorMessage::UpdatedDataset(
                self.asset_meta.asset().clone(),
                dataset)).await
            {
                warn!("Failed to send coin historical data for '{:?}': {}", self.asset_meta.asset(), e);
                break;
            }
        }
    }
}

pub type HistoricalCacheServiceRef = mpsc::Sender<HistoryFetchRequest>;

impl HistoricalCacheService {
    pub fn new(pool: PgPool, buffer_size: usize, default_ttl: Duration, update_interval: Duration) -> (HistoricalCacheService, HistoricalCacheServiceRef) {
        let (tx, rx) = mpsc::channel(buffer_size);
        let ctx = HistoricalCacheService {
            rx,
            pool,
            cache: TtlCache::new(4096),
            default_ttl,
            update_interval,
        };

        (ctx, tx)
    }

    pub async fn into_run(mut self) {
        info!("Starting historical cache service");
        let (monitors_tx, mut monitors_rx) = HistoricalCoinMonitor::make_parent_channel(64);
        loop {
            select! {
                Some(msg) = self.rx.recv() => {
                    // debug!("recv: {:?}", msg);
                    self.handle_request(msg, &monitors_tx).await;
                }
                Some(msg) = monitors_rx.recv() => {
                    // debug!("recv: {:?}", msg);
                    match msg {
                        HistoricalCoinMonitorMessage::ShouldUpdate(coin_id, child) => {
                            child.send(self.cache.contains_key(&coin_id)).ok();
                        },
                        HistoricalCoinMonitorMessage::UpdatedDataset(coin_id, dataset) => {
                            match self.cache.entry(coin_id) {
                                ttl_cache::Entry::Occupied(mut entry) => {
                                    // Avoid updating the TTL
                                    *entry.get_mut() = dataset;
                                },
                                ttl_cache::Entry::Vacant(_) => {
                                    // Ignore message. Entry already expired
                                    continue
                                },
                            }
                        }
                    }
                }
            };
        }
    }

    async fn handle_request(&mut self, msg: HistoryFetchRequest, ctx: &mpsc::Sender<HistoricalCoinMonitorMessage>) {
        let msg_coin_id = msg.coin_id.to_ascii_lowercase();
        let asset_meta = match get_canonical_default_asset(msg_coin_id.as_str()) {
            None => {
                msg.sender.send(Err(ClientFindByIdHistoryError::CoinUnknownOrNotAllowed)).ok();
                return;
            },
            Some(x) => x
        };

        // Determine if we already have this in the cache
        let asset_id = asset_meta.asset();
        if let ttl_cache::Entry::Occupied(entry) = self.cache.entry(asset_id.clone()) {
            let dataset = entry.get();
            msg.sender.send(Ok(dataset.clone())).ok();

            // Refresh the entry TTL
            self.cache.remove(asset_id).map(|x|
                self.cache.insert(asset_id.clone(), x, self.default_ttl));

            return;
        }

        let result = fetch(&self.pool, asset_meta).await;
        if let Ok(dataset) = result.as_ref() {
            let monitor_ctx = HistoricalCoinMonitor {
                asset_meta: asset_meta.clone(),
                update_interval: self.update_interval.clone(),
                pool: self.pool.clone(),
                parent: ctx.clone(),
            };

            self.cache.insert(asset_id.clone(), dataset.clone(), self.default_ttl.clone());
            tokio::spawn(monitor_ctx.into_run());
        }
        msg.sender.send(result).ok();
    }
}


async fn fetch(
    pool: &PgPool,
    asset_meta: &FinancialAssetWithMetadata<FinancialDominanceAsset>
) -> Result<Arc<ClientFindByIdHistoryDataset>, ClientFindByIdHistoryError> {

    // Manually perform a slow fetch
    let db_result = CoinDominanceRepo::find_by_id_history(
        &asset_meta.asset(),
        pool)
        .await;

    let db_dataset = match db_result {
        Err(e @ RepositoryError::SqlError { .. }) => {
            error!("Failed to fetch history for coin `{:?}`: {}", asset_meta.asset(), e);
            return Err(ClientFindByIdHistoryError::DbError);
        }
        Ok(x) => x
    };

    let rows = db_dataset.rows.into_iter()
        .map(|r| {
            let value = r.dominance_percentage;
            ClientFindByIdHistoryEntry {
                tick: r.timestamp_utc_minutely,
                timestamp_original: r.timestamp_utc_exact,
                provenance_uuid: r.provenance_uuid,
                price: asset_meta.value_of(&value),
                price_original: asset_meta.raw_value_of(&value).into_owned(),
            }
        })
        .collect();

    let dataset = ClientFindByIdHistoryDataset {
        asset: asset_meta.clone().into_any(),
        rows: Arc::new(rows),
    };

    Ok(Arc::new(dataset))
}