"""Trendline fitting and support/resistance detection."""

from __future__ import annotations

from typing import Any

import numpy as np
import pandas as pd


def find_support_resistance(
    bars: pd.DataFrame,
    window: int = 20,
    num_levels: int = 3,
) -> dict[str, list[float]]:
    """Detect support and resistance levels from local extrema.

    Uses rolling min/max to find pivot points, then clusters nearby levels.

    Args:
        bars: OHLCV DataFrame.
        window: Rolling window for extrema detection.
        num_levels: Max number of levels to return per side.

    Returns:
        Dict with "support" and "resistance" lists of price levels.
    """
    close = bars["Close"]
    high = bars["High"]
    low = bars["Low"]
    current_price = float(close.iloc[-1])

    # Find local minima (support) and maxima (resistance)
    rolling_min = low.rolling(window, center=True).min()
    rolling_max = high.rolling(window, center=True).max()

    support_candidates = low[low == rolling_min].dropna().values
    resistance_candidates = high[high == rolling_max].dropna().values

    # Cluster nearby levels (within 1% of each other)
    def cluster_levels(levels: np.ndarray, threshold_pct: float = 0.01) -> list[float]:
        if len(levels) == 0:
            return []
        sorted_levels = np.sort(levels)
        clusters: list[list[float]] = [[sorted_levels[0]]]
        for level in sorted_levels[1:]:
            if abs(level - clusters[-1][-1]) / clusters[-1][-1] < threshold_pct:
                clusters[-1].append(level)
            else:
                clusters.append([level])
        # Return the mean of each cluster, sorted by proximity to current price
        means = [float(np.mean(c)) for c in clusters]
        means.sort(key=lambda x: abs(x - current_price))
        return means

    supports = [s for s in cluster_levels(support_candidates) if s < current_price]
    resistances = [r for r in cluster_levels(resistance_candidates) if r > current_price]

    return {
        "support": supports[:num_levels],
        "resistance": resistances[:num_levels],
    }


def fit_trendline(
    bars: pd.DataFrame,
    lookback: int = 60,
    kind: str = "support",
) -> dict[str, Any] | None:
    """Fit a linear trendline to recent lows (support) or highs (resistance).

    Args:
        bars: OHLCV DataFrame.
        lookback: Number of bars to use.
        kind: "support" (fit to lows) or "resistance" (fit to highs).

    Returns:
        Dict with "start", "end" (x,y) tuples and "slope", or None if insufficient data.
    """
    df = bars.iloc[-lookback:]
    if len(df) < 10:
        return None

    x = np.arange(len(df))
    y = df["Low"].values if kind == "support" else df["High"].values

    # Simple linear regression
    coeffs = np.polyfit(x, y, 1)
    slope, intercept = coeffs

    return {
        "start": (0, float(intercept)),
        "end": (len(df) - 1, float(slope * (len(df) - 1) + intercept)),
        "slope": float(slope),
        "color": "#26a69a" if kind == "support" else "#ef5350",
    }
