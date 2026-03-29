"""SQLite-backed decision memory store.

Stores signal history, decision outcomes, and condensed agent memory
for injection into LLM prompts (FinMem-inspired layered memory).
"""

from __future__ import annotations

import json
import sqlite3
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from quantbot.core.signal import Signal

DEFAULT_DB_PATH = Path("~/.quantbot/memory.db").expanduser()

_CREATE_TABLES = """
CREATE TABLE IF NOT EXISTS signal_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    instrument TEXT NOT NULL,
    agent_name TEXT NOT NULL,
    direction TEXT NOT NULL,
    strength REAL NOT NULL,
    confidence REAL NOT NULL,
    signal_type TEXT NOT NULL,
    horizon_days INTEGER,
    metadata TEXT
);

CREATE TABLE IF NOT EXISTS decision_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    instrument TEXT NOT NULL,
    direction TEXT NOT NULL,
    strength REAL NOT NULL,
    confidence REAL NOT NULL,
    signals_json TEXT,
    debate_json TEXT,
    reasoning TEXT,
    actual_return REAL,
    outcome TEXT
);

CREATE TABLE IF NOT EXISTS agent_memory (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    agent_name TEXT NOT NULL,
    lesson TEXT NOT NULL,
    context TEXT
);

CREATE INDEX IF NOT EXISTS idx_signal_instrument ON signal_log(instrument, timestamp);
CREATE INDEX IF NOT EXISTS idx_decision_instrument ON decision_log(instrument, timestamp);
CREATE INDEX IF NOT EXISTS idx_memory_agent ON agent_memory(agent_name, timestamp);
"""


