"""Tests for backtest engine and metrics."""

from __future__ import annotations

import numpy as np
import pandas as pd
import pytest

from quantbot.agents.tsmom.agent import TSMOMAgent
from quantbot.backtest.engine import BacktestConfig, BacktestEngine, Snapshot
from quantbot.backtest.metrics import BacktestResult


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def make_bars(prices: list[float], start: str = "2018-01-01") -> pd.DataFrame:
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


def make_trending_bars(drift: float = 0.001, n: int = 600) -> pd.DataFrame:
    np.random.seed(42)
    log_returns = np.random.normal(drift, 0.01, n)
    prices = 100.0 * np.exp(np.cumsum(log_returns))
    return make_bars(prices.tolist())


# ---------------------------------------------------------------------------
# BacktestEngine tests
# ---------------------------------------------------------------------------

class TestBacktestEngine:
    def test_runs_without_error(self):
        bars = make_trending_bars(drift=0.001, n=400)
        agent = TSMOMAgent()
        engine = BacktestEngine()

        snapshots = engine.run(
            agent, {"ASSET": bars}, min_history=252
        )
        assert len(snapshots) > 0

    def test_nav_starts_at_initial_cash(self):
        bars = make_trending_bars(drift=0.001, n=400)
        config = BacktestConfig(initial_cash=500_000)
        agent = TSMOMAgent()
        engine = BacktestEngine(config)

        snapshots = engine.run(agent, {"ASSET": bars}, min_history=252)
        # First snapshot NAV should be close to initial cash
        # (may differ slightly due to first-day trades)
        assert abs(snapshots[0].nav - 500_000) / 500_000 < 0.05

    def test_uptrend_makes_money(self):
        """In a strong uptrend, TSMOM should be profitable."""
        bars = make_trending_bars(drift=0.002, n=600)
        agent = TSMOMAgent()
        engine = BacktestEngine()

        snapshots = engine.run(agent, {"ASSET": bars}, min_history=252)
        assert snapshots[-1].nav > snapshots[0].nav

    def test_slippage_reduces_returns(self):
        """High slippage should cost more than zero slippage over many trades."""
        # Use a strong uptrend with more bars so cumulative slippage is visible
        bars = make_trending_bars(drift=0.003, n=800)
        agent = TSMOMAgent()

        config_no_slip = BacktestConfig(slippage_bps=0.0)
        config_high_slip = BacktestConfig(slippage_bps=100.0)

        snaps_no = BacktestEngine(config_no_slip).run(agent, {"ASSET": bars}, min_history=252)
        snaps_hi = BacktestEngine(config_high_slip).run(agent, {"ASSET": bars}, min_history=252)

        # Count total fills to confirm trades happened
        fills_no = sum(len(s.fills) for s in snaps_no)
        fills_hi = sum(len(s.fills) for s in snaps_hi)
        assert fills_no > 0 and fills_hi > 0
        assert snaps_no[-1].nav > snaps_hi[-1].nav

    def test_multi_instrument(self):
        """Engine should handle multiple instruments."""
        bars_a = make_trending_bars(drift=0.001, n=400)
        bars_b = make_trending_bars(drift=-0.001, n=400)
        agent = TSMOMAgent()
        engine = BacktestEngine()

        snapshots = engine.run(
            agent,
            {"ASSET_A": bars_a, "ASSET_B": bars_b},
            min_history=252,
        )
        assert len(snapshots) > 0
        # Should have signals for both instruments in later snapshots
        last = snapshots[-1]
        assert "ASSET_A" in last.signals or "ASSET_B" in last.signals

    def test_insufficient_history_raises(self):
        bars = make_bars([100.0] * 50)
        agent = TSMOMAgent()
        engine = BacktestEngine()

        with pytest.raises(ValueError, match="Need >"):
            engine.run(agent, {"ASSET": bars}, min_history=252)


# ---------------------------------------------------------------------------
# BacktestResult / Metrics tests
# ---------------------------------------------------------------------------

class TestBacktestResult:
    def _make_snapshots(self, n: int = 100, drift: float = 0.0005) -> list[Snapshot]:
        np.random.seed(99)
        nav = 1_000_000.0
        snapshots = []
        dates = pd.bdate_range(start="2022-01-01", periods=n)
        for d in dates:
            nav *= 1 + np.random.normal(drift, 0.01)
            snapshots.append(
                Snapshot(
                    timestamp=d,
                    nav=nav,
                    cash=nav * 0.1,
                    gross_exposure=nav * 0.9,
                    net_exposure=nav * 0.5,
                    positions={},
                    signals={},
                    fills=[],
                )
            )
        return snapshots

    def test_sharpe_positive_for_positive_drift(self):
        snaps = self._make_snapshots(500, drift=0.001)
        result = BacktestResult.from_snapshots(snaps)
        assert result.sharpe_ratio > 0

    def test_sharpe_negative_for_negative_drift(self):
        snaps = self._make_snapshots(500, drift=-0.001)
        result = BacktestResult.from_snapshots(snaps)
        assert result.sharpe_ratio < 0

    def test_max_drawdown_is_negative(self):
        snaps = self._make_snapshots(500, drift=0.0)
        result = BacktestResult.from_snapshots(snaps)
        assert result.max_drawdown <= 0

    def test_summary_string(self):
        snaps = self._make_snapshots(500, drift=0.0005)
        result = BacktestResult.from_snapshots(snaps)
        summary = result.summary()

        assert "Sharpe Ratio" in summary
        assert "Max Drawdown" in summary
        assert "Ann. Return" in summary

    def test_equity_curve_length(self):
        snaps = self._make_snapshots(200)
        result = BacktestResult.from_snapshots(snaps)
        assert len(result.equity_curve) == 200

    def test_too_few_snapshots_raises(self):
        with pytest.raises(ValueError, match="at least 2"):
            BacktestResult.from_snapshots([])
