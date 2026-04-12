"""Minimal stub for a future Kronos sidecar / offline batch worker.

This script does not run Kronos yet. It documents the canonical request/response
shape that the Rust scaffold expects when forecast summaries are inserted into
SQLite `forecast_cache`.
"""

from __future__ import annotations

import json
from dataclasses import asdict, dataclass, field
from typing import Any


@dataclass
class ReturnSummary:
    mean: float
    median: float
    std: float
    p05: float
    p25: float
    p75: float
    p95: float


@dataclass
class ProbabilitySummary:
    return_lt_0: float | None = None
    return_lt_neg_1pct: float | None = None
    return_lt_neg_2pct: float | None = None
    return_lt_neg_5pct: float | None = None
    return_gt_1pct: float | None = None
    extra: dict[str, float] = field(default_factory=dict)


@dataclass
class DistributionSummary:
    iqr: float
    tail_width_90: float


@dataclass
class ForecastDiagnostics:
    input_truncated: bool = False
    sampling_temperature: float | None = None
    top_p: float | None = None
    notes: str | None = None


@dataclass
class ForecastSummary:
    instrument: str
    eval_date: str
    horizon_days: int
    lookback_bars: int
    sample_count: int
    target_field: str
    forecast_return: ReturnSummary
    probabilities: ProbabilitySummary
    distribution: DistributionSummary
    diagnostics: ForecastDiagnostics


def build_example_payload() -> dict[str, Any]:
    summary = ForecastSummary(
        instrument="SPY",
        eval_date="2025-03-31",
        horizon_days=5,
        lookback_bars=512,
        sample_count=64,
        target_field="close",
        forecast_return=ReturnSummary(
            mean=-0.0042,
            median=-0.0031,
            std=0.0185,
            p05=-0.0360,
            p25=-0.0120,
            p75=0.0068,
            p95=0.0215,
        ),
        probabilities=ProbabilitySummary(
            return_lt_0=0.63,
            return_lt_neg_1pct=0.41,
            return_lt_neg_2pct=0.22,
            return_gt_1pct=0.17,
        ),
        distribution=DistributionSummary(iqr=0.0188, tail_width_90=0.0575),
        diagnostics=ForecastDiagnostics(
            input_truncated=False,
            sampling_temperature=1.0,
            top_p=0.9,
            notes="stub payload only; replace with Kronos inference",
        ),
    )

    return {
        "status": "ok",
        "model_name": "NeoQuasar/Kronos-mini",
        "model_version": "v1",
        "instrument": summary.instrument,
        "eval_date": summary.eval_date,
        "horizon_days": summary.horizon_days,
        "lookback_bars": summary.lookback_bars,
        "input_hash": "replace-with-real-hash",
        "sample_count": summary.sample_count,
        "summary": asdict(summary),
        "latency_ms": 123,
    }


if __name__ == "__main__":
    print(json.dumps(build_example_payload(), indent=2))
