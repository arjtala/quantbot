"""Batch-fill quantbot's SQLite forecast_cache with Kronos-style summaries.

This worker is intentionally Python-native because the upstream Kronos project
is Python-first. It is designed to be invoked by Rust so the UX stays native:

    quantbot forecast fill --from ... --to ... --horizon ...

The worker supports two modes:

1. Real Kronos mode (future-facing): if Kronos + pandas are importable and the
   adapter wiring is completed, forecasts can come from the actual model.
2. Stub mode (default fallback): derive a deterministic probabilistic summary
   from trailing historical horizon returns. This keeps the cache/replay loop
   usable even when Kronos is not installed.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import sqlite3
import sys
import time
from dataclasses import asdict, dataclass, field
from datetime import UTC, date, datetime
from pathlib import Path
from typing import Any, Iterable, Sequence

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover
    tomllib = None  # type: ignore[assignment]


@dataclass(frozen=True)
class Bar:
    date: date
    open: float
    high: float
    low: float
    close: float
    volume: float


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


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path, required=True)
    parser.add_argument("--db", type=Path, required=True)
    parser.add_argument("--data-dir", type=Path, required=True)
    parser.add_argument("--from", dest="date_from", required=True)
    parser.add_argument("--to", dest="date_to", required=True)
    parser.add_argument("--horizon", type=int, required=True)
    parser.add_argument("--instruments", required=True)
    parser.add_argument("--model-name")
    parser.add_argument("--model-version")
    parser.add_argument("--tokenizer-name")
    parser.add_argument("--lookback-bars", type=int)
    parser.add_argument("--sample-count", type=int)
    parser.add_argument("--temperature", type=float)
    parser.add_argument("--top-p", type=float)
    parser.add_argument("--target-field")
    parser.add_argument("--progress", action="store_true")
    parser.add_argument(
        "--force-stub",
        action="store_true",
        help="Force deterministic historical-summary mode even if Kronos is importable.",
    )
    return parser.parse_args()


def load_toml_defaults(path: Path) -> dict[str, Any]:
    if not path.exists() or tomllib is None:
        return {}
    with path.open("rb") as f:
        data = tomllib.load(f)
    return ((data.get("overlays") or {}).get("kronos") or {})


def resolve_args(args: argparse.Namespace) -> argparse.Namespace:
    cfg = load_toml_defaults(args.config)
    args.model_name = args.model_name or cfg.get("model_name") or "NeoQuasar/Kronos-mini"
    args.model_version = args.model_version or cfg.get("model_version") or "v1"
    args.tokenizer_name = args.tokenizer_name or infer_tokenizer_name(args.model_name)
    args.lookback_bars = args.lookback_bars or int(cfg.get("lookback_bars") or 512)
    args.sample_count = args.sample_count or int(cfg.get("sample_count") or 64)
    args.temperature = (
        args.temperature if args.temperature is not None else 1.0
    )
    args.top_p = args.top_p if args.top_p is not None else 0.9
    args.target_field = args.target_field or cfg.get("target_field") or "close"
    args.date_from = date.fromisoformat(args.date_from)
    args.date_to = date.fromisoformat(args.date_to)
    args.instruments = [s.strip() for s in args.instruments.split(",") if s.strip()]
    if args.horizon <= 0:
        raise SystemExit("--horizon must be > 0")
    if args.lookback_bars <= 1:
        raise SystemExit("--lookback-bars must be > 1")
    if args.sample_count <= 0:
        raise SystemExit("--sample-count must be > 0")
    return args


def infer_tokenizer_name(model_name: str) -> str:
    if "mini" in model_name.lower():
        return "NeoQuasar/Kronos-Tokenizer-2k"
    return "NeoQuasar/Kronos-Tokenizer-base"


def load_bars(path: Path) -> list[Bar]:
    bars: list[Bar] = []
    with path.open(newline="", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        for row in reader:
            bars.append(
                Bar(
                    date=date.fromisoformat(row["Date"]),
                    open=float(row["Open"]),
                    high=float(row["High"]),
                    low=float(row["Low"]),
                    close=float(row["Close"]),
                    volume=float(row.get("Volume") or 0.0),
                )
            )
    bars.sort(key=lambda b: b.date)
    return bars


def quantile(values: Sequence[float], q: float) -> float:
    if not values:
        raise ValueError("quantile() requires at least one value")
    if len(values) == 1:
        return float(values[0])
    xs = sorted(values)
    pos = (len(xs) - 1) * q
    lo = math.floor(pos)
    hi = math.ceil(pos)
    if lo == hi:
        return float(xs[lo])
    frac = pos - lo
    return float(xs[lo] * (1.0 - frac) + xs[hi] * frac)


def mean(values: Sequence[float]) -> float:
    return sum(values) / len(values)


def stdev(values: Sequence[float]) -> float:
    if len(values) < 2:
        return 0.0
    mu = mean(values)
    var = sum((x - mu) ** 2 for x in values) / (len(values) - 1)
    return math.sqrt(max(var, 0.0))


def downside_prob(values: Sequence[float], threshold: float) -> float:
    return sum(1 for v in values if v < threshold) / len(values)


def trailing_horizon_returns(closes: Sequence[float], horizon: int) -> list[float]:
    if len(closes) <= horizon:
        return []
    out: list[float] = []
    for i in range(len(closes) - horizon):
        start = closes[i]
        end = closes[i + horizon]
        if start <= 0.0:
            continue
        out.append(end / start - 1.0)
    return out


class BaseForecaster:
    mode_name = "base"

    def predict_summary(
        self,
        *,
        instrument: str,
        eval_date: date,
        history: Sequence[Bar],
        horizon_days: int,
        lookback_bars: int,
        sample_count: int,
        temperature: float,
        top_p: float,
        target_field: str,
    ) -> tuple[ForecastSummary, dict[str, Any]]:
        raise NotImplementedError


class StubForecaster(BaseForecaster):
    mode_name = "stub"

    def predict_summary(
        self,
        *,
        instrument: str,
        eval_date: date,
        history: Sequence[Bar],
        horizon_days: int,
        lookback_bars: int,
        sample_count: int,
        temperature: float,
        top_p: float,
        target_field: str,
    ) -> tuple[ForecastSummary, dict[str, Any]]:
        closes = [getattr(bar, target_field) for bar in history]
        horizon_returns = trailing_horizon_returns(closes, horizon_days)
        if not horizon_returns:
            raise ValueError(
                f"not enough trailing data to estimate {horizon_days}d return distribution"
            )

        # Deterministic pseudo-sampling: use the historical distribution directly
        # and tile/truncate to sample_count for a stable raw payload.
        tiled = list(horizon_returns)
        while len(tiled) < sample_count:
            tiled.extend(horizon_returns)
        samples = tiled[:sample_count]

        summary = ForecastSummary(
            instrument=instrument,
            eval_date=eval_date.isoformat(),
            horizon_days=horizon_days,
            lookback_bars=lookback_bars,
            sample_count=sample_count,
            target_field=target_field,
            forecast_return=ReturnSummary(
                mean=mean(horizon_returns),
                median=quantile(horizon_returns, 0.50),
                std=stdev(horizon_returns),
                p05=quantile(horizon_returns, 0.05),
                p25=quantile(horizon_returns, 0.25),
                p75=quantile(horizon_returns, 0.75),
                p95=quantile(horizon_returns, 0.95),
            ),
            probabilities=ProbabilitySummary(
                return_lt_0=downside_prob(horizon_returns, 0.0),
                return_lt_neg_1pct=downside_prob(horizon_returns, -0.01),
                return_lt_neg_2pct=downside_prob(horizon_returns, -0.02),
                return_lt_neg_5pct=downside_prob(horizon_returns, -0.05),
                return_gt_1pct=sum(1 for v in horizon_returns if v > 0.01)
                / len(horizon_returns),
            ),
            distribution=DistributionSummary(
                iqr=quantile(horizon_returns, 0.75) - quantile(horizon_returns, 0.25),
                tail_width_90=quantile(horizon_returns, 0.95)
                - quantile(horizon_returns, 0.05),
            ),
            diagnostics=ForecastDiagnostics(
                input_truncated=len(history) == lookback_bars,
                sampling_temperature=temperature,
                top_p=top_p,
                notes=(
                    "stub_mode: deterministic historical horizon-return distribution; "
                    "replace with Kronos inference when dependencies are installed"
                ),
            ),
        )
        raw_payload = {
            "mode": self.mode_name,
            "sample_count": sample_count,
            "samples": samples,
            "history_count": len(horizon_returns),
        }
        return summary, raw_payload


class KronosForecaster(BaseForecaster):
    mode_name = "kronos"

    def __init__(self) -> None:
        # Deferred imports so the script can run without Kronos installed.
        import pandas as pd  # type: ignore
        import importlib
        import os

        model_module = None
        last_error: Exception | None = None
        kronos_path = os.environ.get("KRONOS_PYTHON_PATH")
        search_paths = []
        if kronos_path:
            search_paths.append(kronos_path)
        search_paths.append(str(Path.cwd()))
        search_paths.append(str(Path.cwd() / "Kronos"))
        search_paths.append(str(Path.cwd() / "third_party" / "Kronos"))

        original_sys_path = list(sys.path)
        try:
            for path in search_paths:
                if path and path not in sys.path:
                    sys.path.insert(0, path)
                try:
                    model_module = importlib.import_module("model")
                    break
                except Exception as exc:  # pragma: no cover
                    last_error = exc
            if model_module is None:
                raise RuntimeError(
                    "Kronos Python modules are not importable; set KRONOS_PYTHON_PATH to the "
                    "Kronos repo root, vendor the repo locally, or use --force-stub."
                ) from last_error
        finally:
            sys.path = original_sys_path

        Kronos = getattr(model_module, "Kronos")
        KronosPredictor = getattr(model_module, "KronosPredictor")
        KronosTokenizer = getattr(model_module, "KronosTokenizer")

        self.pd = pd
        self.Kronos = Kronos
        self.KronosPredictor = KronosPredictor
        self.KronosTokenizer = KronosTokenizer
        self._predictor_cache: dict[tuple[str, str, int], Any] = {}

    def _device(self) -> str:
        import torch  # type: ignore

        return "cuda:0" if torch.cuda.is_available() else "cpu"

    def _get_predictor(
        self, *, model_name: str, tokenizer_name: str, max_context: int
    ) -> Any:
        key = (model_name, tokenizer_name, max_context)
        if key in self._predictor_cache:
            return self._predictor_cache[key]

        tokenizer = self.KronosTokenizer.from_pretrained(tokenizer_name)
        model = self.Kronos.from_pretrained(model_name)
        predictor = self.KronosPredictor(
            model, tokenizer, device=self._device(), max_context=max_context
        )
        self._predictor_cache[key] = predictor
        return predictor

    def predict_summary(
        self,
        *,
        instrument: str,
        eval_date: date,
        history: Sequence[Bar],
        horizon_days: int,
        lookback_bars: int,
        sample_count: int,
        temperature: float,
        top_p: float,
        target_field: str,
    ) -> tuple[ForecastSummary, dict[str, Any]]:
        predictor = self._get_predictor(
            model_name=getattr(self, "model_name"),
            tokenizer_name=getattr(self, "tokenizer_name"),
            max_context=lookback_bars,
        )
        pd = self.pd

        x_df = pd.DataFrame(
            [
                {
                    "open": bar.open,
                    "high": bar.high,
                    "low": bar.low,
                    "close": bar.close,
                    "volume": bar.volume,
                }
                for bar in history
            ]
        )
        x_timestamp = pd.Series(pd.to_datetime([bar.date.isoformat() for bar in history]))
        future_dates = pd.bdate_range(
            start=pd.Timestamp(eval_date.isoformat()) + pd.offsets.BDay(1),
            periods=horizon_days,
        )
        y_timestamp = pd.Series(future_dates)

        pred_df = predictor.predict(
            df=x_df,
            x_timestamp=x_timestamp,
            y_timestamp=y_timestamp,
            pred_len=horizon_days,
            T=temperature,
            top_p=top_p,
            sample_count=sample_count,
        )

        if target_field not in pred_df.columns:
            raise ValueError(
                f"Kronos prediction missing target field '{target_field}'; got columns {list(pred_df.columns)}"
            )

        last_value = float(getattr(history[-1], target_field))
        if last_value <= 0.0:
            raise ValueError("last target value must be > 0 for return normalization")
        target_values = [float(v) for v in pred_df[target_field].tolist()]
        if not target_values:
            raise ValueError("Kronos prediction returned no rows")

        final_return = target_values[-1] / last_value - 1.0
        samples = [final_return] * sample_count

        summary = ForecastSummary(
            instrument=instrument,
            eval_date=eval_date.isoformat(),
            horizon_days=horizon_days,
            lookback_bars=lookback_bars,
            sample_count=sample_count,
            target_field=target_field,
            forecast_return=ReturnSummary(
                mean=final_return,
                median=final_return,
                std=0.0,
                p05=final_return,
                p25=final_return,
                p75=final_return,
                p95=final_return,
            ),
            probabilities=ProbabilitySummary(
                return_lt_0=1.0 if final_return < 0.0 else 0.0,
                return_lt_neg_1pct=1.0 if final_return < -0.01 else 0.0,
                return_lt_neg_2pct=1.0 if final_return < -0.02 else 0.0,
                return_lt_neg_5pct=1.0 if final_return < -0.05 else 0.0,
                return_gt_1pct=1.0 if final_return > 0.01 else 0.0,
            ),
            distribution=DistributionSummary(iqr=0.0, tail_width_90=0.0),
            diagnostics=ForecastDiagnostics(
                input_truncated=len(history) == lookback_bars,
                sampling_temperature=temperature,
                top_p=top_p,
                notes="kronos_mode: point forecast adapted into canonical summary",
            ),
        )
        raw_payload = {
            "mode": self.mode_name,
            "predicted_rows": len(pred_df),
            "target_values": target_values,
            "sample_count": sample_count,
            "warning": (
                "Upstream Kronos predict() returns averaged forecasts; canonical distribution "
                "summary is currently approximated from the point forecast until direct sample "
                "path extraction is wired."
            ),
        }
        return summary, raw_payload


def select_forecaster(force_stub: bool) -> BaseForecaster:
    if force_stub:
        return StubForecaster()
    try:
        return KronosForecaster()
    except Exception:
        return StubForecaster()


def compute_input_hash(
    *,
    instrument: str,
    history: Sequence[Bar],
    horizon_days: int,
    lookback_bars: int,
    target_field: str,
) -> str:
    payload = {
        "instrument": instrument,
        "horizon_days": horizon_days,
        "lookback_bars": lookback_bars,
        "target_field": target_field,
        "history": [
            {
                "date": b.date.isoformat(),
                "open": b.open,
                "high": b.high,
                "low": b.low,
                "close": b.close,
                "volume": b.volume,
            }
            for b in history
        ],
    }
    raw = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(raw).hexdigest()[:16]


def compute_cache_key(
    *,
    model_name: str,
    model_version: str,
    tokenizer_name: str,
    instrument: str,
    eval_date: date,
    horizon_days: int,
    lookback_bars: int,
    sample_count: int,
    temperature: float,
    top_p: float,
    target_field: str,
    input_hash: str,
) -> str:
    return (
        f"{model_name}|{model_version}|{tokenizer_name}|{instrument}|{eval_date.isoformat()}"
        f"|h{horizon_days}|lb{lookback_bars}|n{sample_count}|T{temperature:.3f}"
        f"|P{top_p:.3f}|{target_field}|{input_hash}"
    )


def ensure_parent_dir(path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)


def ensure_forecast_cache_table(conn: sqlite3.Connection) -> None:
    conn.execute(
        """
        CREATE TABLE IF NOT EXISTS forecast_cache (
            cache_key           TEXT PRIMARY KEY,
            model_name          TEXT NOT NULL,
            model_version       TEXT NOT NULL,
            tokenizer_name      TEXT,
            instrument          TEXT NOT NULL,
            eval_date           TEXT NOT NULL,
            horizon_days        INTEGER NOT NULL,
            lookback_bars       INTEGER NOT NULL,
            input_hash          TEXT NOT NULL,
            sample_count        INTEGER NOT NULL,
            temperature         REAL,
            top_p               REAL,
            target_field        TEXT,
            status              TEXT NOT NULL,
            forecast_json       TEXT NOT NULL,
            raw_response_json   TEXT,
            error_text          TEXT,
            latency_ms          INTEGER,
            created_at          TEXT NOT NULL
        )
        """
    )
    conn.execute(
        """
        CREATE INDEX IF NOT EXISTS idx_forecast_cache_instr_date
        ON forecast_cache(instrument, eval_date)
        """
    )
    conn.execute(
        """
        CREATE INDEX IF NOT EXISTS idx_forecast_cache_model
        ON forecast_cache(model_name, model_version, tokenizer_name, horizon_days)
        """
    )
    conn.execute(
        """
        CREATE INDEX IF NOT EXISTS idx_forecast_cache_status
        ON forecast_cache(status)
        """
    )


def get_existing_status(conn: sqlite3.Connection, cache_key: str) -> str | None:
    row = conn.execute(
        "SELECT status FROM forecast_cache WHERE cache_key = ?",
        (cache_key,),
    ).fetchone()
    if row is None:
        return None
    return str(row[0])


def delete_existing(conn: sqlite3.Connection, cache_key: str) -> None:
    conn.execute("DELETE FROM forecast_cache WHERE cache_key = ?", (cache_key,))


def table_columns(conn: sqlite3.Connection, table: str) -> set[str]:
    rows = conn.execute(f"PRAGMA table_info({table})").fetchall()
    return {str(r[1]) for r in rows}


def insert_forecast_row(conn: sqlite3.Connection, row: dict[str, Any]) -> None:
    cols = table_columns(conn, "forecast_cache")
    payload = {k: v for k, v in row.items() if k in cols}
    sql = (
        f"INSERT OR IGNORE INTO forecast_cache ({', '.join(payload.keys())}) "
        f"VALUES ({', '.join(['?'] * len(payload))})"
    )
    conn.execute(sql, tuple(payload.values()))


def forecast_row(
    *,
    cache_key: str,
    model_name: str,
    model_version: str,
    tokenizer_name: str,
    instrument: str,
    eval_date: date,
    horizon_days: int,
    lookback_bars: int,
    input_hash: str,
    sample_count: int,
    temperature: float,
    top_p: float,
    target_field: str,
    status: str,
    forecast_json: str,
    raw_response_json: str | None,
    error_text: str | None,
    latency_ms: int | None,
) -> dict[str, Any]:
    return {
        "cache_key": cache_key,
        "model_name": model_name,
        "model_version": model_version,
        "tokenizer_name": tokenizer_name,
        "instrument": instrument,
        "eval_date": eval_date.isoformat(),
        "horizon_days": horizon_days,
        "lookback_bars": lookback_bars,
        "input_hash": input_hash,
        "sample_count": sample_count,
        "temperature": temperature,
        "top_p": top_p,
        "target_field": target_field,
        "status": status,
        "forecast_json": forecast_json,
        "raw_response_json": raw_response_json,
        "error_text": error_text,
        "latency_ms": latency_ms,
        "created_at": datetime.now(UTC).isoformat(),
    }


def eligible_eval_indices(
    bars: Sequence[Bar], date_from: date, date_to: date, lookback_bars: int
) -> Iterable[int]:
    for idx, bar in enumerate(bars):
        if bar.date < date_from or bar.date > date_to:
            continue
        if idx + 1 < lookback_bars:
            continue
        yield idx


def main() -> int:
    args = resolve_args(parse_args())
    ensure_parent_dir(args.db)
    forecaster = select_forecaster(args.force_stub)
    if isinstance(forecaster, KronosForecaster):
        forecaster.model_name = args.model_name
        forecaster.tokenizer_name = args.tokenizer_name

    conn = sqlite3.connect(args.db)
    conn.execute("PRAGMA journal_mode=WAL;")
    conn.execute("PRAGMA busy_timeout=5000;")
    ensure_forecast_cache_table(conn)

    total = 0
    inserted = 0
    skipped = 0
    errors = 0

    for instrument in args.instruments:
        csv_path = args.data_dir / f"{instrument}.csv"
        if not csv_path.exists():
            print(f"WARN: missing CSV for {instrument}: {csv_path}", file=sys.stderr)
            continue

        bars = load_bars(csv_path)
        if args.progress:
            print(f"[{instrument}] loaded {len(bars)} bars")

        for idx in eligible_eval_indices(
            bars, args.date_from, args.date_to, args.lookback_bars
        ):
            total += 1
            eval_bar = bars[idx]
            history = bars[max(0, idx + 1 - args.lookback_bars) : idx + 1]
            input_hash = compute_input_hash(
                instrument=instrument,
                history=history,
                horizon_days=args.horizon,
                lookback_bars=args.lookback_bars,
                target_field=args.target_field,
            )
            cache_key = compute_cache_key(
                model_name=args.model_name,
                model_version=args.model_version,
                tokenizer_name=args.tokenizer_name,
                instrument=instrument,
                eval_date=eval_bar.date,
                horizon_days=args.horizon,
                lookback_bars=args.lookback_bars,
                sample_count=args.sample_count,
                temperature=args.temperature,
                top_p=args.top_p,
                target_field=args.target_field,
                input_hash=input_hash,
            )

            existing = get_existing_status(conn, cache_key)
            if existing == "ok":
                skipped += 1
                continue
            if existing == "error":
                delete_existing(conn, cache_key)

            started = time.perf_counter()
            try:
                summary, raw_payload = forecaster.predict_summary(
                    instrument=instrument,
                    eval_date=eval_bar.date,
                    history=history,
                    horizon_days=args.horizon,
                    lookback_bars=args.lookback_bars,
                    sample_count=args.sample_count,
                    temperature=args.temperature,
                    top_p=args.top_p,
                    target_field=args.target_field,
                )
                latency_ms = int((time.perf_counter() - started) * 1000)
                row = forecast_row(
                    cache_key=cache_key,
                    model_name=args.model_name,
                    model_version=args.model_version,
                    tokenizer_name=args.tokenizer_name,
                    instrument=instrument,
                    eval_date=eval_bar.date,
                    horizon_days=args.horizon,
                    lookback_bars=args.lookback_bars,
                    input_hash=input_hash,
                    sample_count=args.sample_count,
                    temperature=args.temperature,
                    top_p=args.top_p,
                    target_field=args.target_field,
                    status="ok",
                    forecast_json=json.dumps(asdict(summary), sort_keys=True),
                    raw_response_json=json.dumps(raw_payload, sort_keys=True),
                    error_text=None,
                    latency_ms=latency_ms,
                )
                insert_forecast_row(conn, row)
                inserted += 1
                if args.progress:
                    print(
                        f"[{instrument} {eval_bar.date}] ok mode={forecaster.mode_name} "
                        f"key={cache_key}"
                    )
            except Exception as exc:
                errors += 1
                latency_ms = int((time.perf_counter() - started) * 1000)
                row = forecast_row(
                    cache_key=cache_key,
                    model_name=args.model_name,
                    model_version=args.model_version,
                    tokenizer_name=args.tokenizer_name,
                    instrument=instrument,
                    eval_date=eval_bar.date,
                    horizon_days=args.horizon,
                    lookback_bars=args.lookback_bars,
                    input_hash=input_hash,
                    sample_count=args.sample_count,
                    temperature=args.temperature,
                    top_p=args.top_p,
                    target_field=args.target_field,
                    status="error",
                    forecast_json="{}",
                    raw_response_json=None,
                    error_text=str(exc),
                    latency_ms=latency_ms,
                )
                insert_forecast_row(conn, row)
                if args.progress:
                    print(
                        f"[{instrument} {eval_bar.date}] error key={cache_key}: {exc}",
                        file=sys.stderr,
                    )

            conn.commit()

    print(
        json.dumps(
            {
                "status": "ok",
                "mode": forecaster.mode_name,
                "model_name": args.model_name,
                "model_version": args.model_version,
                "tokenizer_name": args.tokenizer_name,
                "from": args.date_from.isoformat(),
                "to": args.date_to.isoformat(),
                "horizon": args.horizon,
                "instruments": args.instruments,
                "total_candidates": total,
                "inserted": inserted,
                "skipped_existing_ok": skipped,
                "error_rows_written": errors,
                "db": str(args.db),
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
