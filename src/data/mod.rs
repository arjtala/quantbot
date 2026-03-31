pub mod loader;

use anyhow::Result;
use chrono::NaiveDate;

use crate::core::bar::BarSeries;

pub trait DataProvider {
    fn get_bars(&self, symbol: &str, start: NaiveDate, end: NaiveDate) -> Result<BarSeries>;
}
