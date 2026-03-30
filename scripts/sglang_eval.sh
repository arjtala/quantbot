#!/bin/bash
# QuantBot — Run eval against SGLang server
#
# Usage:
#   bash scripts/sglang_eval.sh <sglang-node> [model-name] [port] [workers] [days]
#
# Examples:
#   bash scripts/sglang_eval.sh h200-node SUFE-AIFLM-Lab/Fin-R1              # 60 days (default)
#   bash scripts/sglang_eval.sh h200-node SUFE-AIFLM-Lab/Fin-R1 30000 4 252  # 252-day full eval

set -e

NODE=${1:?"Usage: $0 <sglang-node> [model-name] [port] [workers] [days]"}
MODEL=${2:-"SUFE-AIFLM-Lab/Fin-R1"}
PORT=${3:-30000}
WORKERS=${4:-4}
DAYS=${5:-60}

# Bypass cluster proxy for compute node
export no_proxy="$NODE"
export NO_PROXY="$NODE"

echo "=== QuantBot Eval via SGLang ==="
echo "Server:  http://$NODE:$PORT/v1"
echo "Model:   $MODEL"
echo "Workers: $WORKERS"

# Create .env for this run
cat > .env << EOF
DEFAULT_PROVIDER=sglang
OPENAI_BASE_URL=http://$NODE:$PORT/v1
OPENAI_API_KEY=not-needed
no_proxy=$NODE
NO_PROXY=$NODE
INDICATOR_MODEL=sglang:$MODEL
PATTERN_MODEL=sglang:$MODEL
TREND_MODEL=sglang:$MODEL
DEBATE_MODEL=sglang:$MODEL
DECISION_MODEL=sglang:$MODEL
BACKTEST_MODEL=sglang:$MODEL
INSTRUMENTS=BTC-USD,ETH-USD,SOL-USD,BNB-USD,SPY,QQQ,IWM,EFA,EEM,TLT,GLD,ES=F,NQ=F,GC=F,CL=F,ZB=F,EURUSD=X,GBPUSD=X,USDJPY=X,AUDUSD=X,USDCHF=X
EOF

echo "Created .env pointing to SGLang server"
echo ""

# Quick connectivity test
echo "Testing connection..."
curl -s --noproxy "$NODE" "http://$NODE:$PORT/v1/models" | python3 -m json.tool || {
    echo "ERROR: Cannot reach SGLang server at http://$NODE:$PORT/v1"
    echo "Is the SLURM job running? Check: squeue -u \$USER"
    exit 1
}

TOTAL_CALLS=$((21 * DAYS))
echo ""
echo "Connection OK. Running eval (21 instruments × $DAYS days = $TOTAL_CALLS LLM calls, $WORKERS workers)..."
python scripts/eval_round1.py --days "$DAYS" --data-dir data/ --workers "$WORKERS"
