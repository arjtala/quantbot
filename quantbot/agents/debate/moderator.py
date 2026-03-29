"""Debate Moderator — runs bull/bear debate and structures arguments for Decision Agent."""

from __future__ import annotations

import json
from typing import Any

import pandas as pd

from quantbot.agents.debate.bear import invoke_bear_advocate
from quantbot.agents.debate.bull import invoke_bull_advocate
from quantbot.config import settings
from quantbot.core.signal import Signal
from quantbot.graph.state import TradingGraphState


def _summarize_bars(bars: pd.DataFrame, last_n: int = 10) -> str:
    """Create a text summary of recent price action for debate context."""
    df = bars.iloc[-last_n:]
    close = df["Close"]
    change_pct = (close.iloc[-1] / close.iloc[0] - 1) * 100
    high = float(df["High"].max())
    low = float(df["Low"].min())
    current = float(close.iloc[-1])

    return (
        f"Last {last_n} bars: price moved {change_pct:+.1f}% "
        f"(current={current:.2f}, high={high:.2f}, low={low:.2f})"
    )


def make_debate_node() -> Any:
    """Create a LangGraph-compatible debate moderator node.

    Runs bull and bear advocates, structures their arguments,
    and adds the debate summary to the graph state.
    """

    def debate_node(state: TradingGraphState) -> dict[str, Any]:
        if not settings.debate_enabled:
            return {"debate": {}}

        instrument = state["instrument"]
        signals = state.get("signals", [])
        bars = state["bars"]

        bars_summary = _summarize_bars(bars)

        # Run bull and bear advocates
        bull_args = invoke_bull_advocate(instrument, signals, bars_summary)
        bear_args = invoke_bear_advocate(instrument, signals, bars_summary)

        debate_summary = {
            "bull": bull_args,
            "bear": bear_args,
            "bull_conviction": bull_args.get("conviction", 0.0),
            "bear_conviction": bear_args.get("conviction", 0.0),
        }

        return {"debate": debate_summary}

    return debate_node
