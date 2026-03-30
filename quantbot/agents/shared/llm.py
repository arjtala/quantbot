"""Shared LLM utilities for agent signal extraction."""

from __future__ import annotations

import base64
import io
import json
import logging
import re
from typing import Any

from quantbot.config import settings
from quantbot.core.signal import Signal, SignalDirection, SignalType

logger = logging.getLogger(__name__)

# Regex to match <think>...</think> blocks (reasoning models like DeepSeek-R1)
_THINK_RE = re.compile(r"<think>(.*?)</think>", re.DOTALL)


def _extract_thinking(text: str) -> tuple[str, str]:
    """Strip <think> blocks from text, returning (reasoning, cleaned_text)."""
    reasoning_parts: list[str] = []
    for m in _THINK_RE.finditer(text):
        reasoning_parts.append(m.group(1).strip())
    cleaned = _THINK_RE.sub("", text).strip()
    return "\n\n".join(reasoning_parts), cleaned


def _extract_json(text: str) -> dict[str, Any]:
    """Extract a JSON object from text that may contain markdown fences or prose."""
    text = text.strip()

    # Strip markdown code fences if present
    if text.startswith("```"):
        lines = text.split("\n")
        lines = [l for l in lines if not l.strip().startswith("```")]
        text = "\n".join(lines).strip()

    # Try direct parse first (fastest path)
    try:
        return json.loads(text)
    except json.JSONDecodeError:
        pass

    # Fall back: find JSON by matching braces
    start = text.find("{")
    if start == -1:
        raise json.JSONDecodeError("No JSON object found in response", text, 0)

    depth = 0
    for i in range(start, len(text)):
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                return json.loads(text[start : i + 1])

    raise json.JSONDecodeError("No JSON object found in response", text, 0)


def parse_signal_response(
    raw: str,
    instrument: str,
    agent_name: str,
    signal_type: SignalType = SignalType.LLM,
) -> Signal:
    """Parse an LLM JSON response into a Signal.

    Handles common extraction issues:
    - <think>...</think> reasoning blocks (DeepSeek-R1, QwQ, etc.)
    - Markdown code fences
    - JSON embedded in prose text

    Falls back to FLAT signal on parse failure.
    """
    try:
        # Extract and preserve reasoning from <think> tags
        llm_reasoning, text = _extract_thinking(raw)

        data = _extract_json(text)

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
        if llm_reasoning:
            metadata["llm_reasoning"] = llm_reasoning
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
        logger.debug("Raw LLM response:\n%s", raw[:2000])
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
      - "ollama:<model>"  → Ollama (local Mac)
      - "sglang:<model>"  → Custom OpenAI-compatible endpoint (SGLang/vLLM on GPU cluster)
      - "claude*"         → Anthropic API
      - "gpt*"/"o1*"/"o3" → OpenAI API
      - anything else     → default_provider setting (sglang or ollama)
    """
    # Explicit prefix routing
    if model.startswith("ollama:"):
        return _get_ollama_client(model[len("ollama:"):])

    if model.startswith("sglang:"):
        return _get_sglang_client(model[len("sglang:"):])

    # Route by model name
    if model.startswith("claude"):
        from langchain_anthropic import ChatAnthropic
        return ChatAnthropic(
            model=model,
            api_key=settings.anthropic_api_key,
            max_tokens=2048,
        )
    elif model.startswith("gpt") or model.startswith("o1") or model.startswith("o3"):
        from langchain_openai import ChatOpenAI
        kwargs: dict[str, Any] = {
            "model": model,
            "max_tokens": 2048,
        }
        # If custom base_url is set, use it (SGLang/vLLM served as OpenAI-compatible)
        if settings.openai_base_url:
            kwargs["base_url"] = settings.openai_base_url
            kwargs["api_key"] = settings.openai_api_key or "not-needed"
        else:
            kwargs["api_key"] = settings.openai_api_key
        return ChatOpenAI(**kwargs)
    else:
        # No prefix match — use default_provider setting as fallback
        if settings.default_provider == "sglang":
            return _get_sglang_client(model)
        else:
            return _get_ollama_client(model)


def _get_ollama_client(model: str) -> Any:
    """Create an Ollama LangChain client."""
    from langchain_ollama import ChatOllama
    return ChatOllama(
        model=model,
        base_url=settings.ollama_base_url,
        temperature=0.2,
    )


def _get_sglang_client(model: str) -> Any:
    """Create a client for SGLang/vLLM via OpenAI-compatible API.

    SGLang serves models at an OpenAI-compatible /v1/chat/completions endpoint.
    We use ChatOpenAI with a custom base_url.
    """
    from langchain_openai import ChatOpenAI

    base_url = settings.openai_base_url
    if not base_url:
        raise ValueError(
            "OPENAI_BASE_URL must be set for sglang: models "
            "(e.g., http://slurm-node:30000/v1)"
        )

    return ChatOpenAI(
        model=model,
        base_url=base_url,
        api_key=settings.openai_api_key or "not-needed",
        max_tokens=2048,
        temperature=0.2,
    )
