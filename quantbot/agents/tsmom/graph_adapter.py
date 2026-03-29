"""Wraps the Phase 1 TSMOM agent as a LangGraph node.

No LLM call — pure numeric signal in the same Signal schema.
Acts as the deterministic anchor in the ensemble.
"""

from __future__ import annotations

from typing import Any

from quantbot.agents.tsmom.agent import TSMOMAgent
from quantbot.graph.state import TradingGraphState


def make_tsmom_node(
    agent: TSMOMAgent | None = None,
) -> Any:
    """Create a LangGraph-compatible TSMOM node function.

    Args:
        agent: Pre-configured TSMOMAgent. If None, creates one with defaults.

    Returns:
        A function compatible with StateGraph.add_node().
    """
    if agent is None:
        agent = TSMOMAgent()

    def tsmom_node(state: TradingGraphState) -> dict[str, Any]:
        bars = state["bars"]
        instrument = state["instrument"]
        signal = agent.generate_signal(bars, instrument)
        return {"signals": [signal]}

    return tsmom_node
