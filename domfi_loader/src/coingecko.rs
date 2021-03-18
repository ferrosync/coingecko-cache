use chrono::{DateTime, Utc};
use chrono::serde::ts_seconds;
use bigdecimal::BigDecimal;
use serde::Deserialize;
use serde_with::serde_as;
use domfi_ext_serde::BigDecimalExact;

#[derive(Deserialize, Debug)]
pub struct CoinDominanceResponse {
    pub data: Vec<CoinDominance>,

    #[serde(with = "ts_seconds")]
    pub timestamp: DateTime<Utc>,
}

#[serde_as]
#[derive(Deserialize, Debug)]
pub struct CoinDominance {
    pub name: String,
    pub id: String,
    #[serde_as(as = "BigDecimalExact")]
    pub market_cap_usd: BigDecimal,
    #[serde_as(as = "BigDecimalExact")]
    pub dominance_percentage: BigDecimal,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn coin_dominance_should_parse_decimals_exactly() {
        let test = r#"{
                "name":"LTC",
                "id":"litecoin",
                "market_cap_usd":8364520669.848436,
                "dominance_percentage":1.1188873746973713
            }"#;

        let buf = test.as_bytes();
        let json = serde_json::from_slice::<CoinDominance>(buf).unwrap();

        assert_eq!(json.market_cap_usd, BigDecimal::from_str("8364520669.848436").unwrap());
        assert_eq!(json.dominance_percentage, BigDecimal::from_str("1.1188873746973713").unwrap());
    }
}
