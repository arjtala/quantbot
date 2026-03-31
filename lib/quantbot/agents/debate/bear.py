"""Bear Advocate — argues the SHORT case in structured debate."""

from __future__ import annotations

import json
from typing import Any

from langchain_core.messages import HumanMessage, SystemMessage

from quantbot.agents.shared.llm import get_llm_client
from quantbot.config import load_prompt, settings
from quantbot.core.signal import Signal


def invoke_bear_advocate(
    instrument: str,
    signals: list[Signal],
    bars_summary: str,
) -> dict[str, Any]:
    """Run the bear advocate and return structured arguments.

    Args:
        instrument: The instrument being debated.
        signals: All signals collected from the fan-out phase.
        bars_summary: Text summary of recent price action.

    Returns:
        Dict with thesis, evidence, conviction, etc.
    """
    system_prompt = load_prompt("bear_advocate")

    signal_text = "\n".join(
        f"- {s.agent_name}: {s.direction.value} (strength={s.strength:.2f}, confidence={s.confidence:.2f})"
        for s in signals
    )

    user_content = f"""## Instrument: {instrument}

## Current Agent Signals
{signal_text}

## Recent Price Action
{bars_summary}

Argue the strongest possible SHORT case for {instrument}.
"""

    llm = get_llm_client(settings.debate_model)
    messages = [
        SystemMessage(content=system_prompt),
        HumanMessage(content=user_content),
    ]

    response = llm.invoke(messages)

    try:
        text = response.content.strip()
        if text.startswith("```"):
            lines = text.split("\n")
            lines = [l for l in lines if not l.strip().startswith("```")]
            text = "\n".join(lines)
        return json.loads(text)
    except (json.JSONDecodeError, ValueError):
        return {
            "thesis": "Unable to parse bear argument",
            "supporting_evidence": [],
            "conviction": 0.0,
            "raw_response": response.content[:500],
        }
