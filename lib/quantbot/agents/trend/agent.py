"""Trend Agent — support/resistance analysis via annotated chart + LLM."""

from __future__ import annotations

import json
from typing import Any

from langchain_core.messages import HumanMessage, SystemMessage

from quantbot.agents.shared.chart_renderer import render_candlestick
from quantbot.agents.shared.llm import (
    get_llm_client,
    image_to_base64,
    parse_signal_response,
)
from quantbot.agents.trend.trendlines import find_support_resistance, fit_trendline
from quantbot.config import load_prompt, settings
from quantbot.graph.state import TradingGraphState


def make_trend_node() -> Any:
    """Create a LangGraph-compatible Trend Agent node."""

    system_prompt = load_prompt("trend_system")

    def trend_node(state: TradingGraphState) -> dict[str, Any]:
        bars = state["bars"]
        instrument = state["instrument"]
        memory_context = state.get("memory_context", "")

        # Compute support/resistance and trendlines
        levels = find_support_resistance(bars)
        support_tl = fit_trendline(bars, kind="support")
        resistance_tl = fit_trendline(bars, kind="resistance")

        trendlines = []
        if support_tl:
            trendlines.append(support_tl)
        if resistance_tl:
            trendlines.append(resistance_tl)

        # Render annotated chart
        chart_buf = render_candlestick(
            bars=bars,
            title=f"{instrument} — Trend Analysis",
            support_levels=levels["support"],
            resistance_levels=levels["resistance"],
            trendlines=trendlines,
            show_volume=True,
            last_n=60,
        )
        chart_b64 = image_to_base64(chart_buf)

        # Build message with both visual and numeric data
        text_content = f"""## Instrument: {instrument}

## Detected Levels
```json
{json.dumps(levels, indent=2)}
```

## Trendline Analysis
- Support trendline slope: {support_tl['slope']:.4f} per bar ({"rising" if support_tl and support_tl['slope'] > 0 else "falling"})
- Resistance trendline slope: {resistance_tl['slope']:.4f} per bar ({"rising" if resistance_tl and resistance_tl['slope'] > 0 else "falling"})

Current price: {float(bars['Close'].iloc[-1]):.2f}
""" if support_tl and resistance_tl else f"""## Instrument: {instrument}

## Detected Levels
```json
{json.dumps(levels, indent=2)}
```

Current price: {float(bars['Close'].iloc[-1]):.2f}
"""

        if memory_context:
            text_content += f"\n## Decision History\n{memory_context}"
        text_content += "\n\nAnalyze the annotated chart and levels step-by-step. Produce your signal."

        llm = get_llm_client(settings.trend_model)
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
            response.content, instrument, "Trend"
        )

        return {"signals": [signal]}

    return trend_node
