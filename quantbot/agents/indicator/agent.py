"""Indicator Agent — interprets technical indicators via LLM."""

from __future__ import annotations

import json
from typing import Any

from langchain_core.messages import HumanMessage, SystemMessage

from quantbot.agents.indicator.tools import compute_all_indicators
from quantbot.agents.shared.llm import get_llm_client, parse_signal_response
from quantbot.config import load_prompt, settings
from quantbot.graph.state import TradingGraphState


def make_indicator_node() -> Any:
    """Create a LangGraph-compatible Indicator Agent node."""

    system_prompt = load_prompt("indicator_system")

    def indicator_node(state: TradingGraphState) -> dict[str, Any]:
        bars = state["bars"]
        instrument = state["instrument"]
        memory_context = state.get("memory_context", "")

        # Compute all technical indicators
        indicators = compute_all_indicators(bars)

        # Build the user message with indicator data
        user_content = f"""## Instrument: {instrument}

## Current Technical Indicators
```json
{json.dumps(indicators, indent=2)}
```

## How to Read These Indicators
- **trend.trend_regime**: The current trend direction based on SMA slopes and price position
- **Momentum (MACD histogram, ROC)**: Positive = bullish momentum, negative = bearish. These CONFIRM trends.
- **Oscillators (RSI, Stochastic, Williams %R)**: Show overbought/oversold. In a trend, these confirm strength — only signal exhaustion when they DIVERGE from price.

Assess whether the indicators confirm or contradict the current trend regime, then produce your signal."""

        if memory_context:
            user_content += f"\n## Decision History\n{memory_context}\n"

        user_content += "\nAnalyze these indicators step-by-step and produce your signal."

        llm = get_llm_client(settings.indicator_model)
        messages = [
            SystemMessage(content=system_prompt),
            HumanMessage(content=user_content),
        ]

        response = llm.invoke(messages)
        signal = parse_signal_response(
            response.content, instrument, "Indicator"
        )

        return {"signals": [signal]}

    return indicator_node
