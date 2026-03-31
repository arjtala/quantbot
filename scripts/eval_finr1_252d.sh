#!/bin/bash
# QuantBot — 252-day Fin-R1 eval (in-sample + out-of-sample)
#
# Launches Fin-R1 (7B) on 1 H200, waits for server, runs 252-day eval,
# then runs combiner simulation on the results.
#
# Usage:
#   bash scripts/eval_finr1_252d.sh
#
# Results:
#   eval_results/finr1-252d/          — 21 instrument CSVs (252 days each)
#   eval_results/finr1-252d-summary/  — combiner simulation output

set -e

WORKERS=4
DAYS=252
DATA_DIR="data"
RUN_NAME="finr1-252d"
MODEL="SUFE-AIFLM-Lab/Fin-R1"
PORT=30000
LOG_DIR="logs"

mkdir -p "$LOG_DIR"

echo "========================================================================"
echo "  252-DAY FIN-R1 EVAL — 21 instruments × ${DAYS} days = $((21 * DAYS)) LLM calls"
echo "========================================================================"
echo ""
echo "  Model:   $MODEL (7B, finance-specialized)"
echo "  GPU:     1× H200"
echo "  Est:     ~1-3 hours at ~2-5s/call with $WORKERS workers"
echo ""

# --- Step 1: Submit Fin-R1 server job ---
echo "Submitting Fin-R1 SGLang server..."

JOBID=$(sbatch --parsable \
    --job-name="qb-finr1-252" \
    --account=dream \
    --qos=h200_comm_shared \
    --gres=gpu:h200:1 \
    --cpus-per-task=16 \
    --mem=32G \
    --time=24:00:00 \
    --output="${LOG_DIR}/sglang-finr1-252d-%j.out" \
    --error="${LOG_DIR}/sglang-finr1-252d-%j.err" \
    --wrap="#!/bin/bash
eval \"\$(conda shell.bash hook 2>/dev/null)\"
conda activate quantbot-cluster
echo \"NODE=\$(hostname) PORT=${PORT} MODEL=${MODEL}\"
python -m sglang.launch_server \
    --model-path '${MODEL}' \
    --host 0.0.0.0 \
    --port ${PORT} \
    --tp 1 \
    --trust-remote-code \
    --log-level info
")

echo "  Job ${JOBID} submitted"
echo ""

# --- Step 2: Wait for job to start ---
echo -n "Waiting for job to start..."

while true; do
    STATE=$(squeue -j "$JOBID" -h -o "%T" 2>/dev/null || echo "UNKNOWN")
    if [ "$STATE" = "RUNNING" ]; then
        NODE=$(squeue -j "$JOBID" -h -o "%N" 2>/dev/null)
        echo " RUNNING on ${NODE}"
        break
    elif [ "$STATE" = "PENDING" ]; then
        echo -n "."
        sleep 10
    else
        echo " FAILED (state: ${STATE})"
        echo "Check: sacct -j ${JOBID} --format=State,ExitCode"
        exit 1
    fi
done

echo ""

# --- Step 3: Wait for SGLang server to load model ---
echo -n "Waiting for Fin-R1 to load (7B, should be <2 min)..."

for i in $(seq 1 120); do
    if curl -s --noproxy "$NODE" --max-time 3 "http://${NODE}:${PORT}/v1/models" >/dev/null 2>&1; then
        echo " READY"
        break
    fi
    if [ "$i" -eq 120 ]; then
        echo " TIMEOUT — server didn't respond after 20 min"
        echo "Check: cat ${LOG_DIR}/sglang-finr1-252d-${JOBID}.out"
        scancel "$JOBID" 2>/dev/null
        exit 1
    fi
    echo -n "."
    sleep 10
done

# Verify model identity
echo ""
echo "Server response:"
curl -s --noproxy "$NODE" "http://${NODE}:${PORT}/v1/models" | python3 -m json.tool
echo ""

# --- Step 4: Run 252-day eval ---
export no_proxy="$NODE"
export NO_PROXY="$NODE"
export OPENAI_BASE_URL="http://${NODE}:${PORT}/v1"
export OPENAI_API_KEY="not-needed"
export DEFAULT_PROVIDER="sglang"
export INDICATOR_MODEL="sglang:${MODEL}"

echo "Starting 252-day eval..."
echo ""

EVAL_START=$(date +%s)

python scripts/eval_round1.py \
    --days "$DAYS" \
    --data-dir "$DATA_DIR" \
    --workers "$WORKERS" \
    --run-name "$RUN_NAME"

EVAL_END=$(date +%s)
EVAL_ELAPSED=$(( EVAL_END - EVAL_START ))
EVAL_MIN=$(( EVAL_ELAPSED / 60 ))
EVAL_SEC=$(( EVAL_ELAPSED % 60 ))

echo ""
echo "========================================================================"
echo "  EVAL COMPLETE — ${EVAL_MIN}m ${EVAL_SEC}s"
echo "========================================================================"
echo ""
echo "  Results: eval_results/${RUN_NAME}/"
echo ""

# --- Step 5: Run combiner simulation on results ---
echo "Running combiner simulation on 252-day results..."
echo ""

python scripts/analyze_eval.py --model-dirs "eval_results/${RUN_NAME}" 2>/dev/null || {
    echo "  (analyze_eval.py didn't accept --model-dirs, running default)"
    echo "  To analyze: python scripts/analyze_eval.py"
}

# --- Step 6: Cancel server job ---
echo ""
echo "Cleaning up Fin-R1 server (job ${JOBID})..."
scancel "$JOBID" 2>/dev/null && echo "  Cancelled" || echo "  Already finished"

echo ""
echo "========================================================================"
echo "  DONE"
echo "========================================================================"
echo ""
echo "  Next steps:"
echo "    1. Review results in eval_results/${RUN_NAME}/"
echo "    2. Run combiner simulation: python scripts/analyze_eval.py"
echo "    3. If Sharpe holds near 0.793 → Phase 3 is GO"
echo ""
