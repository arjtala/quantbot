"""Pattern Agent — visual chart pattern recognition via vision LLM."""

from __future__ import annotations

from typing import Any

from langchain_core.messages import HumanMessage, SystemMessage

from quantbot.agents.pattern.charts import render_pattern_chart
from quantbot.agents.shared.llm import (
    get_llm_client,
    image_to_base64,
    parse_signal_response,
)
from quantbot.config import load_prompt, settings
from quantbot.graph.state import TradingGraphState


def make_pattern_node() -> Any:
    """Create a LangGraph-compatible Pattern Agent node."""

    system_prompt = load_prompt("pattern_system")

    def pattern_node(state: TradingGraphState) -> dict[str, Any]:
        bars = state["bars"]
        instrument = state["instrument"]
        memory_context = state.get("memory_context", "")

        # Render chart to BytesIO
        chart_buf = render_pattern_chart(bars, instrument)
        chart_b64 = image_to_base64(chart_buf)

        # Build multimodal message
        text_content = f"## Instrument: {instrument}\n\nAnalyze the candlestick chart below for chart patterns."
        if memory_context:
            text_content += f"\n\n## Decision History\n{memory_context}"
        text_content += "\n\nIdentify patterns step-by-step and produce your signal."

        llm = get_llm_client(settings.pattern_model)
        messages = [
            SystemMessage(content=system_prompt),
            HumanMessage(
                content=[
                    {"type": "text", "text": text_content},
                    {"type": "image_url", "image_url": {"url": chart_b64}},
                ]
            ),
        ]

        response = llm.invoke(messages)
        signal = parse_signal_response(
            response.content, instrument, "Pattern"
        )

        return {"signals": [signal]}

    return pattern_node
