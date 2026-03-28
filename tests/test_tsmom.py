"""Tests for TSMOM agent and volatility."""

from __future__ import annotations

import numpy as np
import pandas as pd
import pytest

from quantbot.agents.tsmom.agent import TSMOMAgent
from quantbot.agents.tsmom.volatility import ewma_volatility
from quantbot.core.signal import SignalDirection, SignalType


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def make_bars(prices: list[float], start: str = "2020-01-01") -> pd.DataFrame:
    """Create a minimal OHLCV DataFrame from a list of close prices."""
    dates = pd.bdate_range(start=start, periods=len(prices))
    return pd.DataFrame(
        {
            "Open": prices,
            "High": [p * 1.01 for p in prices],
            "Low": [p * 0.99 for p in prices],
            "Close": prices,
            "Volume": [1_000_000] * len(prices),
        },
        index=dates,
    )


def make_trending_bars(direction: str = "up", n: int = 300) -> pd.DataFrame:
    """Generate bars with a clear trend."""
    np.random.seed(42)
    if direction == "up":
        drift = 0.001  # ~25% annualized
    else:
        drift = -0.001
    log_returns = np.random.normal(drift, 0.01, n)
    prices = 100.0 * np.exp(np.cumsum(log_returns))
    return make_bars(prices.tolist())


# ---------------------------------------------------------------------------
# Volatility tests
# ---------------------------------------------------------------------------

class TestEWMAVolatility:
    def test_positive_output(self):
        np.random.seed(0)
        returns = pd.Series(np.random.normal(0, 0.01, 252))
        vol = ewma_volatility(returns)
        assert (vol.dropna() > 0).all()

    def test_annualized_scale(self):
        """Vol of ~1% daily returns should be roughly 15-16% annualized."""
        np.random.seed(1)
        returns = pd.Series(np.random.normal(0, 0.01, 500))
        vol = ewma_volatility(returns)
        last_vol = vol.iloc[-1]
        assert 0.10 < last_vol < 0.25, f"Unexpected vol: {last_vol:.3f}"

    def test_higher_vol_returns(self):
        """Higher daily std should produce higher annualized vol."""
        np.random.seed(2)
        low_vol = pd.Series(np.random.normal(0, 0.005, 300))
        high_vol = pd.Series(np.random.normal(0, 0.02, 300))
        v_low = ewma_volatility(low_vol).iloc[-1]
        v_high = ewma_volatility(high_vol).iloc[-1]
        assert v_high > v_low


# ---------------------------------------------------------------------------
# TSMOM Agent tests
# ---------------------------------------------------------------------------

class TestTSMOMAgent:
    def test_uptrend_produces_long(self):
        bars = make_trending_bars("up", n=300)
        agent = TSMOMAgent()
        signal = agent.generate_signal(bars, "TEST")

        assert signal.direction == SignalDirection.LONG
        assert signal.strength > 0
        assert signal.confidence > 0
        assert signal.signal_type == SignalType.QUANT
        assert signal.agent_name == "TSMOM"

    def test_downtrend_produces_short(self):
        bars = make_trending_bars("down", n=300)
        agent = TSMOMAgent()
        signal = agent.generate_signal(bars, "TEST")

        assert signal.direction == SignalDirection.SHORT
        assert signal.strength < 0

    def test_insufficient_data_returns_flat(self):
        bars = make_bars([100.0] * 50)  # only 50 bars, need 252
        agent = TSMOMAgent()
        signal = agent.generate_signal(bars, "TEST")

        assert signal.direction == SignalDirection.FLAT
        assert signal.strength == 0.0
        assert signal.confidence == 0.0

    def test_metadata_contains_vol(self):
        bars = make_trending_bars("up", n=300)
        agent = TSMOMAgent()
        signal = agent.generate_signal(bars, "TEST")

        assert "ann_vol" in signal.metadata
        assert "vol_scalar" in signal.metadata
        assert signal.metadata["ann_vol"] > 0

    def test_target_weight_respects_direction(self):
        bars = make_trending_bars("up", n=300)
        agent = TSMOMAgent()
        signal = agent.generate_signal(bars, "TEST")
        weight = agent.compute_target_weight(signal)

        assert weight > 0  # long trend → positive weight

    def test_custom_lookbacks(self):
        bars = make_trending_bars("up", n=300)
        agent = TSMOMAgent(lookbacks=(21, 63))
        signal = agent.generate_signal(bars, "TEST")

        assert signal.metadata["lookbacks"] == [21, 63]

    def test_signal_validation(self):
        """Signal strength and confidence must be in valid ranges."""
        bars = make_trending_bars("up", n=300)
        agent = TSMOMAgent()
        signal = agent.generate_signal(bars, "TEST")

        assert -1.0 <= signal.strength <= 1.0
        assert 0.0 <= signal.confidence <= 1.0
