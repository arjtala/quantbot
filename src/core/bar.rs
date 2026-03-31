use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BarError {
    #[error("bar series is empty")]
    Empty,
    #[error("bar series is not sorted by date: {0} comes after {1}")]
    Unsorted(NaiveDate, NaiveDate),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bar {
    pub date: NaiveDate,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarSeries {
    bars: Vec<Bar>,
}

impl BarSeries {
    pub fn new(bars: Vec<Bar>) -> Result<Self, BarError> {
        let series = Self { bars };
        series.validate()?;
        Ok(series)
    }

    fn validate(&self) -> Result<(), BarError> {
        if self.bars.is_empty() {
            return Err(BarError::Empty);
        }
        for w in self.bars.windows(2) {
            if w[1].date < w[0].date {
                return Err(BarError::Unsorted(w[1].date, w[0].date));
            }
        }
        Ok(())
    }

    pub fn bars(&self) -> &[Bar] {
        &self.bars
    }

    pub fn len(&self) -> usize {
        self.bars.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bars.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(year: i32, month: u32, day: u32, close: f64) -> Bar {
        Bar {
            date: NaiveDate::from_ymd_opt(year, month, day).unwrap(),
            open: close,
            high: close,
            low: close,
            close,
            volume: 1000.0,
        }
    }

    #[test]
    fn valid_series() {
        let series = BarSeries::new(vec![
            bar(2025, 1, 2, 100.0),
            bar(2025, 1, 3, 101.0),
            bar(2025, 1, 6, 99.0),
        ])
        .unwrap();
        assert_eq!(series.len(), 3);
    }

    #[test]
    fn empty_series_rejected() {
        let err = BarSeries::new(vec![]).unwrap_err();
        assert!(matches!(err, BarError::Empty));
    }

    #[test]
    fn unsorted_series_rejected() {
        let err = BarSeries::new(vec![
            bar(2025, 1, 6, 99.0),
            bar(2025, 1, 2, 100.0),
        ])
        .unwrap_err();
        assert!(matches!(err, BarError::Unsorted(_, _)));
    }
}
