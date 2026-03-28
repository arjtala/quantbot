"""Signal types for inter-agent communication."""

from __future__ import annotations

from dataclasses import dataclass, field
from datetime import UTC, datetime
from enum import Enum
from typing import Any


class SignalDirection(Enum):
    LONG = "LONG"
    SHORT = "SHORT"
    FLAT = "FLAT"


class SignalType(Enum):
    QUANT = "QUANT"
    LLM = "LLM"
    COMBINED = "COMBINED"


@dataclass(frozen=True)
class Signal:
    """Unified signal produced by every agent.

    Attributes:
        instrument: Ticker / identifier the signal applies to.
        direction: LONG, SHORT, or FLAT.
        strength: Continuous value in [-1, 1]. Sign matches direction.
        confidence: How confident the agent is in [0, 1].
        agent_name: Which agent produced the signal.
        signal_type: QUANT, LLM, or COMBINED.
        horizon_days: Expected holding period in trading days.
        timestamp: When the signal was generated.
        metadata: Arbitrary extra info (lookback returns, vol estimate, etc.).
    """

    instrument: str
    direction: SignalDirection
    strength: float  # [-1, 1]
    confidence: float  # [0, 1]
    agent_name: str
    signal_type: SignalType
    horizon_days: int = 21
    timestamp: datetime = field(default_factory=lambda: datetime.now(UTC))
    metadata: dict[str, Any] = field(default_factory=dict)

    def __post_init__(self) -> None:
        if not -1.0 <= self.strength <= 1.0:
            raise ValueError(f"strength must be in [-1, 1], got {self.strength}")
        if not 0.0 <= self.confidence <= 1.0:
            raise ValueError(f"confidence must be in [0, 1], got {self.confidence}")

    @property
    def sized_strength(self) -> float:
        """Strength weighted by confidence."""
        return self.strength * self.confidence
