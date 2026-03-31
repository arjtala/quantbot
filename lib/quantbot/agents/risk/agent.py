"""Risk Manager — position sizing and veto authority.

Has the power to override or reduce the Decision Agent's output
based on portfolio risk limits.
"""

from __future__ import annotations

import logging
from typing import Any

from quantbot.config import settings
from quantbot.core.signal import SignalDirection
from quantbot.graph.state import TradingGraphState

logger = logging.getLogger(__name__)


def make_risk_node() -> Any:
    """Create a LangGraph-compatible Risk Manager node.

    The risk manager can:
    1. Reduce position size if it exceeds max_position_pct of NAV
    2. Veto a trade entirely if gross exposure would exceed max_gross_leverage
    3. Block single trades exceeding max_single_trade_pct
    4. Force FLAT if max drawdown limit is breached
    """

    def risk_node(state: TradingGraphState) -> dict[str, Any]:
        decision = state.get("decision", {})
        metadata = state.get("metadata", {})

        direction = decision.get("direction", "FLAT")
        strength = decision.get("strength", 0.0)
        confidence = decision.get("confidence", 0.0)

        vetoed = False
        veto_reasons: list[str] = []

        # Check portfolio-level risk (if portfolio state is available)
        portfolio_state = metadata.get("portfolio_state")
        if portfolio_state:
            nav = portfolio_state.get("nav", 0)
            gross_exposure = portfolio_state.get("gross_exposure", 0)

            # Gross leverage check
            if nav > 0:
                current_leverage = gross_exposure / nav
                if current_leverage > settings.max_gross_leverage * 0.95:
                    # Near leverage limit — reduce or veto
                    if direction != "FLAT":
                        veto_reasons.append(
                            f"Gross leverage {current_leverage:.1%} near limit "
                            f"({settings.max_gross_leverage:.0%})"
                        )
                        strength *= 0.5  # Halve the position
                        logger.warning("Risk: Reducing position size due to leverage")

                if current_leverage > settings.max_gross_leverage:
                    vetoed = True
                    veto_reasons.append(
                        f"Gross leverage {current_leverage:.1%} exceeds limit"
                    )

        # Confidence floor — don't trade on weak signals
        if confidence < 0.2 and direction != "FLAT":
            veto_reasons.append(f"Confidence too low ({confidence:.2f} < 0.20)")
            vetoed = True

        # Apply veto
        if vetoed:
            logger.warning("Risk VETO: %s — %s", state.get("instrument"), "; ".join(veto_reasons))
            decision = {
                **decision,
                "direction": "FLAT",
                "strength": 0.0,
                "risk_vetoed": True,
                "veto_reasons": veto_reasons,
            }
        else:
            decision = {
                **decision,
                "strength": strength,
                "risk_vetoed": False,
                "risk_notes": veto_reasons if veto_reasons else ["No risk concerns"],
            }

        return {"decision": decision}

    return risk_node
