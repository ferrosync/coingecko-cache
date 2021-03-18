use std::fmt::Formatter;
use serde::{Deserialize, Serialize, Deserializer, de};
use crate::models::TickerDisplay;
use std::str::FromStr;
use snafu::Snafu;
use crate::ext::bigdecimal::RoundingMode;

#[derive(Serialize, Eq, PartialEq, Copy, Clone, Hash, Debug)]
#[serde(rename_all = "lowercase")]
pub enum FinancialDominanceMode {
    Dom,
    AltDom
}

impl<'de> Deserialize<'de> for FinancialDominanceMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

impl FinancialDominanceMode {
    pub fn opposite(&self) -> FinancialDominanceMode {
        use super::FinancialDominanceMode::*;
        match self {
            Dom => AltDom,
            AltDom => Dom,
        }
    }

    pub fn rounding_mode(&self) -> RoundingMode {
        match self {
            FinancialDominanceMode::Dom => RoundingMode::HalfUp,
            FinancialDominanceMode::AltDom => RoundingMode::HalfUpOpposite,
        }
    }
}

#[derive(Snafu, Debug)]
pub enum FinancialDominanceParseError {
    #[snafu(display("Invalid dominance kind specified: '{}'", input))]
    InvalidFormat {
        input: String,
    }
}

impl FromStr for FinancialDominanceMode {
    type Err = FinancialDominanceParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase()
            .replace(|c: char| !c.is_ascii_alphanumeric(), "")
            .as_str()
        {
            "dom" => Ok(FinancialDominanceMode::Dom),
            "altdom" => Ok(FinancialDominanceMode::AltDom),
            _ => InvalidFormat { input: s.to_owned() }.fail()
        }
    }
}

impl TickerDisplay for FinancialDominanceMode {
    fn write_ticker_id(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FinancialDominanceMode::Dom => f.write_str("dom"),
            FinancialDominanceMode::AltDom => f.write_str("altdom"),
        }
    }

    fn write_ticker_display(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FinancialDominanceMode::Dom => f.write_str("DOM"),
            FinancialDominanceMode::AltDom => f.write_str("-ALTDOM"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use bigdecimal::BigDecimal;
    use crate::models::{FinancialDominanceMode, FinancialAssetValueOf};
    use crate::models::financial_assets::*;

    fn decimal(x: impl AsRef<str>) -> BigDecimal {
        BigDecimal::from_str(x.as_ref()).unwrap()
    }

    fn test_case_opposites_should_sum_total(dom: impl AsRef<str>) {
        let max = decimal("100");
        let dom = decimal(dom.as_ref());
        let altdom = &max - &dom;

        let dom_rounded = BTCDOM.value_of(&dom);
        let altdom_rounded = ALTDOM.value_of(&altdom);
        let sum = &dom_rounded + &altdom_rounded;
        assert_eq!(sum, max,
                   "Got {}, but expected {}: DOM ({} => {}), ALTDOM ({} => {})",
                   &sum, &max,
                   &dom, &dom_rounded,
                   &altdom, &altdom_rounded);
    }

    #[test]
    fn opposites_should_sum_total() {
        test_case_opposites_should_sum_total("12.432");
        test_case_opposites_should_sum_total("12.435");
        test_case_opposites_should_sum_total("12.437");
        test_case_opposites_should_sum_total("12.430");
    }

    #[test]
    fn should_deserialize() {
        let x: FinancialDominanceMode = serde_json::from_str(r#""alt___DOM""#).unwrap();
        assert_eq!(x, FinancialDominanceMode::AltDom);

        let x: FinancialDominanceMode = serde_json::from_str(r#""DOM""#).unwrap();
        assert_eq!(x, FinancialDominanceMode::Dom);
    }
}
