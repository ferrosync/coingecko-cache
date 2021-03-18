use std::fmt::{Write, Formatter};
use serde::{Deserialize, Serialize};
use crate::models::{FinancialAsset, FinancialDominanceMode, FinancialUnderlying, FinancialAssetLike, TickerDisplay};

#[derive(Serialize, Deserialize, Clone, Hash, Eq, PartialEq, Debug)]
pub struct FinancialDominanceAsset {
    #[serde(rename = "symbol")]
    underlying: FinancialUnderlying,
    mode: FinancialDominanceMode,
}

impl FinancialAssetLike for FinancialDominanceAsset { }

impl FinancialDominanceAsset {
    pub fn new(
        underlying: FinancialUnderlying,
        mode: FinancialDominanceMode,
    ) -> Self {
        FinancialDominanceAsset { underlying, mode, }
    }

    pub fn opposite(&self) -> FinancialDominanceAsset {
        let next_mode = self.mode().opposite();
        Self::new(
            self.underlying().clone(),
            next_mode)
    }

    pub fn underlying(&self) -> &FinancialUnderlying { &self.underlying }
    pub fn mode(&self) -> FinancialDominanceMode { self.mode }
}

impl From<FinancialDominanceAsset> for FinancialAsset {
    fn from(inner: FinancialDominanceAsset) -> Self {
        FinancialAsset::Dominance { inner }
    }
}

impl<'a> From<&'a FinancialDominanceAsset> for FinancialAsset {
    fn from(inner: &'a FinancialDominanceAsset) -> Self {
        FinancialAsset::Dominance { inner: inner.clone() }
    }
}

const FINANCIAL_ASSET_TICKER_SEP: char = '^';

impl TickerDisplay for FinancialDominanceAsset {
    fn write_ticker_id(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.underlying().write_ticker_id(f)?;
        f.write_char(FINANCIAL_ASSET_TICKER_SEP)?;
        self.mode().write_ticker_id(f)?;
        Ok(())
    }

    fn write_ticker_display(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.underlying().write_ticker_display(f)?;

        // Will already have separator if necessary
        self.mode().write_ticker_display(f)?;
        Ok(())
    }
}

pub mod defaults {
    use std::collections::HashMap;
    use crate::ext::bigdecimal::RoundingMode;
    use crate::models::*;
    use crate::models::FinancialDominanceMode::*;
    use crate::models::financial_underlying::defaults::*;
    use crate::models::financial_asset::FinancialAssetMetadata;

    fn dom(underlying: FinancialUnderlying) -> FinancialAssetWithMetadata<FinancialDominanceAsset> {
        FinancialAssetWithMetadata::new(
            FinancialDominanceAsset::new(underlying, Dom),
            FinancialAssetMetadata::new(
                FinancialAssetRounding::new(2, RoundingMode::HalfUp)))
    }

    fn altdom(dom: FinancialAssetWithMetadata<FinancialDominanceAsset>) -> FinancialAssetWithMetadata<FinancialDominanceAsset> {
        FinancialAssetWithMetadata::new(
            dom.asset().opposite(),
            FinancialAssetMetadata::new(
                dom.metadata().rounding().with_mode(RoundingMode::HalfUpOpposite)))
    }

    lazy_static! {
        pub static ref BTCDOM: FinancialAssetWithMetadata<FinancialDominanceAsset> =
            dom(BTC.clone());

        pub static ref ALTDOM: FinancialAssetWithMetadata<FinancialDominanceAsset> =
            altdom(BTCDOM.clone());

        //

        pub static ref ETHDOM: FinancialAssetWithMetadata<FinancialDominanceAsset> =
            dom(ETH.clone());

        pub static ref BNBDOM: FinancialAssetWithMetadata<FinancialDominanceAsset> =
            dom(BNB.clone());

        pub static ref USDTDOM: FinancialAssetWithMetadata<FinancialDominanceAsset> =
            dom(USDT.clone());

        pub static ref DOTDOM: FinancialAssetWithMetadata<FinancialDominanceAsset> =
            dom(DOT.clone());

        pub static ref XRPDOM: FinancialAssetWithMetadata<FinancialDominanceAsset> =
            dom(XRP.clone());
                
        pub static ref LTCDOM: FinancialAssetWithMetadata<FinancialDominanceAsset> =
            dom(LTC.clone());

        pub static ref LINKDOM: FinancialAssetWithMetadata<FinancialDominanceAsset> =
            dom(LINK.clone());

        pub static ref BCHDOM: FinancialAssetWithMetadata<FinancialDominanceAsset> =
            dom(BCH.clone());

        pub static ref BSVDOM: FinancialAssetWithMetadata<FinancialDominanceAsset> =
            dom(BSV.clone());

        pub static ref CANONICAL_DEFAULT_ASSETS: HashMap<&'static str, FinancialAssetWithMetadata<FinancialDominanceAsset>> = {
            let mut x = HashMap::new();
            x.insert("btcdom", (&*BTCDOM).clone());
            x.insert("altdom", (&*ALTDOM).clone());
            //
            x.insert("ethdom", (&*ETHDOM).clone());
            x.insert("bnbdom", (&*BNBDOM).clone());
            x.insert("usdtdom", (&*USDTDOM).clone());
            x.insert("dotdom", (&*DOTDOM).clone());
            x.insert("xrpdom", (&*XRPDOM).clone());
            x.insert("ltcdom", (&*LTCDOM).clone());
            x.insert("linkdom", (&*LINKDOM).clone());
            x.insert("bchdom", (&*BCHDOM).clone());
            x.insert("bsvdom", (&*BSVDOM).clone());
            x
        };

        pub static ref ASSET_METADATA_LOOKUP: HashMap<FinancialDominanceAsset, &'static FinancialAssetMetadata> = {
            let mut x = HashMap::new();
            for asset_meta in (&*CANONICAL_DEFAULT_ASSETS).values() {
                x.insert(asset_meta.asset().clone(), asset_meta.metadata());
            }
            x
        };
    }

    pub fn get_canonical_default_asset(id: impl AsRef<str>) -> Option<&'static FinancialAssetWithMetadata<FinancialDominanceAsset>> {
        let normalized_id = id.as_ref().to_ascii_lowercase();
        CANONICAL_DEFAULT_ASSETS.get(normalized_id.as_str())
    }
}
