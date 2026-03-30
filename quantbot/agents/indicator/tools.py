"""Technical indicator computation tools.

Computes RSI, MACD, Stochastic, ROC, Williams %R from OHLCV data.
Uses pure numpy/pandas — TA-Lib is optional for cross-validation.
"""

from __future__ import annotations

from typing import Any

import numpy as np
import pandas as pd


def compute_rsi(close: pd.Series, period: int = 14) -> pd.Series:
    """Relative Strength Index."""
    delta = close.diff()
    gain = delta.where(delta > 0, 0.0)
    loss = -delta.where(delta < 0, 0.0)
    avg_gain = gain.ewm(com=period - 1, min_periods=period).mean()
    avg_loss = loss.ewm(com=period - 1, min_periods=period).mean()
    rs = avg_gain / avg_loss.replace(0, np.inf)
    return 100 - (100 / (1 + rs))


def compute_macd(
    close: pd.Series,
    fast: int = 12,
    slow: int = 26,
    signal: int = 9,
) -> dict[str, pd.Series]:
    """MACD line, signal line, and histogram."""
    ema_fast = close.ewm(span=fast, adjust=False).mean()
    ema_slow = close.ewm(span=slow, adjust=False).mean()
    macd_line = ema_fast - ema_slow
    signal_line = macd_line.ewm(span=signal, adjust=False).mean()
    histogram = macd_line - signal_line
    return {"macd": macd_line, "signal": signal_line, "histogram": histogram}


def compute_stochastic(
    high: pd.Series,
    low: pd.Series,
    close: pd.Series,
    k_period: int = 14,
    d_period: int = 3,
) -> dict[str, pd.Series]:
    """Stochastic Oscillator (%K and %D)."""
    lowest_low = low.rolling(k_period).min()
    highest_high = high.rolling(k_period).max()
    k = 100 * (close - lowest_low) / (highest_high - lowest_low).replace(0, np.inf)
    d = k.rolling(d_period).mean()
    return {"k": k, "d": d}


def compute_roc(close: pd.Series, period: int = 12) -> pd.Series:
    """Rate of Change (percentage)."""
    return close.pct_change(periods=period) * 100


def compute_williams_r(
    high: pd.Series,
    low: pd.Series,
    close: pd.Series,
    period: int = 14,
) -> pd.Series:
    """Williams %R."""
    highest_high = high.rolling(period).max()
    lowest_low = low.rolling(period).min()
    return -100 * (highest_high - close) / (highest_high - lowest_low).replace(0, np.inf)


def compute_trend_context(close: pd.Series) -> dict[str, Any]:
    """Compute trend regime context from moving averages."""
    sma_20 = close.rolling(20).mean()
    sma_50 = close.rolling(50).mean()
    sma_200 = close.rolling(200).mean()

    price = float(close.iloc[-1])

    # SMA slopes (annualized % change over last 5 days)
    def slope_pct(sma: pd.Series) -> float:
        if len(sma.dropna()) < 6:
            return 0.0
        return float((sma.iloc[-1] / sma.iloc[-6] - 1) * 252 / 5 * 100)

    sma_20_slope = slope_pct(sma_20)
    sma_50_slope = slope_pct(sma_50)

    # Trend regime label
    above_20 = price > float(sma_20.iloc[-1]) if not np.isnan(sma_20.iloc[-1]) else None
    above_50 = price > float(sma_50.iloc[-1]) if not np.isnan(sma_50.iloc[-1]) else None
    above_200 = price > float(sma_200.iloc[-1]) if len(sma_200.dropna()) > 0 and not np.isnan(sma_200.iloc[-1]) else None

    if above_20 and above_50 and sma_20_slope > 0 and sma_50_slope > 0:
        regime = "UPTREND"
    elif not above_20 and not above_50 and sma_20_slope < 0 and sma_50_slope < 0:
        regime = "DOWNTREND"
    else:
        regime = "SIDEWAYS"

    return {
        "trend_regime": regime,
        "price_vs_sma20": "above" if above_20 else "below" if above_20 is not None else "n/a",
        "price_vs_sma50": "above" if above_50 else "below" if above_50 is not None else "n/a",
        "price_vs_sma200": "above" if above_200 else "below" if above_200 is not None else "n/a",
        "sma20_slope_ann_pct": round(sma_20_slope, 1),
        "sma50_slope_ann_pct": round(sma_50_slope, 1),
    }


def compute_all_indicators(bars: pd.DataFrame) -> dict[str, Any]:
    """Compute all technical indicators and return latest values as a dict.

    Args:
        bars: OHLCV DataFrame.

    Returns:
        Dict with latest indicator values, suitable for LLM prompt injection.
    """
    close = bars["Close"]
    high = bars["High"]
    low = bars["Low"]

    rsi = compute_rsi(close)
    macd = compute_macd(close)
    stoch = compute_stochastic(high, low, close)
    roc = compute_roc(close)
    williams = compute_williams_r(high, low, close)

    return {
        "trend": compute_trend_context(close),
        "rsi_14": round(float(rsi.iloc[-1]), 2),
        "macd_line": round(float(macd["macd"].iloc[-1]), 4),
        "macd_signal": round(float(macd["signal"].iloc[-1]), 4),
        "macd_histogram": round(float(macd["histogram"].iloc[-1]), 4),
        "stoch_k": round(float(stoch["k"].iloc[-1]), 2),
        "stoch_d": round(float(stoch["d"].iloc[-1]), 2),
        "roc_12": round(float(roc.iloc[-1]), 2),
        "williams_r_14": round(float(williams.iloc[-1]), 2),
        "close": round(float(close.iloc[-1]), 2),
        "close_change_1d": round(float(close.pct_change().iloc[-1] * 100), 2),
        "close_change_5d": round(float(close.pct_change(5).iloc[-1] * 100), 2),
    }
