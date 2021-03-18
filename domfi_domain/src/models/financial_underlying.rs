use std::fmt::Formatter;
use serde::{Deserialize, Serialize};
use crate::models::ticker_display::TickerDisplay;
use crate::models::financial_symbol::FinancialSymbol;

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct FinancialUnderlying(FinancialSymbol);

impl From<FinancialSymbol> for FinancialUnderlying {
    fn from(symbol: FinancialSymbol) -> Self {
        Self(symbol)
    }
}

impl<'a> From<&'a FinancialSymbol> for FinancialUnderlying {
    fn from(symbol: &'a FinancialSymbol) -> Self {
        Self(symbol.to_owned())
    }
}

impl FinancialUnderlying {
    pub fn new(symbol: FinancialSymbol) -> FinancialUnderlying {
        Self(symbol)
    }
    pub fn symbol(&self) -> &FinancialSymbol {
        &self.0
    }
}

impl TickerDisplay for FinancialUnderlying {
    fn write_ticker_id(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.write_ticker_id(f)
    }

    fn write_ticker_display(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.write_ticker_display(f)
    }
}

pub mod defaults {
    use crate::models::{FinancialSymbol, FinancialUnderlying};

    lazy_static! {
        pub static ref BTC: FinancialUnderlying =
            FinancialSymbol::new("bitcoin", "BTC").into();

        pub static ref ETH: FinancialUnderlying =
            FinancialSymbol::new("ethereum", "ETH").into();

        pub static ref BNB: FinancialUnderlying =
            FinancialSymbol::new("binancecoin", "BNB").into();

        pub static ref USDT: FinancialUnderlying =
            FinancialSymbol::new("tether", "USDT").into();

        pub static ref DOT: FinancialUnderlying =
            FinancialSymbol::new("polkadot", "DOT").into();

        pub static ref XRP: FinancialUnderlying =
            FinancialSymbol::new("ripple", "XRP").into();

        pub static ref LTC: FinancialUnderlying =
            FinancialSymbol::new("litecoin", "LTC").into();

        pub static ref LINK: FinancialUnderlying =
            FinancialSymbol::new("chainlink", "LINK").into();

        pub static ref BCH: FinancialUnderlying =
            FinancialSymbol::new("bitcoin-cash", "BCH").into();

        pub static ref BSV: FinancialUnderlying =
            FinancialSymbol::new("bitcoin-cash-sv", "BSV").into();
    }
}
