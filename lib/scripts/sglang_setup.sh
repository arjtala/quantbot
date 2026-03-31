#!/bin/bash
# QuantBot — SGLang Setup on SLURM Cluster (H200 GPUs)
#
# Usage:
#   1. Clone quantbot repo on the cluster
#   2. Run this script to install deps: bash scripts/sglang_setup.sh
#   3. Launch model server: sbatch scripts/sglang_serve.slurm
#   4. Run eval: python scripts/eval_round1.py --days 60
#
# Prerequisites:
#   - SLURM cluster with H200 GPUs
#   - conda or venv available
#   - Internet access for model downloads (first run only)

set -e

echo "=== QuantBot SGLang Setup ==="

# Create conda env if it doesn't exist
if ! conda env list | grep -q quantbot-cluster; then
    echo "Creating conda environment..."
    conda create -n quantbot-cluster python=3.12 -y
fi

echo "Activating environment..."
source activate quantbot-cluster || conda activate quantbot-cluster

echo "Installing quantbot + dependencies..."
pip install -e ".[dev]"

echo "Installing SGLang..."
pip install "sglang[all]>=0.4" --find-links https://flashinfer.ai/whl/cu124/torch2.5/flashinfer-python

echo "Installing langchain-openai (for SGLang OpenAI-compatible endpoint)..."
pip install langchain-openai

echo ""
echo "=== Setup Complete ==="
echo ""
echo "Next steps:"
echo "  1. Launch model server:  sbatch scripts/sglang_serve.slurm"
echo "  2. Wait for it to start: squeue -u \$USER"
echo "  3. Find the node:        grep 'port' slurm-*.out"
echo "  4. Create .env file with OPENAI_BASE_URL pointing to the server"
echo "  5. Run eval:             python scripts/eval_round1.py --days 60"
