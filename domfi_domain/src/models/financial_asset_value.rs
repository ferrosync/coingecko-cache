use std::fmt::Formatter;
use std::borrow::Cow;
use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize};

use crate::models::financial_asset::{FinancialAsset, FinancialAssetRawValueOf};
use crate::models::{FinancialAssetRounding, TickerDisplay};
use crate::ext::bigdecimal::RoundExt;

#[derive(Serialize, Deserialize)]
pub struct FinancialAssetValue {
    asset: FinancialAsset,
    #[serde(rename = "value")]
    value_original: BigDecimal,
}

impl FinancialAssetValue {
    pub fn new(value: BigDecimal, asset: FinancialAsset) -> FinancialAssetValue {
        FinancialAssetValue {
            value_original: value,
            asset,
        }
    }

    pub fn asset(&self) -> &FinancialAsset {
        &self.asset
    }

    pub fn value(&self, rounding: FinancialAssetRounding) -> BigDecimal {
        self.asset
            .raw_value_of(&self.value_original)
            .with_rounding(
                rounding.digits(),
                rounding.mode())
    }

    pub fn value_original(&self) -> Cow<BigDecimal> {
        self.asset.raw_value_of(&self.value_original)
    }
}

impl TickerDisplay for FinancialAssetValue {
    fn write_ticker_id(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.asset.write_ticker_id(f)
    }

    fn write_ticker_display(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.asset.write_ticker_display(f)
    }
}

#[cfg(test)]
mod tests {
    use bigdecimal::BigDecimal;
    use std::str::FromStr;
    use crate::models::financial_assets::BTCDOM;
    use crate::models::*;

    #[test]
    fn should_serialize_dominance_asset_value_to_json() {
        let x = FinancialAssetValue::new(
            BigDecimal::from_str("64.3322098302").unwrap(),
            BTCDOM.asset().into(),
        );
        let json = serde_json::to_string(&x).unwrap();

        assert_eq!(
            json,
            r#"{"asset":{"kind":"dominance","symbol":{"id":"bitcoin","symbol":"BTC"},"mode":"dom"},"value":"64.3322098302"}"#
        );
    }
}
