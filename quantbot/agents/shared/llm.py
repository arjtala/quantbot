"""Shared LLM utilities for agent signal extraction."""

from __future__ import annotations

import base64
import io
import json
import logging
from typing import Any

from quantbot.config import settings
from quantbot.core.signal import Signal, SignalDirection, SignalType

logger = logging.getLogger(__name__)


def parse_signal_response(
    raw: str,
    instrument: str,
    agent_name: str,
    signal_type: SignalType = SignalType.LLM,
) -> Signal:
    """Parse an LLM JSON response into a Signal.

    Handles common extraction issues (markdown code fences, trailing text).
    Falls back to FLAT signal on parse failure.
    """
    try:
        # Strip markdown code fences if present
        text = raw.strip()
        if text.startswith("```"):
            lines = text.split("\n")
            # Remove first and last fence lines
            lines = [l for l in lines if not l.strip().startswith("```")]
            text = "\n".join(lines)

        data = json.loads(text)

        direction_str = data.get("direction", "FLAT").upper()
        direction = SignalDirection(direction_str)
        strength = float(data.get("strength", 0.0))
        confidence = float(data.get("confidence", 0.0))
        horizon = int(data.get("horizon_days", 21))

        # Clamp values
        strength = max(-1.0, min(1.0, strength))
        confidence = max(0.0, min(1.0, confidence))

        # Extract reasoning and extra fields for metadata
        metadata: dict[str, Any] = {}
        for key in ("reasoning", "patterns_identified", "trend_direction", "key_levels"):
            if key in data:
                metadata[key] = data[key]

        return Signal(
            instrument=instrument,
            direction=direction,
            strength=strength,
            confidence=confidence,
            agent_name=agent_name,
            signal_type=signal_type,
            horizon_days=horizon,
            metadata=metadata,
        )

    except (json.JSONDecodeError, KeyError, ValueError) as e:
        logger.warning("Failed to parse LLM response for %s/%s: %s", agent_name, instrument, e)
        return Signal(
            instrument=instrument,
            direction=SignalDirection.FLAT,
            strength=0.0,
            confidence=0.0,
            agent_name=agent_name,
            signal_type=signal_type,
            metadata={"parse_error": str(e), "raw_response": raw[:500]},
        )


def image_to_base64(buf: io.BytesIO) -> str:
    """Convert a BytesIO image buffer to a base64-encoded data URI."""
    buf.seek(0)
    b64 = base64.b64encode(buf.read()).decode("utf-8")
    return f"data:image/png;base64,{b64}"


def get_llm_client(model: str) -> Any:
    """Get the appropriate LangChain LLM client based on model name.

    Routing:
      - "ollama:" prefix or settings.default_provider == "ollama" → Ollama (local)
      - "claude" prefix → Anthropic API
      - anything else → OpenAI API
    """
    # Explicit ollama prefix: "ollama:qwen3:14b" → model = "qwen3:14b"
    if model.startswith("ollama:"):
        ollama_model = model[len("ollama:"):]
        return _get_ollama_client(ollama_model)

    # Route by model name or default provider
    if model.startswith("claude"):
        from langchain_anthropic import ChatAnthropic
        return ChatAnthropic(
            model=model,
            api_key=settings.anthropic_api_key,
            max_tokens=2048,
        )
    elif model.startswith("gpt") or model.startswith("o1") or model.startswith("o3"):
        from langchain_openai import ChatOpenAI
        return ChatOpenAI(
            model=model,
            api_key=settings.openai_api_key,
            max_tokens=2048,
        )
    else:
        # Default: treat as Ollama model name
        return _get_ollama_client(model)


def _get_ollama_client(model: str) -> Any:
    """Create an Ollama LangChain client."""
    from langchain_ollama import ChatOllama
    return ChatOllama(
        model=model,
        base_url=settings.ollama_base_url,
        temperature=0.2,
    )
