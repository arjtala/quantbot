"""Bar data types and validation."""

from __future__ import annotations

from typing import TypeAlias

import pandas as pd

# A BarDataFrame is a pd.DataFrame with a DatetimeIndex and OHLCV columns.
BarDataFrame: TypeAlias = pd.DataFrame

REQUIRED_COLUMNS = {"Open", "High", "Low", "Close", "Volume"}


def validate_bars(df: pd.DataFrame) -> BarDataFrame:
    """Validate and return a BarDataFrame.

    Raises ValueError if the DataFrame doesn't have the expected schema.
    """
    if df.empty:
        raise ValueError("Bar DataFrame is empty")

    if not isinstance(df.index, pd.DatetimeIndex):
        raise ValueError(
            f"Expected DatetimeIndex, got {type(df.index).__name__}"
        )

    missing = REQUIRED_COLUMNS - set(df.columns)
    if missing:
        raise ValueError(f"Missing columns: {missing}")

    if not df.index.is_monotonic_increasing:
        df = df.sort_index()

    return df
