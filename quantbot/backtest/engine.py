"""Backtest engine with look-ahead bias prevention."""

from __future__ import annotations

import logging
from dataclasses import dataclass, field
from datetime import datetime

import numpy as np
import pandas as pd

from quantbot.agents.tsmom.agent import TSMOMAgent
from quantbot.core.portfolio import Fill, Order, OrderSide, PortfolioState, Position
from quantbot.core.signal import Signal, SignalDirection
from quantbot.data.bar import BarDataFrame

logger = logging.getLogger(__name__)


@dataclass
class BacktestConfig:
    initial_cash: float = 1_000_000.0
    slippage_bps: float = 5.0  # 5 basis points
    commission_bps: float = 0.0
    vol_target: float = 0.40
    max_gross_leverage: float = 2.0  # 200% gross
    max_position_pct: float = 0.20  # 20% per position


@dataclass
class Snapshot:
    timestamp: datetime
    nav: float
    cash: float
    gross_exposure: float
    net_exposure: float
    positions: dict[str, float]  # instrument -> quantity
    signals: dict[str, Signal]
    fills: list[Fill]


class BacktestEngine:
    """Event-driven backtest engine.

    Iterates through timestamps chronologically. At each bar:
    1. Mark-to-market existing positions at the bar's OPEN
    2. Run agent on data available UP TO (but not including) the current bar's close
       (using previous bars only to prevent look-ahead bias)
    3. Generate target positions from signals
    4. Execute rebalancing trades at the current bar's OPEN (next-open execution)
    5. Record snapshot

    The "next-open" execution model means signals generated from day T's data
    are executed at day T+1's open price.
    """

    def __init__(self, config: BacktestConfig | None = None) -> None:
        self.config = config or BacktestConfig()
        self.snapshots: list[Snapshot] = []

    def run(
        self,
        agent: TSMOMAgent,
        bars_by_instrument: dict[str, BarDataFrame],
        min_history: int = 252,
    ) -> list[Snapshot]:
        """Run backtest across multiple instruments.

        Args:
            agent: The trading agent to use.
            bars_by_instrument: Dict of instrument -> OHLCV bars.
            min_history: Minimum bars required before generating first signal.

        Returns:
            List of daily snapshots.
        """
        # Align all instruments to the same date index
        all_dates = self._get_common_dates(bars_by_instrument)

        if len(all_dates) <= min_history:
            raise ValueError(
                f"Need > {min_history} common dates, got {len(all_dates)}"
            )

        portfolio = PortfolioState(cash=self.config.initial_cash)
        self.snapshots = []
        pending_targets: dict[str, float] = {}  # from previous day's signal

        for i in range(min_history, len(all_dates)):
            today = all_dates[i]
            prices_today: dict[str, float] = {}
            open_prices: dict[str, float] = {}

            for sym, bars in bars_by_instrument.items():
                if today in bars.index:
                    prices_today[sym] = float(bars.loc[today, "Close"])
                    open_prices[sym] = float(bars.loc[today, "Open"])

            # Step 1: Execute pending orders at today's open
            fills = []
            if pending_targets:
                fills = self._rebalance(
                    portfolio, pending_targets, open_prices
                )

            # Step 2: Mark positions to today's close
            self._mark_to_market(portfolio, prices_today)

            # Step 3: Generate signals using data UP TO today's close
            # (signals will execute tomorrow — no look-ahead)
            signals: dict[str, Signal] = {}
            target_weights: dict[str, float] = {}

            for sym, bars in bars_by_instrument.items():
                history = bars.loc[:today]  # inclusive of today
                if len(history) < min_history:
                    continue
                sig = agent.generate_signal(history, sym)
                signals[sym] = sig
                if sig.direction != SignalDirection.FLAT:
                    target_weights[sym] = agent.compute_target_weight(sig)
                else:
                    target_weights[sym] = 0.0

            # Apply risk limits to target weights
            target_weights = self._apply_risk_limits(target_weights)

            # Convert weights to target quantities
            nav = portfolio.nav
            pending_targets = {}
            for sym, weight in target_weights.items():
                if sym in prices_today and prices_today[sym] > 0:
                    target_notional = weight * nav
                    pending_targets[sym] = target_notional / prices_today[sym]

            # Step 4: Record snapshot
            snapshot = Snapshot(
                timestamp=today,
                nav=portfolio.nav,
                cash=portfolio.cash,
                gross_exposure=portfolio.gross_exposure(prices_today),
                net_exposure=portfolio.net_exposure(prices_today),
                positions={
                    sym: pos.quantity
                    for sym, pos in portfolio.positions.items()
                },
                signals=signals,
                fills=fills,
            )
            self.snapshots.append(snapshot)

        return self.snapshots

    def _get_common_dates(
        self, bars_by_instrument: dict[str, BarDataFrame]
    ) -> pd.DatetimeIndex:
        """Get the union of all dates (we trade each instrument on its own dates)."""
        all_dates: set[datetime] = set()
        for bars in bars_by_instrument.values():
            all_dates.update(bars.index)
        return pd.DatetimeIndex(sorted(all_dates))

    def _rebalance(
        self,
        portfolio: PortfolioState,
        target_quantities: dict[str, float],
        open_prices: dict[str, float],
    ) -> list[Fill]:
        """Rebalance portfolio to target quantities at open prices."""
        fills = []

        # Close positions for instruments no longer targeted
        for sym in list(portfolio.positions.keys()):
            if sym not in target_quantities:
                target_quantities[sym] = 0.0

        for sym, target_qty in target_quantities.items():
            if sym not in open_prices:
                continue

            current_qty = 0.0
            if sym in portfolio.positions:
                current_qty = portfolio.positions[sym].quantity

            delta = target_qty - current_qty
            if abs(delta) < 1e-8:
                continue

            price = open_prices[sym]
            # Apply slippage
            slippage = self.config.slippage_bps / 10_000
            if delta > 0:
                fill_price = price * (1 + slippage)
                side = OrderSide.BUY
            else:
                fill_price = price * (1 - slippage)
                side = OrderSide.SELL

            order = Order(
                instrument=sym,
                side=side,
                quantity=abs(delta),
            )
            fill = Fill(
                order=order,
                fill_price=fill_price,
                fill_quantity=abs(delta),
                slippage_bps=self.config.slippage_bps,
            )
            fills.append(fill)

            # Update portfolio
            cost = delta * fill_price
            portfolio.cash -= cost

            if abs(target_qty) < 1e-8:
                portfolio.positions.pop(sym, None)
            else:
                portfolio.positions[sym] = Position(
                    instrument=sym,
                    quantity=target_qty,
                    avg_entry_price=fill_price,
                )

        return fills

    def _mark_to_market(
        self, portfolio: PortfolioState, prices: dict[str, float]
    ) -> None:
        """Update position entries to current market prices.

        Only updates avg_entry_price so portfolio.nav reflects current
        market value.  Cash is NOT adjusted — it tracks cumulative
        trade costs only, avoiding double-counting of PnL.
        """
        for sym, pos in list(portfolio.positions.items()):
            if sym in prices:
                pos.avg_entry_price = prices[sym]

    def _apply_risk_limits(
        self, weights: dict[str, float]
    ) -> dict[str, float]:
        """Scale down weights if gross leverage exceeds limit."""
        gross = sum(abs(w) for w in weights.values())
        if gross > self.config.max_gross_leverage:
            scale = self.config.max_gross_leverage / gross
            weights = {sym: w * scale for sym, w in weights.items()}

        # Cap individual position weights
        for sym in weights:
            if abs(weights[sym]) > self.config.max_position_pct * self.config.max_gross_leverage:
                cap = self.config.max_position_pct * self.config.max_gross_leverage
                weights[sym] = np.sign(weights[sym]) * cap

        return weights
