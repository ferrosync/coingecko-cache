use std::fmt::{Formatter, Debug};
use serde::{Deserialize, Serialize};
use bigdecimal::BigDecimal;
use crate::ext::bigdecimal::{RoundingMode, RoundExt};
use crate::models::{FinancialUnderlying, FinancialDominanceAsset};
use crate::models::FinancialDominanceMode;
use crate::models::TickerDisplay;
use std::borrow::{Cow, Borrow};

pub trait FinancialAssetLike: Sized + Clone + TickerDisplay + Debug {
    fn with_metadata(&self, metadata: &FinancialAssetMetadata) -> FinancialAssetWithMetadata<Self> {
        FinancialAssetWithMetadata::new(self.clone(), metadata.clone())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Hash, Eq, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum FinancialAsset {
    Base {
        symbol: FinancialUnderlying
    },
    Dominance {
        #[serde(flatten)]
        inner: FinancialDominanceAsset
    },
}

impl FinancialAssetLike for FinancialAsset { }

#[derive(Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
pub struct FinancialAssetWithMetadata<T: FinancialAssetLike + Sized + Debug> {
    asset: T,
    metadata: FinancialAssetMetadata
}

impl<T: FinancialAssetLike + Sized + Debug> FinancialAssetWithMetadata<T> {
    pub fn into_any(self) -> FinancialAssetWithMetadataOfAny
    where T: Into<FinancialAsset>
    {
        FinancialAssetWithMetadataOfAny::new(self.asset.into(), self.metadata)
    }
}

pub type FinancialAssetWithMetadataOfAny = FinancialAssetWithMetadata<FinancialAsset>;

pub trait FinancialAssetValueOf<'a> {
    fn value_of(&self, value: &'a BigDecimal) -> BigDecimal;
}

pub trait FinancialAssetRawValueOf<'a> {
    fn raw_value_of(&self, value: &'a BigDecimal) -> Cow<'a, BigDecimal>;
}

impl<T: FinancialAssetLike + Debug> FinancialAssetWithMetadata<T> {
    pub fn new(asset: T, metadata: FinancialAssetMetadata) -> Self {
        FinancialAssetWithMetadata { asset, metadata }
    }

    pub fn asset(&self) -> &T {
        &self.asset
    }

    pub fn metadata(&self) -> &FinancialAssetMetadata {
        &self.metadata
    }
}

lazy_static! {
    static ref BN_100: BigDecimal = BigDecimal::from(100);
}
//
// impl<T: FinancialAssetLike + Debug> FinancialAssetValueOf for FinancialAssetWithMetadata<T> {
//     fn value_of(&self, value: &BigDecimal) -> BigDecimal {
//         self.metadata.round(value)
//     }
// }

impl<'a> FinancialAssetRawValueOf<'a> for FinancialDominanceAsset {
    fn raw_value_of(&self, value: &'a BigDecimal) -> Cow<'a, BigDecimal> {
        if self.mode() == FinancialDominanceMode::AltDom {
            Cow::Owned(&*BN_100 - value)
        } else {
            Cow::Borrowed(value)
        }
    }
}

impl<'a> FinancialAssetRawValueOf<'a> for FinancialUnderlying {
    fn raw_value_of(&self, value: &'a BigDecimal) -> Cow<'a, BigDecimal> {
        Cow::Borrowed(value)
    }
}

impl<'a> FinancialAssetRawValueOf<'a> for FinancialAsset {
    fn raw_value_of(&self, value: &'a BigDecimal) -> Cow<'a, BigDecimal> {
        match self {
            FinancialAsset::Base { symbol } => {
                symbol.raw_value_of(value)
            }
            FinancialAsset::Dominance { inner } => {
                inner.raw_value_of(value)
            }
        }
    }
}

impl<'a, T> FinancialAssetValueOf<'a> for FinancialAssetWithMetadata<T>
where
    T: 'a + FinancialAssetLike + FinancialAssetRawValueOf<'a>
{
    fn value_of(&self, value: &'a BigDecimal) -> BigDecimal {
        let value = self.asset.raw_value_of(value);
        self.metadata.round(value.borrow())
    }
}

impl<'a, T> FinancialAssetRawValueOf<'a> for FinancialAssetWithMetadata<T>
    where
        T: 'a + FinancialAssetLike + FinancialAssetRawValueOf<'a>
{
    fn raw_value_of(&self, value: &'a BigDecimal) -> Cow<'a, BigDecimal> {
        self.asset.raw_value_of(value)
    }
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
pub struct FinancialAssetMetadata {
    rounding: FinancialAssetRounding,
}

impl FinancialAssetMetadata {
    pub fn new(rounding: FinancialAssetRounding) -> Self {
        FinancialAssetMetadata { rounding }
    }

    pub fn rounding(&self) -> FinancialAssetRounding {
        self.rounding
    }

    pub fn round(&self, value: &BigDecimal) -> BigDecimal {
        self.rounding.round(value)
    }
}


#[derive(Serialize, Deserialize, Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct FinancialAssetRounding {
    digits: i64,
    mode: RoundingMode,
}

impl FinancialAssetRounding {
    pub fn new(digits: i64, mode: RoundingMode) -> Self {
        FinancialAssetRounding { digits, mode }
    }

    pub fn round(&self, value: &BigDecimal) -> BigDecimal {
        value.with_rounding(self.digits, self.mode)
    }

    pub fn with_mode(&self, mode: RoundingMode) -> Self {
        let mut copy = self.clone();
        copy.mode = mode;
        copy
    }

    pub fn digits(&self) -> i64 { self.digits }
    pub fn mode(&self) -> RoundingMode { self.mode }
}

impl FinancialAsset {
    pub fn base(symbol: FinancialUnderlying) -> FinancialAsset {
        FinancialAsset::Base { symbol }
    }

    pub fn dominance(underlying: FinancialUnderlying, kind: FinancialDominanceMode) -> FinancialAsset {
        let inner = FinancialDominanceAsset::new(underlying, kind);
        FinancialAsset::Dominance { inner }
    }

    pub fn with_metadata(&self, metadata: FinancialAssetMetadata) -> FinancialAssetWithMetadata<Self> {
        FinancialAssetWithMetadata::new(self.clone(), metadata)
    }

    pub fn as_dominance(&self) -> Option<&FinancialDominanceAsset> {
        match self {
            FinancialAsset::Base { .. } => None,
            FinancialAsset::Dominance { inner } => Some(inner),
        }
    }
}

impl TickerDisplay for FinancialAsset {
    fn write_ticker_id(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FinancialAsset::Base { symbol } => {
                symbol.write_ticker_id(f)
            },
            FinancialAsset::Dominance { inner } => {
                inner.write_ticker_id(f)
            }
        }
    }

    fn write_ticker_display(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FinancialAsset::Base { symbol } => {
                symbol.write_ticker_display(f)
            },
            FinancialAsset::Dominance { inner } => {
                inner.write_ticker_display(f)
            }
        }
    }
}
