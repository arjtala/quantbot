#!/bin/bash
# QuantBot — Run eval against SGLang server
#
# Usage:
#   bash scripts/sglang_eval.sh <sglang-node> <model-name>
#
# Example:
#   bash scripts/sglang_eval.sh gpu-node-01 deepseek-ai/DeepSeek-R1-Distill-Qwen-32B

set -e

NODE=${1:?"Usage: $0 <sglang-node> [model-name]"}
MODEL=${2:-"deepseek-ai/DeepSeek-R1-Distill-Qwen-32B"}
PORT=${3:-30000}

echo "=== QuantBot Eval via SGLang ==="
echo "Server: http://$NODE:$PORT/v1"
echo "Model:  $MODEL"

# Create .env for this run
cat > .env << EOF
DEFAULT_PROVIDER=sglang
OPENAI_BASE_URL=http://$NODE:$PORT/v1
OPENAI_API_KEY=not-needed
INDICATOR_MODEL=sglang:$MODEL
PATTERN_MODEL=sglang:$MODEL
TREND_MODEL=sglang:$MODEL
DEBATE_MODEL=sglang:$MODEL
DECISION_MODEL=sglang:$MODEL
BACKTEST_MODEL=sglang:$MODEL
INSTRUMENTS=SPY,BTC-USD,ES=F,GC=F
EOF

echo "Created .env pointing to SGLang server"
echo ""

# Quick connectivity test
echo "Testing connection..."
curl -s "http://$NODE:$PORT/v1/models" | python3 -m json.tool || {
    echo "ERROR: Cannot reach SGLang server at http://$NODE:$PORT/v1"
    echo "Is the SLURM job running? Check: squeue -u $USER"
    exit 1
}

echo ""
echo "Connection OK. Running eval..."
python scripts/eval_round1.py --days 60 --instruments SPY,BTC-USD,ES=F,GC=F
