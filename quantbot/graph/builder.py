"""Build the trading graph with configurable agent selection."""

from __future__ import annotations

from typing import Any

from langgraph.graph import END, StateGraph

from quantbot.config import settings
from quantbot.graph.state import TradingGraphState


def build_trading_graph(
    tsmom_node: Any,
    indicator_node: Any | None = None,
    pattern_node: Any | None = None,
    trend_node: Any | None = None,
    debate_node: Any | None = None,
    combiner_node: Any | None = None,
    decision_node: Any | None = None,
    risk_node: Any | None = None,
) -> StateGraph:
    """Build a fan-out/fan-in trading graph.

    All signal-generating agents run in parallel (fan-out).
    Signals merge at the combiner node (fan-in).
    Optional debate and risk nodes add layers of refinement.

    Args:
        tsmom_node: TSMOM agent node function (required).
        indicator_node: Indicator agent node function (optional).
        pattern_node: Pattern agent node function (optional).
        trend_node: Trend agent node function (optional).
        debate_node: Bull/bear debate node function (optional).
        combiner_node: Signal combiner node function (optional).
        decision_node: Decision agent node function (optional).
        risk_node: Risk manager node function (optional).

    Returns:
        Compiled StateGraph ready for invocation.
    """
    graph = StateGraph(TradingGraphState)

    # --- Register all nodes ---
    graph.add_node("tsmom", tsmom_node)

    signal_agents = ["tsmom"]

    if indicator_node is not None:
        graph.add_node("indicator", indicator_node)
        signal_agents.append("indicator")

    if pattern_node is not None:
        graph.add_node("pattern", pattern_node)
        signal_agents.append("pattern")

    if trend_node is not None:
        graph.add_node("trend", trend_node)
        signal_agents.append("trend")

    # --- Fan-out: entry → all signal agents in parallel ---
    def fan_out(state: TradingGraphState) -> list[str]:
        return signal_agents

    graph.add_conditional_edges("__start__", fan_out, signal_agents)

    # --- Fan-in: all signal agents → combiner ---
    fan_in_target = "combiner"

    if combiner_node is not None:
        graph.add_node("combiner", combiner_node)
    else:
        # Passthrough if no combiner
        graph.add_node("combiner", lambda state: state)

    for agent_name in signal_agents:
        graph.add_edge(agent_name, "combiner")

    # --- Optional debate after combiner ---
    if debate_node is not None and settings.debate_enabled:
        graph.add_node("debate", debate_node)
        graph.add_edge("combiner", "debate")
        next_after_debate = "decision" if decision_node else END
        graph.add_edge("debate", next_after_debate)
    else:
        next_after_combiner = "decision" if decision_node else END
        graph.add_edge("combiner", next_after_combiner)

    # --- Decision agent ---
    if decision_node is not None:
        graph.add_node("decision", decision_node)
        if risk_node is not None:
            graph.add_node("risk", risk_node)
            graph.add_edge("decision", "risk")
            graph.add_edge("risk", END)
        else:
            graph.add_edge("decision", END)

    return graph.compile()
