"""Paper trading engine with simulated fills."""

from __future__ import annotations

import logging
from datetime import UTC, datetime

import numpy as np

from quantbot.core.portfolio import Fill, Order, OrderSide, PortfolioState, Position
from quantbot.core.signal import Signal, SignalDirection

logger = logging.getLogger(__name__)


class PaperTradingEngine:
    """Simulated execution engine for paper trading.

    Fills orders instantly at the current price ± configurable slippage.
    """

    def __init__(
        self,
        initial_cash: float = 1_000_000.0,
        slippage_bps: float = 5.0,
        commission_bps: float = 0.0,
    ) -> None:
        self.slippage_bps = slippage_bps
        self.commission_bps = commission_bps
        self.portfolio = PortfolioState(cash=initial_cash)
        self.fills: list[Fill] = []

    def execute_order(self, order: Order, market_price: float) -> Fill:
        """Execute an order at market price with slippage.

        Args:
            order: The order to execute.
            market_price: Current market price of the instrument.

        Returns:
            The resulting Fill.
        """
        slip = self.slippage_bps / 10_000
        if order.side == OrderSide.BUY:
            fill_price = market_price * (1 + slip)
        else:
            fill_price = market_price * (1 - slip)

        fill = Fill(
            order=order,
            fill_price=fill_price,
            fill_quantity=order.quantity,
            timestamp=datetime.now(UTC),
            slippage_bps=self.slippage_bps,
        )

        # Update portfolio
        signed_qty = order.quantity if order.side == OrderSide.BUY else -order.quantity
        cost = signed_qty * fill_price
        commission = abs(cost) * (self.commission_bps / 10_000)
        self.portfolio.cash -= cost + commission

        sym = order.instrument
        if sym in self.portfolio.positions:
            pos = self.portfolio.positions[sym]
            new_qty = pos.quantity + signed_qty
            if abs(new_qty) < 1e-8:
                del self.portfolio.positions[sym]
            else:
                # Weighted average entry price for adds, keep for reduces
                if np.sign(signed_qty) == np.sign(pos.quantity):
                    total_cost = pos.quantity * pos.avg_entry_price + signed_qty * fill_price
                    avg_price = total_cost / new_qty
                else:
                    avg_price = fill_price if np.sign(new_qty) != np.sign(pos.quantity) else pos.avg_entry_price
                self.portfolio.positions[sym] = Position(
                    instrument=sym,
                    quantity=new_qty,
                    avg_entry_price=avg_price,
                )
        else:
            if abs(signed_qty) > 1e-8:
                self.portfolio.positions[sym] = Position(
                    instrument=sym,
                    quantity=signed_qty,
                    avg_entry_price=fill_price,
                )

        self.fills.append(fill)
        logger.info(
            "FILL %s %s %.4f @ %.2f (slip=%.1f bps)",
            order.side.value,
            sym,
            order.quantity,
            fill_price,
            self.slippage_bps,
        )
        return fill

    def execute_target_weights(
        self,
        target_weights: dict[str, float],
        prices: dict[str, float],
        nav: float | None = None,
    ) -> list[Fill]:
        """Rebalance portfolio to target weights.

        Args:
            target_weights: instrument -> signed weight (fraction of NAV).
            prices: instrument -> current market price.
            nav: Portfolio NAV to use. If None, computed from current state.

        Returns:
            List of fills from rebalancing.
        """
        if nav is None:
            nav = self.portfolio.nav

        fills = []

        # Determine target quantities
        all_instruments = set(target_weights.keys()) | set(self.portfolio.positions.keys())

        for sym in all_instruments:
            target_weight = target_weights.get(sym, 0.0)
            price = prices.get(sym)
            if price is None or price <= 0:
                continue

            target_qty = (target_weight * nav) / price
            current_qty = self.portfolio.positions[sym].quantity if sym in self.portfolio.positions else 0.0
            delta = target_qty - current_qty

            if abs(delta) < 1e-8:
                continue

            side = OrderSide.BUY if delta > 0 else OrderSide.SELL
            order = Order(instrument=sym, side=side, quantity=abs(delta))
            fill = self.execute_order(order, price)
            fills.append(fill)

        return fills
