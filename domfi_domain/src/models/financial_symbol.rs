use std::fmt::Formatter;
use serde::{Deserialize, Serialize};
use derivative::{Derivative};
use crate::models::ticker_display::TickerDisplay;

#[derive(Derivative)]
#[derive(Serialize, Deserialize, Clone, Debug, Eq)]
#[derivative(Hash, PartialEq)]
pub struct FinancialSymbol {
    id: String,

    #[serde(default)]
    #[derivative(PartialEq="ignore")]
    #[derivative(Hash="ignore")]
    symbol: String,
}

#[cfg(test)]
mod tests {
    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;
    use super::*;

    fn hash<T: Hash>(t: &T) -> u64 {
        let mut s = DefaultHasher::new();
        t.hash(&mut s);
        s.finish()
    }

    #[test]
    fn financial_symbol_should_only_eq_hash_id() {
        let a = FinancialSymbol::new("XYZ", "FOOBAR");
        let b = FinancialSymbol::new("XYZ", "SNAFU");
        assert_eq!(a, b);
        assert_eq!(hash(&a), hash(&b))
    }
}

pub mod defaults {
    use crate::models::FinancialSymbol;

    lazy_static! {
        pub static ref BTC: FinancialSymbol = FinancialSymbol {
            id: "bitcoin".to_string(),
            symbol: "BTC".to_string(),
        };
    }
}

impl FinancialSymbol {
    pub fn new(id: impl AsRef<str>, symbol: impl AsRef<str>) -> FinancialSymbol {
        FinancialSymbol {
            id: normalize_symbol_id(id),
            symbol: symbol.as_ref().to_owned(),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn symbol(&self) -> &str {
        &self.symbol
    }
}

fn normalize_symbol_id(s: impl AsRef<str>) -> String {
    s.as_ref().to_owned()
        .replace(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'), "")
        // .replace(|c: char| c == '-', "_")
}

impl TickerDisplay for FinancialSymbol {
    fn write_ticker_id(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.id)
    }

    fn write_ticker_display(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.symbol.to_uppercase())
    }
}
