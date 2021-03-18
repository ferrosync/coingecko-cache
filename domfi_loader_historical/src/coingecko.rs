use chrono::{DateTime, Utc};
use chrono::serde::{ts_milliseconds};
use bigdecimal::BigDecimal;

use serde::{Deserialize};
use serde_with::serde_as;
use domfi_ext_serde::BigDecimalExact;

#[serde_as]
#[derive(Deserialize, Debug)]
pub struct Series {
    pub name: String,
    pub data: Vec<SeriesEntry>,
}

#[serde_as]
#[derive(Deserialize, Debug)]
pub struct SeriesEntry(
    #[serde(with = "ts_milliseconds")]
    pub DateTime<Utc>,
    #[serde_as(as = "Option<BigDecimalExact>")]
    pub Option<BigDecimal>,
);

#[serde_as]
#[derive(Deserialize, Debug)]
pub struct MarketDominanceData {
    pub series_data_array: Vec<Series>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use chrono::TimeZone;

    #[test]
    fn series_entry_should_parse_decimals_exactly() {
        let n = "1.1234679123479120374890123740981237498";
        let test = format!("[1609339955000, {}]", n);

        let dt = Utc
            .ymd(2020, 12, 30)
            .and_hms(14, 52, 35);

        let buf = test.as_bytes();
        let json = serde_json::from_slice::<SeriesEntry>(buf).unwrap();

        assert_eq!(json.0, dt);
        assert_eq!(json.1, Some(BigDecimal::from_str(n).unwrap()));
    }
}