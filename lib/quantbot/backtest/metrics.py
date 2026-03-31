"""Backtest result analysis and metrics."""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np
import pandas as pd
import plotly.graph_objects as go
from plotly.subplots import make_subplots

from quantbot.backtest.engine import Snapshot

TRADING_DAYS_PER_YEAR = 252


@dataclass
class BacktestResult:
    """Computed from a list of backtest snapshots."""

    equity_curve: pd.Series
    daily_returns: pd.Series
    sharpe_ratio: float
    annualized_return: float
    annualized_vol: float
    max_drawdown: float
    max_drawdown_duration_days: int
    calmar_ratio: float
    sortino_ratio: float
    monthly_returns: pd.DataFrame
    total_trades: int

    @classmethod
    def from_snapshots(cls, snapshots: list[Snapshot]) -> BacktestResult:
        if len(snapshots) < 2:
            raise ValueError("Need at least 2 snapshots to compute metrics")

        navs = pd.Series(
            [s.nav for s in snapshots],
            index=pd.DatetimeIndex([s.timestamp for s in snapshots]),
            name="NAV",
        )

        daily_rets = navs.pct_change().dropna()
        total_trades = sum(len(s.fills) for s in snapshots)

        # Annualized return
        total_return = navs.iloc[-1] / navs.iloc[0] - 1
        n_years = len(daily_rets) / TRADING_DAYS_PER_YEAR
        ann_return = (1 + total_return) ** (1 / n_years) - 1 if n_years > 0 else 0.0

        # Annualized volatility
        ann_vol = float(daily_rets.std() * np.sqrt(TRADING_DAYS_PER_YEAR))

        # Sharpe ratio (assuming 0% risk-free rate)
        sharpe = ann_return / ann_vol if ann_vol > 1e-8 else 0.0

        # Max drawdown
        cummax = navs.cummax()
        drawdown = (navs - cummax) / cummax
        max_dd = float(drawdown.min())

        # Max drawdown duration
        dd_duration = 0
        max_dd_dur = 0
        for i in range(1, len(drawdown)):
            if drawdown.iloc[i] < 0:
                dd_duration += 1
                max_dd_dur = max(max_dd_dur, dd_duration)
            else:
                dd_duration = 0

        # Calmar ratio
        calmar = ann_return / abs(max_dd) if abs(max_dd) > 1e-8 else 0.0

        # Sortino ratio (downside deviation)
        downside = daily_rets[daily_rets < 0]
        downside_std = float(downside.std() * np.sqrt(TRADING_DAYS_PER_YEAR)) if len(downside) > 0 else 1e-8
        sortino = ann_return / downside_std if downside_std > 1e-8 else 0.0

        # Monthly returns table
        monthly = daily_rets.resample("ME").apply(lambda x: (1 + x).prod() - 1)
        monthly_pivot = pd.DataFrame(
            {
                "Year": monthly.index.year,
                "Month": monthly.index.month,
                "Return": monthly.values,
            }
        )
        monthly_table = monthly_pivot.pivot_table(
            values="Return", index="Year", columns="Month", aggfunc="sum"
        )
        monthly_table.columns = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun",
            "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ][: len(monthly_table.columns)]

        return cls(
            equity_curve=navs,
            daily_returns=daily_rets,
            sharpe_ratio=sharpe,
            annualized_return=ann_return,
            annualized_vol=ann_vol,
            max_drawdown=max_dd,
            max_drawdown_duration_days=max_dd_dur,
            calmar_ratio=calmar,
            sortino_ratio=sortino,
            monthly_returns=monthly_table,
            total_trades=total_trades,
        )

    def summary(self) -> str:
        """Human-readable performance summary."""
        lines = [
            "=" * 50,
            "  BACKTEST RESULTS",
            "=" * 50,
            f"  Period:          {self.equity_curve.index[0].date()} → {self.equity_curve.index[-1].date()}",
            f"  Starting NAV:    ${self.equity_curve.iloc[0]:,.0f}",
            f"  Ending NAV:      ${self.equity_curve.iloc[-1]:,.0f}",
            f"  Total Return:    {self.equity_curve.iloc[-1] / self.equity_curve.iloc[0] - 1:.1%}",
            "-" * 50,
            f"  Ann. Return:     {self.annualized_return:.2%}",
            f"  Ann. Volatility: {self.annualized_vol:.2%}",
            f"  Sharpe Ratio:    {self.sharpe_ratio:.2f}",
            f"  Sortino Ratio:   {self.sortino_ratio:.2f}",
            f"  Calmar Ratio:    {self.calmar_ratio:.2f}",
            f"  Max Drawdown:    {self.max_drawdown:.2%}",
            f"  Max DD Duration: {self.max_drawdown_duration_days} days",
            f"  Total Trades:    {self.total_trades:,}",
            "=" * 50,
        ]
        return "\n".join(lines)

    def plot(self, save_path: str | None = None) -> None:
        """Plot interactive equity curve and drawdown with Plotly."""
        cummax = self.equity_curve.cummax()
        drawdown = (self.equity_curve - cummax) / cummax

        fig = make_subplots(
            rows=2,
            cols=1,
            shared_xaxes=True,
            vertical_spacing=0.06,
            row_heights=[0.75, 0.25],
            subplot_titles=["Equity Curve (log scale)", "Drawdown"],
        )

        # Equity curve
        fig.add_trace(
            go.Scatter(
                x=self.equity_curve.index,
                y=self.equity_curve.values,
                mode="lines",
                name="NAV",
                line=dict(width=1.5, color="#1f77b4"),
                hovertemplate="Date: %{x|%Y-%m-%d}<br>NAV: $%{y:,.0f}<extra></extra>",
            ),
            row=1,
            col=1,
        )

        # Drawdown
        fig.add_trace(
            go.Scatter(
                x=drawdown.index,
                y=drawdown.values,
                mode="lines",
                name="Drawdown",
                fill="tozeroy",
                line=dict(width=1, color="crimson"),
                fillcolor="rgba(220, 20, 60, 0.3)",
                hovertemplate="Date: %{x|%Y-%m-%d}<br>DD: %{y:.1%}<extra></extra>",
            ),
            row=2,
            col=1,
        )

        fig.update_yaxes(type="log", title_text="NAV ($)", row=1, col=1)
        fig.update_yaxes(title_text="Drawdown", tickformat=".0%", row=2, col=1)
        fig.update_xaxes(title_text="Date", row=2, col=1)

        title = (
            f"TSMOM Backtest  |  Sharpe: {self.sharpe_ratio:.2f}  "
            f"Ann. Return: {self.annualized_return:.1%}  "
            f"Max DD: {self.max_drawdown:.1%}"
        )
        fig.update_layout(
            title=title,
            height=700,
            showlegend=False,
            template="plotly_white",
            hovermode="x unified",
        )

        if save_path:
            if save_path.endswith(".html"):
                fig.write_html(save_path)
            else:
                fig.write_image(save_path, scale=2)
            print(f"Plot saved to {save_path}")
        else:
            fig.show()
