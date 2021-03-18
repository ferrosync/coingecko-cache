use std::fmt::{Formatter, Display};

pub trait TickerDisplay: Sized {
    fn write_ticker_id(&self, f: &mut Formatter<'_>) -> std::fmt::Result;
    fn write_ticker_display(&self, f: &mut Formatter<'_>) -> std::fmt::Result;

    fn ticker_id(&self) -> TickerIdDisplay<Self> {
        TickerIdDisplay(self)
    }

    fn to_ticker_id(&self) -> String {
        self.ticker_id().to_string()
    }

    fn ticker_display(&self) -> TickerFriendlyDisplay<Self> {
        TickerFriendlyDisplay(self)
    }

    fn to_ticker_display(&self) -> String {
        self.ticker_display().to_string()
    }
}

pub struct TickerIdDisplay<'a, T: 'a + TickerDisplay>(&'a T);
pub struct TickerFriendlyDisplay<'a, T: 'a + TickerDisplay>(&'a T);

impl<'a, T: 'a + TickerDisplay> Display for TickerIdDisplay<'a, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.write_ticker_id(f)
    }
}

impl<'a, T: 'a + TickerDisplay> Display for TickerFriendlyDisplay<'a, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.write_ticker_display(f)
    }
}
