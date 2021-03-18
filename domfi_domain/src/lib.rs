#[macro_use]
extern crate lazy_static;

pub mod models;
pub mod ext;

use bigdecimal::BigDecimal;

pub fn round_price_identifier(value: &BigDecimal) -> BigDecimal {
    value.round(2).with_scale(2)
}

#[cfg(test)]
mod tests {
    use crate::round_price_identifier;
    use bigdecimal::BigDecimal;
    use std::str::FromStr;

    #[test]
    fn rounds_correctly() {
        let input = BigDecimal::from_str("53.357").unwrap();
        let expected = BigDecimal::from_str("53.36").unwrap();
        assert_eq!(round_price_identifier(&input), expected);
    }
}
