use chrono::{Datelike, NaiveDate, Weekday};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FreshnessError {
    #[error("{symbol}: data stale — last bar {last_date}, expected at least {expected_date} (gap: {gap_days} days, max allowed: {max_stale_days})")]
    Stale {
        symbol: String,
        last_date: NaiveDate,
        expected_date: NaiveDate,
        gap_days: i64,
        max_stale_days: u32,
    },

    #[error("{symbol}: no data available")]
    NoData { symbol: String },
}

/// Check whether the data for `symbol` is fresh enough to trade.
///
/// Returns `Ok(())` if the last bar date is within `max_stale_days` trading days
/// of the expected date. The expected date is the previous trading day (weekday)
/// relative to `today`.
pub fn check_freshness(
    symbol: &str,
    last_bar_date: Option<NaiveDate>,
    today: NaiveDate,
    max_stale_days: u32,
) -> Result<(), FreshnessError> {
    let last = last_bar_date.ok_or_else(|| FreshnessError::NoData {
        symbol: symbol.to_string(),
    })?;

    let expected = previous_trading_day(today);
    let gap = (expected - last).num_days();

    if gap > max_stale_days as i64 {
        return Err(FreshnessError::Stale {
            symbol: symbol.to_string(),
            last_date: last,
            expected_date: expected,
            gap_days: gap,
            max_stale_days,
        });
    }

    Ok(())
}

/// Return the most recent weekday before `today`.
///
/// - Monday → Friday
/// - Saturday → Friday
/// - Sunday → Friday
/// - Tuesday–Friday → yesterday
pub fn previous_trading_day(today: NaiveDate) -> NaiveDate {
    match today.weekday() {
        Weekday::Mon => today - chrono::Days::new(3),
        Weekday::Sun => today - chrono::Days::new(2),
        Weekday::Sat => today - chrono::Days::new(1),
        _ => today - chrono::Days::new(1),
    }
}

/// Check freshness for all symbols. Returns a list of errors for stale/missing symbols.
pub fn check_all_fresh(
    symbols_with_dates: &[(String, Option<NaiveDate>)],
    today: NaiveDate,
    max_stale_days: u32,
) -> Vec<FreshnessError> {
    symbols_with_dates
        .iter()
        .filter_map(|(sym, last)| check_freshness(sym, *last, today, max_stale_days).err())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn previous_trading_day_tuesday() {
        // Tuesday → Monday
        assert_eq!(previous_trading_day(d(2025, 1, 7)), d(2025, 1, 6));
    }

    #[test]
    fn previous_trading_day_monday() {
        // Monday → Friday
        assert_eq!(previous_trading_day(d(2025, 1, 6)), d(2025, 1, 3));
    }

    #[test]
    fn previous_trading_day_saturday() {
        // Saturday → Friday
        assert_eq!(previous_trading_day(d(2025, 1, 4)), d(2025, 1, 3));
    }

    #[test]
    fn previous_trading_day_sunday() {
        // Sunday → Friday
        assert_eq!(previous_trading_day(d(2025, 1, 5)), d(2025, 1, 3));
    }

    #[test]
    fn previous_trading_day_wednesday() {
        // Wednesday → Tuesday
        assert_eq!(previous_trading_day(d(2025, 1, 8)), d(2025, 1, 7));
    }

    #[test]
    fn fresh_data_passes() {
        // Today is Tuesday, last bar is Monday → gap 0
        let result = check_freshness("SPY", Some(d(2025, 1, 6)), d(2025, 1, 7), 3);
        assert!(result.is_ok());
    }

    #[test]
    fn stale_data_fails() {
        // Today is Friday Jan 10, last bar is Monday Jan 6 → expected Thu Jan 9 → gap 3
        // With max_stale_days=2, this should fail
        let result = check_freshness("SPY", Some(d(2025, 1, 6)), d(2025, 1, 10), 2);
        assert!(result.is_err());
    }

    #[test]
    fn weekend_tolerant() {
        // Today is Monday, last bar is Friday → gap 0
        let result = check_freshness("SPY", Some(d(2025, 1, 3)), d(2025, 1, 6), 3);
        assert!(result.is_ok());
    }

    #[test]
    fn holiday_tolerance() {
        // Today is Thursday, last bar is Monday (2 day gap due to holiday)
        // max_stale_days=3 → should pass
        let result = check_freshness("SPY", Some(d(2025, 1, 6)), d(2025, 1, 9), 3);
        assert!(result.is_ok());
    }

    #[test]
    fn no_data_fails() {
        let result = check_freshness("SPY", None, d(2025, 1, 7), 3);
        assert!(matches!(result, Err(FreshnessError::NoData { .. })));
    }

    #[test]
    fn check_all_collects_errors() {
        let symbols = vec![
            ("SPY".to_string(), Some(d(2025, 1, 6))),  // fresh
            ("GLD".to_string(), None),                    // no data
            ("GC=F".to_string(), Some(d(2024, 12, 1))), // stale
        ];
        let errors = check_all_fresh(&symbols, d(2025, 1, 7), 3);
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn exact_boundary_passes() {
        // Today is Tuesday Jan 7, expected = Monday Jan 6, last bar = Jan 3 → gap = 3
        let result = check_freshness("SPY", Some(d(2025, 1, 3)), d(2025, 1, 7), 3);
        assert!(result.is_ok());
    }

    #[test]
    fn one_over_boundary_fails() {
        // Today is Wednesday Jan 8, expected = Tuesday Jan 7, last bar = Jan 3 → gap = 4 > 3
        let result = check_freshness("SPY", Some(d(2025, 1, 3)), d(2025, 1, 8), 3);
        assert!(result.is_err());
    }
}
