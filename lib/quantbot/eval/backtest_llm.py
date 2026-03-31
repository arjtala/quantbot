"""Evaluate LLM agents against historical data.

Runs agents on historical bars day-by-day, records predicted direction
vs actual next-day return, and computes accuracy metrics.
"""

from __future__ import annotations

import logging
from dataclasses import dataclass, field
from datetime import date

import numpy as np
import pandas as pd

from quantbot.agents.indicator.tools import compute_all_indicators
from quantbot.agents.shared.llm import parse_signal_response
from quantbot.core.signal import Signal, SignalDirection
from quantbot.data.yahoo import YahooProvider
from quantbot.memory.store import MemoryStore

logger = logging.getLogger(__name__)


@dataclass
class LLMBacktestResult:
    """Results from evaluating LLM agents on historical data."""

    predictions: list[dict]
    total: int = 0
    correct: int = 0
    accuracy: float = 0.0
    long_accuracy: float = 0.0
    short_accuracy: float = 0.0
    flat_count: int = 0

    @classmethod
    def from_predictions(cls, predictions: list[dict]) -> LLMBacktestResult:
        total = len([p for p in predictions if p["predicted"] != "FLAT"])
        correct = len([p for p in predictions if p.get("correct", False)])
        flat_count = len([p for p in predictions if p["predicted"] == "FLAT"])

        longs = [p for p in predictions if p["predicted"] == "LONG"]
        shorts = [p for p in predictions if p["predicted"] == "SHORT"]

        long_correct = len([p for p in longs if p.get("correct", False)])
        short_correct = len([p for p in shorts if p.get("correct", False)])

        return cls(
            predictions=predictions,
            total=total,
            correct=correct,
            accuracy=correct / total if total > 0 else 0.0,
            long_accuracy=long_correct / len(longs) if longs else 0.0,
            short_accuracy=short_correct / len(shorts) if shorts else 0.0,
            flat_count=flat_count,
        )

    def summary(self) -> str:
        return (
            f"LLM Backtest: {self.correct}/{self.total} correct "
            f"({self.accuracy:.1%} accuracy)\n"
            f"  LONG accuracy: {self.long_accuracy:.1%}\n"
            f"  SHORT accuracy: {self.short_accuracy:.1%}\n"
            f"  FLAT signals: {self.flat_count}"
        )


def evaluate_agent_accuracy(
    agent_fn,
    instrument: str,
    start: date,
    end: date,
    min_history: int = 60,
) -> LLMBacktestResult:
    """Run an agent function over historical data and measure directional accuracy.

    Args:
        agent_fn: Callable(bars, instrument) -> Signal
        instrument: Ticker symbol.
        start: Backtest start date.
        end: Backtest end date.
        min_history: Minimum bars before first prediction.

    Returns:
        LLMBacktestResult with accuracy metrics.
    """
    provider = YahooProvider()
    bars = provider.fetch_bars(instrument, start, end)

    predictions = []

    for i in range(min_history, len(bars) - 1):
        history = bars.iloc[: i + 1]
        next_return = float(bars["Close"].iloc[i + 1] / bars["Close"].iloc[i] - 1)

        try:
            signal = agent_fn(history, instrument)
        except Exception as e:
            logger.warning("Agent error at %s: %s", bars.index[i], e)
            continue

        predicted = signal.direction.value
        actual = "LONG" if next_return > 0 else "SHORT"

        correct = False
        if predicted == "FLAT":
            correct = False  # FLAT doesn't count as correct or incorrect
        elif predicted == actual:
            correct = True

        predictions.append({
            "date": str(bars.index[i].date()),
            "predicted": predicted,
            "actual": actual,
            "next_return": next_return,
            "strength": signal.strength,
            "confidence": signal.confidence,
            "correct": correct,
        })

    return LLMBacktestResult.from_predictions(predictions)
