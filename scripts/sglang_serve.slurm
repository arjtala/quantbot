#!/bin/bash
#SBATCH --job-name=quantbot-llm
#SBATCH --partition=gpu          # adjust to your cluster's GPU partition
#SBATCH --gres=gpu:h200:1       # 1 H200 for 32B models, increase for larger
#SBATCH --cpus-per-task=16
#SBATCH --mem=64G
#SBATCH --time=24:00:00
#SBATCH --output=sglang-server-%j.out
#SBATCH --error=sglang-server-%j.err

# ============================================================================
# QuantBot — SGLang Model Server
#
# Serves an LLM via OpenAI-compatible API for the quantbot eval pipeline.
#
# Recommended models (adjust MODEL below):
#   1 H200:  deepseek-ai/DeepSeek-R1-Distill-Qwen-32B  (best reasoning/GPU)
#   1 H200:  Qwen/Qwen3-32B                              (strong reasoning)
#   1 H200:  TheFinAI/Fin-R1-7B                           (finance-specialized)
#   2 H200:  Qwen/Qwen3-235B-A22B  (MoE, --tp 2)
#   4 H200:  deepseek-ai/DeepSeek-V3.2  (--tp 4)
#   8 H200:  deepseek-ai/DeepSeek-R1  (--tp 8)
# ============================================================================

# --- Configuration ---
MODEL="deepseek-ai/DeepSeek-R1-Distill-Qwen-32B"
PORT=30000
TP=1  # tensor parallelism — set to number of GPUs

# --- Activate environment ---
source activate quantbot-cluster || conda activate quantbot-cluster

# --- Print connection info ---
echo "============================================"
echo "  SGLang Model Server"
echo "  Model:  $MODEL"
echo "  Node:   $(hostname)"
echo "  Port:   $PORT"
echo "  TP:     $TP"
echo "  URL:    http://$(hostname):$PORT/v1"
echo "============================================"
echo ""
echo "To use from quantbot, set in .env:"
echo "  OPENAI_BASE_URL=http://$(hostname):$PORT/v1"
echo "  INDICATOR_MODEL=sglang:$MODEL"
echo ""

# --- Launch SGLang server ---
python -m sglang.launch_server \
    --model-path "$MODEL" \
    --port "$PORT" \
    --tp "$TP" \
    --trust-remote-code \
    --log-level info