class MemoryStore:
    """SQLite-backed memory for trading agents."""

    def __init__(self, db_path: str | Path = DEFAULT_DB_PATH) -> None:
        self.db_path = Path(db_path).expanduser()
        self.db_path.parent.mkdir(parents=True, exist_ok=True)
        self._conn = sqlite3.connect(str(self.db_path))
        self._conn.row_factory = sqlite3.Row
        self._conn.executescript(_CREATE_TABLES)

    def log_signal(self, signal: Signal) -> None:
        """Record a signal produced by any agent."""
        self._conn.execute(
            """INSERT INTO signal_log
               (timestamp, instrument, agent_name, direction, strength,
                confidence, signal_type, horizon_days, metadata)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)""",
            (
                signal.timestamp.isoformat(),
                signal.instrument,
                signal.agent_name,
                signal.direction.value,
                signal.strength,
                signal.confidence,
                signal.signal_type.value,
                signal.horizon_days,
                json.dumps(signal.metadata, default=str),
            ),
        )
        self._conn.commit()

    def log_decision(
        self,
        instrument: str,
        direction: str,
        strength: float,
        confidence: float,
        signals: list[Signal] | None = None,
        debate: dict[str, Any] | None = None,
        reasoning: str = "",
    ) -> int:
        """Record a decision. Returns the decision ID for later outcome update."""
        signals_json = json.dumps(
            [
                {
                    "agent": s.agent_name,
                    "direction": s.direction.value,
                    "strength": s.strength,
                    "confidence": s.confidence,
                }
                for s in (signals or [])
            ]
        )
        cursor = self._conn.execute(
            """INSERT INTO decision_log
               (timestamp, instrument, direction, strength, confidence,
                signals_json, debate_json, reasoning)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)""",
            (
                datetime.now(UTC).isoformat(),
                instrument,
                direction,
                strength,
                confidence,
                signals_json,
                json.dumps(debate, default=str) if debate else None,
                reasoning,
            ),
        )
        self._conn.commit()
        return cursor.lastrowid  # type: ignore[return-value]

    def update_decision_outcome(
        self, decision_id: int, actual_return: float, outcome: str
    ) -> None:
        """Update a decision with its actual outcome (win/loss/flat)."""
        self._conn.execute(
            """UPDATE decision_log
               SET actual_return = ?, outcome = ?
               WHERE id = ?""",
            (actual_return, outcome, decision_id),
        )
        self._conn.commit()

    def add_lesson(self, agent_name: str, lesson: str, context: str = "") -> None:
        """Store a condensed lesson for an agent."""
        self._conn.execute(
            """INSERT INTO agent_memory (timestamp, agent_name, lesson, context)
               VALUES (?, ?, ?, ?)""",
            (datetime.now(UTC).isoformat(), agent_name, lesson, context),
        )
        self._conn.commit()

    def get_recent_signals(
        self, instrument: str, agent_name: str | None = None, limit: int = 20
    ) -> list[dict[str, Any]]:
        """Fetch recent signals for an instrument."""
        if agent_name:
            rows = self._conn.execute(
                """SELECT * FROM signal_log
                   WHERE instrument = ? AND agent_name = ?
                   ORDER BY timestamp DESC LIMIT ?""",
                (instrument, agent_name, limit),
            ).fetchall()
        else:
            rows = self._conn.execute(
                """SELECT * FROM signal_log
                   WHERE instrument = ?
                   ORDER BY timestamp DESC LIMIT ?""",
                (instrument, limit),
            ).fetchall()
        return [dict(r) for r in rows]

    def get_recent_decisions(
        self, instrument: str, limit: int = 10
    ) -> list[dict[str, Any]]:
        """Fetch recent decisions with outcomes."""
        rows = self._conn.execute(
            """SELECT * FROM decision_log
               WHERE instrument = ?
               ORDER BY timestamp DESC LIMIT ?""",
            (instrument, limit),
        ).fetchall()
        return [dict(r) for r in rows]

    def get_agent_lessons(self, agent_name: str, limit: int = 10) -> list[str]:
        """Fetch condensed lessons for an agent's prompt context."""
        rows = self._conn.execute(
            """SELECT lesson FROM agent_memory
               WHERE agent_name = ?
               ORDER BY timestamp DESC LIMIT ?""",
            (agent_name, limit),
        ).fetchall()
        return [r["lesson"] for r in rows]

    def get_win_rate(self, instrument: str, agent_name: str | None = None) -> dict[str, Any]:
        """Compute win/loss stats for an instrument (optionally filtered by agent)."""
        if agent_name:
            rows = self._conn.execute(
                """SELECT outcome, COUNT(*) as cnt FROM decision_log
                   WHERE instrument = ? AND outcome IS NOT NULL
                   AND signals_json LIKE ?
                   GROUP BY outcome""",
                (instrument, f'%"{agent_name}"%'),
            ).fetchall()
        else:
            rows = self._conn.execute(
                """SELECT outcome, COUNT(*) as cnt FROM decision_log
                   WHERE instrument = ? AND outcome IS NOT NULL
                   GROUP BY outcome""",
                (instrument,),
            ).fetchall()
        stats = {r["outcome"]: r["cnt"] for r in rows}
        total = sum(stats.values())
        wins = stats.get("win", 0)
        return {
            "wins": wins,
            "losses": stats.get("loss", 0),
            "flat": stats.get("flat", 0),
            "total": total,
            "win_rate": wins / total if total > 0 else 0.0,
        }

    def build_memory_context(self, instrument: str, limit: int = 5) -> str:
        """Build a text summary of recent decisions for LLM prompt injection."""
        decisions = self.get_recent_decisions(instrument, limit=limit)
        if not decisions:
            return "No prior decision history for this instrument."

        lines = [f"Recent decision history for {instrument}:"]
        for d in decisions:
            outcome_str = f" → {d['outcome']} ({d['actual_return']:+.2%})" if d.get("outcome") else " → pending"
            lines.append(
                f"  {d['timestamp'][:10]}: {d['direction']} "
                f"(strength={d['strength']:.2f}, confidence={d['confidence']:.2f})"
                f"{outcome_str}"
            )

        stats = self.get_win_rate(instrument)
        if stats["total"] > 0:
            lines.append(
                f"  Overall: {stats['wins']}W / {stats['losses']}L / {stats['flat']}F "
                f"({stats['win_rate']:.0%} win rate)"
            )

        return "\n".join(lines)

    def close(self) -> None:
        self._conn.close()
