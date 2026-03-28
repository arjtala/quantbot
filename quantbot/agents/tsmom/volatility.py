"""Volatility estimation utilities (Moskowitz et al. JFE 2012)."""

from __future__ import annotations

import numpy as np
import pandas as pd

# Annualization factor for daily returns
SQRT_252 = np.sqrt(252)


def ewma_volatility(
    returns: pd.Series,
    com: int = 60,
    min_periods: int = 20,
) -> pd.Series:
    """Compute annualized EWMA volatility.

    Args:
        returns: Daily log or simple returns.
        com: Center of mass for the exponential weighting (paper uses 60).
        min_periods: Minimum observations required before producing a value.

    Returns:
        Annualized volatility series aligned with the input index.
    """
    variance = returns.pow(2).ewm(com=com, min_periods=min_periods).mean()
    daily_vol = np.sqrt(variance)
    return daily_vol * SQRT_252


def realised_volatility(
    returns: pd.Series,
    window: int = 252,
) -> pd.Series:
    """Simple rolling realised volatility (annualized)."""
    return returns.rolling(window=window, min_periods=window // 2).std() * SQRT_252
