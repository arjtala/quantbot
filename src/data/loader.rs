use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::NaiveDate;
use serde::Deserialize;

use crate::core::bar::{Bar, BarSeries};
use crate::data::DataProvider;

/// Raw row from Yahoo Finance CSV export.
/// Column order: Date, Close, High, Low, Open, Volume
#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct CsvRow {
    Date: String,
    Close: f64,
    High: f64,
    Low: f64,
    Open: f64,
    Volume: f64,
}

/// Loads bar data from CSV files in a directory.
/// Expects files named `{symbol}.csv` (e.g. `SPY.csv`, `GC=F.csv`).
pub struct CsvLoader {
    data_dir: PathBuf,
}

impl CsvLoader {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
        }
    }

    fn csv_path(&self, symbol: &str) -> PathBuf {
        self.data_dir.join(format!("{symbol}.csv"))
    }

    /// Load all bars from a CSV file, optionally filtered by date range.
    pub fn load_bars(
        &self,
        symbol: &str,
        start: Option<NaiveDate>,
        end: Option<NaiveDate>,
    ) -> Result<BarSeries> {
        let path = self.csv_path(symbol);
        Self::load_from_path(&path, start, end)
    }

    fn load_from_path(
        path: &Path,
        start: Option<NaiveDate>,
        end: Option<NaiveDate>,
    ) -> Result<BarSeries> {
        let mut reader = csv::Reader::from_path(path)
            .with_context(|| format!("failed to open {}", path.display()))?;

        let mut bars = Vec::new();
        for result in reader.deserialize() {
            let row: CsvRow =
                result.with_context(|| format!("failed to parse row in {}", path.display()))?;

            let date = NaiveDate::parse_from_str(&row.Date, "%Y-%m-%d")
                .with_context(|| format!("invalid date '{}' in {}", row.Date, path.display()))?;

            if let Some(s) = start {
                if date < s {
                    continue;
                }
            }
            if let Some(e) = end {
                if date > e {
                    continue;
                }
            }

            bars.push(Bar {
                date,
                open: row.Open,
                high: row.High,
                low: row.Low,
                close: row.Close,
                volume: row.Volume,
            });
        }

        // CSV should already be sorted, but ensure it
        bars.sort_by_key(|b| b.date);

        BarSeries::new(bars).with_context(|| format!("invalid bar series from {}", path.display()))
    }
}

impl DataProvider for CsvLoader {
    fn get_bars(&self, symbol: &str, start: NaiveDate, end: NaiveDate) -> Result<BarSeries> {
        self.load_bars(symbol, Some(start), Some(end))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn write_test_csv(dir: &Path, symbol: &str, content: &str) {
        let path = dir.join(format!("{symbol}.csv"));
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    const TEST_CSV: &str = "\
Date,Close,High,Low,Open,Volume
2025-01-02,100.5,101.0,99.0,100.0,50000
2025-01-03,102.0,103.0,100.0,100.5,60000
2025-01-06,99.0,102.5,98.5,102.0,45000
2025-01-07,101.0,101.5,98.0,99.0,55000
";

    #[test]
    fn load_all_bars() {
        let dir = TempDir::new().unwrap();
        write_test_csv(dir.path(), "TEST", TEST_CSV);

        let loader = CsvLoader::new(dir.path());
        let series = loader.load_bars("TEST", None, None).unwrap();
        assert_eq!(series.len(), 4);

        let bars = series.bars();
        assert!((bars[0].open - 100.0).abs() < 1e-10);
        assert!((bars[0].close - 100.5).abs() < 1e-10);
        assert!((bars[0].high - 101.0).abs() < 1e-10);
        assert!((bars[0].low - 99.0).abs() < 1e-10);
    }

    #[test]
    fn load_with_date_range() {
        let dir = TempDir::new().unwrap();
        write_test_csv(dir.path(), "TEST", TEST_CSV);

        let loader = CsvLoader::new(dir.path());
        let start = NaiveDate::from_ymd_opt(2025, 1, 3).unwrap();
        let end = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let series = loader.load_bars("TEST", Some(start), Some(end)).unwrap();
        assert_eq!(series.len(), 2);
        assert_eq!(series.bars()[0].date, start);
        assert_eq!(series.bars()[1].date, end);
    }

    #[test]
    fn data_provider_trait() {
        let dir = TempDir::new().unwrap();
        write_test_csv(dir.path(), "SPY", TEST_CSV);

        let loader = CsvLoader::new(dir.path());
        let provider: &dyn DataProvider = &loader;
        let start = NaiveDate::from_ymd_opt(2025, 1, 2).unwrap();
        let end = NaiveDate::from_ymd_opt(2025, 1, 7).unwrap();
        let series = provider.get_bars("SPY", start, end).unwrap();
        assert_eq!(series.len(), 4);
    }

    #[test]
    fn missing_file_gives_error() {
        let dir = TempDir::new().unwrap();
        let loader = CsvLoader::new(dir.path());
        assert!(loader.load_bars("NOSUCH", None, None).is_err());
    }
}
