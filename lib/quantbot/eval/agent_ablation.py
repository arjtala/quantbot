"""Agent ablation study — measure marginal contribution of each signal source.

Runs the backtest with different agent combinations to quantify
each agent's value-add to the ensemble.
"""

from __future__ import annotations

import logging
from dataclasses import dataclass
from typing import Any

import pandas as pd

from quantbot.agents.decision.combiner import SignalCombiner
from quantbot.agents.tsmom.agent import TSMOMAgent
from quantbot.backtest.engine import BacktestConfig, BacktestEngine
from quantbot.backtest.metrics import BacktestResult
from quantbot.core.signal import Signal

logger = logging.getLogger(__name__)


@dataclass
class AblationResult:
    """Results from an ablation study."""

    name: str
    agents_used: list[str]
    sharpe: float
    ann_return: float
    max_drawdown: float
    total_trades: int


def run_ablation(
    bars_by_instrument: dict[str, pd.DataFrame],
    signal_sets: dict[str, dict[str, list[list[Signal]]]],
    config: BacktestConfig | None = None,
) -> list[AblationResult]:
    """Run ablation study comparing different agent combinations.

    This is a framework for comparing ensemble performance. In practice,
    you'd pre-compute signals from each agent and test subsets.

    Args:
        bars_by_instrument: Dict of instrument -> OHLCV bars.
        signal_sets: Dict of experiment_name -> {instrument -> list of daily signal lists}.
        config: Backtest configuration.

    Returns:
        List of AblationResult for each experiment.
    """
    results = []

    # Always run TSMOM-only as baseline
    agent = TSMOMAgent()
    engine = BacktestEngine(config or BacktestConfig())
    snapshots = engine.run(agent, bars_by_instrument, min_history=252)

    if len(snapshots) >= 2:
        bt = BacktestResult.from_snapshots(snapshots)
        results.append(AblationResult(
            name="TSMOM Only (baseline)",
            agents_used=["TSMOM"],
            sharpe=bt.sharpe_ratio,
            ann_return=bt.annualized_return,
            max_drawdown=bt.max_drawdown,
            total_trades=bt.total_trades,
        ))

    return results


def print_ablation_table(results: list[AblationResult]) -> str:
    """Format ablation results as a readable table."""
    lines = [
        f"{'Experiment':<35} {'Agents':<30} {'Sharpe':>8} {'Ann.Ret':>10} {'MaxDD':>10} {'Trades':>8}",
        "-" * 101,
    ]
    for r in results:
        agents = ", ".join(r.agents_used)
        lines.append(
            f"{r.name:<35} {agents:<30} {r.sharpe:>8.2f} {r.ann_return:>9.1%} {r.max_drawdown:>9.1%} {r.total_trades:>8,}"
        )
    return "\n".join(lines)
