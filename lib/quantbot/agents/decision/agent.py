"""Decision Agent — final trading decision synthesis via LLM."""

from __future__ import annotations

import json
from typing import Any

from langchain_core.messages import HumanMessage, SystemMessage

from quantbot.agents.decision.combiner import SignalCombiner
from quantbot.agents.shared.llm import get_llm_client, parse_signal_response
from quantbot.config import load_prompt, settings
from quantbot.graph.state import TradingGraphState


def make_combiner_node(combiner: SignalCombiner | None = None) -> Any:
    """Create a LangGraph node that runs the SignalCombiner."""
    if combiner is None:
        combiner = SignalCombiner()

    def combiner_node(state: TradingGraphState) -> dict[str, Any]:
        signals = state.get("signals", [])
        combined = combiner.combine(signals)
        return {
            "signals": [combined],
            "metadata": {"combined_signal": {
                "direction": combined.direction.value,
                "strength": combined.strength,
                "confidence": combined.confidence,
            }},
        }

    return combiner_node


def make_decision_node() -> Any:
    """Create a LangGraph-compatible Decision Agent node."""

    system_prompt = load_prompt("decision_system")

    def decision_node(state: TradingGraphState) -> dict[str, Any]:
        signals = state.get("signals", [])
        debate = state.get("debate", {})
        memory_context = state.get("memory_context", "")
        instrument = state["instrument"]

        # Build signal summary for the LLM
        signal_summary = []
        for sig in signals:
            signal_summary.append({
                "agent": sig.agent_name,
                "direction": sig.direction.value,
                "strength": round(sig.strength, 3),
                "confidence": round(sig.confidence, 3),
            })

        user_content = f"""## Instrument: {instrument}

## Signals Received
```json
{json.dumps(signal_summary, indent=2)}
```
"""

        if debate:
            bull = debate.get("bull", {})
            bear = debate.get("bear", {})
            user_content += f"""
## Bull/Bear Debate
### Bull Case (conviction: {debate.get('bull_conviction', 0):.2f})
Thesis: {bull.get('thesis', 'N/A')}
Evidence: {json.dumps(bull.get('supporting_evidence', []))}

### Bear Case (conviction: {debate.get('bear_conviction', 0):.2f})
Thesis: {bear.get('thesis', 'N/A')}
Evidence: {json.dumps(bear.get('supporting_evidence', []))}
"""

        if memory_context:
            user_content += f"\n## Decision History\n{memory_context}\n"

        user_content += "\nSynthesize all inputs and produce your final trading decision."

        llm = get_llm_client(settings.decision_model)
        messages = [
            SystemMessage(content=system_prompt),
            HumanMessage(content=user_content),
        ]

        response = llm.invoke(messages)
        signal = parse_signal_response(
            response.content, instrument, "Decision"
        )

        return {
            "decision": {
                "direction": signal.direction.value,
                "strength": signal.strength,
                "confidence": signal.confidence,
                "reasoning": signal.metadata.get("reasoning", ""),
            },
            "signals": [signal],
        }

    return decision_node
