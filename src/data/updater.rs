use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::NaiveDate;

use crate::core::bar::Bar;
use crate::data::loader::CsvLoader;
use crate::data::yahoo::YahooClient;

/// Result of updating one symbol's CSV data.
#[derive(Debug)]
pub struct UpdateResult {
    pub symbol: String,
    pub bars_fetched: usize,
    pub bars_appended: usize,
    pub total_bars: usize,
    pub error: Option<String>,
}

/// Manages CSV data files — reads existing data, fetches new bars, and appends.
pub struct DataUpdater {
    data_dir: PathBuf,
}

impl DataUpdater {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
        }
    }

    fn csv_path(&self, symbol: &str) -> PathBuf {
        self.data_dir.join(format!("{symbol}.csv"))
    }

    /// Return the last date present in the CSV for `symbol`, or None if file missing.
    pub fn last_date(&self, symbol: &str) -> Result<Option<NaiveDate>> {
        let path = self.csv_path(symbol);
        if !path.exists() {
            return Ok(None);
        }

        let loader = CsvLoader::new(&self.data_dir);
        let series = loader
            .load_bars(symbol, None, None)
            .with_context(|| format!("failed to read existing CSV for {symbol}"))?;

        Ok(series.bars().last().map(|b| b.date))
    }

    /// Merge new bars into the CSV file, appending only bars after the last existing date.
    /// Creates the file with header if it doesn't exist.
    /// Returns the number of bars appended and total bar count.
    pub fn merge_bars(&self, symbol: &str, new_bars: &[Bar]) -> Result<UpdateResult> {
        let path = self.csv_path(symbol);
        let existing_last = self.last_date(symbol)?;

        // Filter new bars to only those after existing data
        let to_append: Vec<&Bar> = match existing_last {
            Some(last) => new_bars.iter().filter(|b| b.date > last).collect(),
            None => new_bars.iter().collect(),
        };

        let bars_appended = to_append.len();

        if path.exists() && !to_append.is_empty() {
            // Append to existing file
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .with_context(|| format!("failed to open {} for append", path.display()))?;

            for bar in &to_append {
                write_bar_row(&mut file, bar)?;
            }
        } else if !path.exists() {
            // Create new file with header
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut file = std::fs::File::create(&path)
                .with_context(|| format!("failed to create {}", path.display()))?;

            writeln!(file, "Date,Close,High,Low,Open,Volume")?;
            for bar in &to_append {
                write_bar_row(&mut file, bar)?;
            }
        }

        // Count total bars
        let total_bars = if path.exists() {
            let loader = CsvLoader::new(&self.data_dir);
            loader
                .load_bars(symbol, None, None)
                .map(|s| s.len())
                .unwrap_or(0)
        } else {
            0
        };

        Ok(UpdateResult {
            symbol: symbol.to_string(),
            bars_fetched: new_bars.len(),
            bars_appended,
            total_bars,
            error: None,
        })
    }

    /// Update all symbols: fetch from Yahoo and merge into CSVs.
    /// Continues on individual failures.
    pub async fn update_all(
        &self,
        client: &mut YahooClient,
        symbols: &[String],
        to: NaiveDate,
    ) -> Vec<UpdateResult> {
        let mut results = Vec::new();

        for sym in symbols {
            // Fetch from the day after last existing bar (or 2021-01-01 if new)
            let from = match self.last_date(sym) {
                Ok(Some(last)) => last.succ_opt().unwrap_or(last),
                Ok(None) => NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
                Err(e) => {
                    results.push(UpdateResult {
                        symbol: sym.clone(),
                        bars_fetched: 0,
                        bars_appended: 0,
                        total_bars: 0,
                        error: Some(format!("failed to read existing data: {e}")),
                    });
                    continue;
                }
            };

            if from > to {
                // Already up to date
                let total = self
                    .last_date(sym)
                    .ok()
                    .flatten()
                    .map(|_| {
                        let loader = CsvLoader::new(&self.data_dir);
                        loader
                            .load_bars(sym, None, None)
                            .map(|s| s.len())
                            .unwrap_or(0)
                    })
                    .unwrap_or(0);

                results.push(UpdateResult {
                    symbol: sym.clone(),
                    bars_fetched: 0,
                    bars_appended: 0,
                    total_bars: total,
                    error: None,
                });
                continue;
            }

            match client.fetch_daily_bars(sym, from, to).await {
                Ok(bars) => {
                    let fetched = bars.len();
                    match self.merge_bars(sym, &bars) {
                        Ok(mut result) => {
                            result.bars_fetched = fetched;
                            results.push(result);
                        }
                        Err(e) => {
                            results.push(UpdateResult {
                                symbol: sym.clone(),
                                bars_fetched: fetched,
                                bars_appended: 0,
                                total_bars: 0,
                                error: Some(format!("merge failed: {e}")),
                            });
                        }
                    }
                }
                Err(e) => {
                    results.push(UpdateResult {
                        symbol: sym.clone(),
                        bars_fetched: 0,
                        bars_appended: 0,
                        total_bars: 0,
                        error: Some(format!("fetch failed: {e}")),
                    });
                }
            }
        }

        results
    }

    /// Discover symbols from existing CSV files in the data directory.
    pub fn discover_symbols(&self) -> Result<Vec<String>> {
        let mut symbols = Vec::new();
        if !self.data_dir.is_dir() {
            return Ok(symbols);
        }
        for entry in std::fs::read_dir(&self.data_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "csv") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    symbols.push(stem.to_string());
                }
            }
        }
        symbols.sort();
        Ok(symbols)
    }
}

