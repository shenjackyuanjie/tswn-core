#!/usr/bin/env bash
# PBB 新一天的结果入池后，重新混合新旧候选并写出五席的人工分析报告。
set -euo pipefail

PROJECT_DIR="/rdxfs/user/lulab/xiongyk/projects/name_arena"
CORE_DIR="$PROJECT_DIR/tswn-core"
PYTHON_BIN="$PROJECT_DIR/PBB-linux/PBB/.linux-env/bin/python"
CLI="$CORE_DIR/target/debug/tswn-cli"
DAY="${1:?用法: refresh_daily_five_seats.sh YYYY-MM-DD}"
MODEL_DIR="$CORE_DIR/data/website_rank_models_pbb_2026-10-22_skill"
REFERENCE_DIR="$CORE_DIR/data/website_rank_models_pbb_2026-10-22_skill"
OUT_DIR="$CORE_DIR/data/daily_five_seats/$DAY"

[[ -x "$PYTHON_BIN" && -x "$CLI" ]] || { echo "缺少 Python 或 tswn-cli" >&2; exit 1; }
[[ -f "$MODEL_DIR/single_model/model.cbm" && -f "$REFERENCE_DIR/single_website_training.csv" ]] || { echo "缺少网站排名模型或训练参照" >&2; exit 1; }

"$PYTHON_BIN" "$CORE_DIR/scripts/strength_model/select_competition_candidates.py" \
  --pool "$CORE_DIR/data/rank_data/pbb_candidate_pool.csv" --cli "$CLI" \
  --models-dir "$MODEL_DIR" --reference-training-dir "$REFERENCE_DIR" --output-dir "$OUT_DIR" \
  --screen-size 160 --per-day 24 --single-beam 48 --per-day-beam 8 \
  --duo-seeds 24 --duo-partners 48 --duo-beam 48 --mixed-duo-beam 16 \
  --trio-seeds 20 --trio-partners 36 --trio-beam 24 --threads 40

"$PYTHON_BIN" "$CORE_DIR/scripts/strength_model/write_five_seat_report.py" \
  --finalists "$OUT_DIR/provisional_finalists.csv" --cli "$CLI" \
  --output "$OUT_DIR/five_seats_skills_and_scores.md" --rounds 10000

echo "每日五席报告已完成：$OUT_DIR/five_seats_skills_and_scores.md"
