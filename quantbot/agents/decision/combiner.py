"""Confidence-weighted signal ensemble combiner."""

from __future__ import annotations

import numpy as np

from quantbot.config import settings
from quantbot.core.signal import Signal, SignalDirection, SignalType

# Default agent weights — configurable via settings
DEFAULT_WEIGHTS = {
    "TSMOM": settings.weight_tsmom,
    "Indicator": settings.weight_indicator,
    "Pattern": settings.weight_pattern,
    "Trend": settings.weight_trend,
}

CONVICTION_THRESHOLD = 0.10  # |combined| < this → FLAT


class SignalCombiner:
    """Combine signals from multiple agents using confidence-weighted ensemble.

    Formula:
        combined = Σ(weight_i × strength_i × confidence_i) / Σ(weight_i × confidence_i)

    This weights each agent's contribution by both its assigned importance
    AND its self-reported confidence. An agent that is uncertain has less
    influence even if its weight is high.
    """

    def __init__(self, weights: dict[str, float] | None = None) -> None:
        self.weights = weights or DEFAULT_WEIGHTS

    def combine(self, signals: list[Signal]) -> Signal:
        """Combine multiple signals into a single ensemble signal.

        Args:
            signals: List of signals from different agents.

        Returns:
            A combined Signal with direction, strength, and confidence.
        """
        if not signals:
            return Signal(
                instrument="UNKNOWN",
                direction=SignalDirection.FLAT,
                strength=0.0,
                confidence=0.0,
                agent_name="Combiner",
                signal_type=SignalType.COMBINED,
                metadata={"reason": "no signals received"},
            )

        instrument = signals[0].instrument

        numerator = 0.0
        denominator = 0.0
        agent_contributions: dict[str, float] = {}

        for sig in signals:
            w = self.weights.get(sig.agent_name, 0.10)  # default weight for unknown agents
            contribution = w * sig.strength * sig.confidence
            weight_sum = w * sig.confidence

            numerator += contribution
            denominator += weight_sum
            agent_contributions[sig.agent_name] = contribution

        if denominator < 1e-8:
            combined_strength = 0.0
        else:
            combined_strength = numerator / denominator

        # Clamp to [-1, 1]
        combined_strength = float(np.clip(combined_strength, -1.0, 1.0))

        # Confidence = normalized total weight (how much input we had)
        max_possible_denom = sum(
            self.weights.get(s.agent_name, 0.10) * 1.0 for s in signals
        )
        combined_confidence = denominator / max_possible_denom if max_possible_denom > 0 else 0.0
        combined_confidence = min(1.0, combined_confidence)

        # Apply conviction threshold
        if abs(combined_strength) < CONVICTION_THRESHOLD:
            direction = SignalDirection.FLAT
            combined_strength = 0.0
        elif combined_strength > 0:
            direction = SignalDirection.LONG
        else:
            direction = SignalDirection.SHORT

        return Signal(
            instrument=instrument,
            direction=direction,
            strength=combined_strength,
            confidence=combined_confidence,
            agent_name="Combiner",
            signal_type=SignalType.COMBINED,
            metadata={
                "agent_contributions": agent_contributions,
                "raw_numerator": numerator,
                "raw_denominator": denominator,
                "conviction_threshold": CONVICTION_THRESHOLD,
            },
        )
