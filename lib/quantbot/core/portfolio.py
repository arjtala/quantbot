"""Portfolio state, position, and order types."""

from __future__ import annotations

from dataclasses import dataclass, field
from datetime import UTC, datetime
from enum import Enum
from typing import Any


class OrderSide(Enum):
    BUY = "BUY"
    SELL = "SELL"


@dataclass
class Position:
    instrument: str
    quantity: float  # positive = long, negative = short
    avg_entry_price: float
    point_value: float = 1.0

    @property
    def notional(self) -> float:
        return abs(self.quantity) * self.avg_entry_price * self.point_value

    def unrealised_pnl(self, current_price: float) -> float:
        return self.quantity * (current_price - self.avg_entry_price) * self.point_value


@dataclass
class Order:
    instrument: str
    side: OrderSide
    quantity: float  # always positive
    timestamp: datetime = field(default_factory=lambda: datetime.now(UTC))
    limit_price: float | None = None
    metadata: dict[str, Any] = field(default_factory=dict)


@dataclass
class Fill:
    order: Order
    fill_price: float
    fill_quantity: float
    timestamp: datetime = field(default_factory=lambda: datetime.now(UTC))
    slippage_bps: float = 0.0


@dataclass
class PortfolioState:
    """Snapshot of portfolio at a point in time."""

    cash: float
    positions: dict[str, Position] = field(default_factory=dict)
    timestamp: datetime = field(default_factory=lambda: datetime.now(UTC))

    @property
    def nav(self) -> float:
        """Net asset value (requires current prices passed separately for real use).
        For backtest snapshots, positions carry mark prices in avg_entry_price after re-mark."""
        pos_value = sum(
            p.quantity * p.avg_entry_price * p.point_value
            for p in self.positions.values()
        )
        return self.cash + pos_value

    def gross_exposure(self, prices: dict[str, float] | None = None) -> float:
        total = 0.0
        for sym, pos in self.positions.items():
            px = prices.get(sym, pos.avg_entry_price) if prices else pos.avg_entry_price
            total += abs(pos.quantity) * px * pos.point_value
        return total

    def net_exposure(self, prices: dict[str, float] | None = None) -> float:
        total = 0.0
        for sym, pos in self.positions.items():
            px = prices.get(sym, pos.avg_entry_price) if prices else pos.avg_entry_price
            total += pos.quantity * px * pos.point_value
        return total