/// Write a single bar row in the expected CSV column order.
fn write_bar_row(file: &mut impl Write, bar: &Bar) -> Result<()> {
    writeln!(
        file,
        "{},{},{},{},{},{}",
        bar.date, bar.close, bar.high, bar.low, bar.open, bar.volume
    )
    .context("failed to write bar row")?;
    Ok(())
}

/// Format a path for display (used in CLI output).
pub fn csv_path_for(data_dir: &Path, symbol: &str) -> PathBuf {
    data_dir.join(format!("{symbol}.csv"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn bar(date: &str, open: f64, high: f64, low: f64, close: f64, vol: f64) -> Bar {
        Bar {
            date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            open,
            high,
            low,
            close,
            volume: vol,
        }
    }

    fn write_csv(dir: &Path, symbol: &str, content: &str) {
        let path = dir.join(format!("{symbol}.csv"));
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn last_date_from_existing_csv() {
        let dir = TempDir::new().unwrap();
        write_csv(
            dir.path(),
            "SPY",
            "Date,Close,High,Low,Open,Volume\n\
             2025-01-02,100.5,101.0,99.0,100.0,50000\n\
             2025-01-03,102.0,103.0,100.0,100.5,60000\n",
        );

        let updater = DataUpdater::new(dir.path());
        let last = updater.last_date("SPY").unwrap();
        assert_eq!(last, Some(NaiveDate::from_ymd_opt(2025, 1, 3).unwrap()));
    }

    #[test]
    fn last_date_missing_file() {
        let dir = TempDir::new().unwrap();
        let updater = DataUpdater::new(dir.path());
        assert_eq!(updater.last_date("NOSUCH").unwrap(), None);
    }

    #[test]
    fn merge_creates_new_file() {
        let dir = TempDir::new().unwrap();
        let updater = DataUpdater::new(dir.path());

        let bars = vec![
            bar("2025-01-02", 100.0, 101.0, 99.0, 100.5, 50000.0),
            bar("2025-01-03", 100.5, 103.0, 100.0, 102.0, 60000.0),
        ];

        let result = updater.merge_bars("NEW", &bars).unwrap();
        assert_eq!(result.bars_appended, 2);
        assert_eq!(result.total_bars, 2);

        // Verify CSV can be loaded back
        let loader = CsvLoader::new(dir.path());
        let series = loader.load_bars("NEW", None, None).unwrap();
        assert_eq!(series.len(), 2);
    }

    #[test]
    fn merge_appends_only_new_bars() {
        let dir = TempDir::new().unwrap();
        write_csv(
            dir.path(),
            "SPY",
            "Date,Close,High,Low,Open,Volume\n\
             2025-01-02,100.5,101.0,99.0,100.0,50000\n\
             2025-01-03,102.0,103.0,100.0,100.5,60000\n",
        );

        let updater = DataUpdater::new(dir.path());
        let new_bars = vec![
            bar("2025-01-02", 100.0, 101.0, 99.0, 100.5, 50000.0), // duplicate
            bar("2025-01-03", 100.5, 103.0, 100.0, 102.0, 60000.0), // duplicate
            bar("2025-01-06", 102.0, 102.5, 98.5, 99.0, 45000.0),  // new
            bar("2025-01-07", 99.0, 101.5, 98.0, 101.0, 55000.0),  // new
        ];

        let result = updater.merge_bars("SPY", &new_bars).unwrap();
        assert_eq!(result.bars_appended, 2);
        assert_eq!(result.total_bars, 4);

        let loader = CsvLoader::new(dir.path());
        let series = loader.load_bars("SPY", None, None).unwrap();
        assert_eq!(series.len(), 4);
    }

    #[test]
    fn merge_no_new_bars() {
        let dir = TempDir::new().unwrap();
        write_csv(
            dir.path(),
            "SPY",
            "Date,Close,High,Low,Open,Volume\n\
             2025-01-02,100.5,101.0,99.0,100.0,50000\n",
        );

        let updater = DataUpdater::new(dir.path());
        let new_bars = vec![bar("2025-01-02", 100.0, 101.0, 99.0, 100.5, 50000.0)];

        let result = updater.merge_bars("SPY", &new_bars).unwrap();
        assert_eq!(result.bars_appended, 0);
        assert_eq!(result.total_bars, 1);
    }

    #[test]
    fn csv_column_order_preserved() {
        let dir = TempDir::new().unwrap();
        let updater = DataUpdater::new(dir.path());

        let bars = vec![bar("2025-01-02", 100.0, 101.0, 99.0, 100.5, 50000.0)];
        updater.merge_bars("TEST", &bars).unwrap();

        let content = std::fs::read_to_string(dir.path().join("TEST.csv")).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines[0], "Date,Close,High,Low,Open,Volume");
        // CSV order: Date,Close,High,Low,Open,Volume
        assert!(lines[1].starts_with("2025-01-02,100.5,"));
    }

    #[test]
    fn discover_symbols_finds_csvs() {
        let dir = TempDir::new().unwrap();
        write_csv(dir.path(), "SPY", "Date,Close,High,Low,Open,Volume\n");
        write_csv(dir.path(), "GC=F", "Date,Close,High,Low,Open,Volume\n");
        // Non-CSV file should be ignored
        std::fs::write(dir.path().join("notes.txt"), "hello").unwrap();

        let updater = DataUpdater::new(dir.path());
        let symbols = updater.discover_symbols().unwrap();
        assert_eq!(symbols, vec!["GC=F", "SPY"]);
    }
}
