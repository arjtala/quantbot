"""LangGraph state definitions for the trading graph."""

from __future__ import annotations

import operator
from typing import Annotated, Any, TypedDict

from quantbot.core.signal import Signal


def add_signals(left: list[Signal], right: list[Signal]) -> list[Signal]:
    """Reducer that accumulates signals from parallel agent nodes."""
    return left + right


def merge_dicts(left: dict[str, Any], right: dict[str, Any]) -> dict[str, Any]:
    """Reducer that merges metadata dicts."""
    return {**left, **right}


class TradingGraphState(TypedDict):
    """State flowing through the trading graph.

    Attributes:
        instrument: The instrument being analyzed.
        bars: Serialized bar data (JSON or reference key).
        signals: Accumulated signals from all agents (fan-out reducer).
        debate: Bull/bear debate arguments (if debate is enabled).
        decision: Final decision from the Decision Agent.
        memory_context: Recent decision history injected from SQLite.
        metadata: Arbitrary pass-through data.
    """

    instrument: str
    bars: Any  # pd.DataFrame passed through (LangGraph supports arbitrary types)
    signals: Annotated[list[Signal], add_signals]
    debate: Annotated[dict[str, Any], merge_dicts]
    decision: dict[str, Any]
    memory_context: str
    metadata: Annotated[dict[str, Any], merge_dicts]
