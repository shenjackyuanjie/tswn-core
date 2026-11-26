#!/usr/bin/env python3
"""将一天新增的、已标注的 PBB 命中候选增量训练进既有 CatBoost 强度模型。

不读取历史全量训练集：每日只训练新分区，加一个固定小型 replay 集来保持旧分布。
输入必须有 target_win_rate（由 tswn-core 实测产生）；PBB 分数只能作为特征。
"""
import argparse, csv, hashlib, json, math, shutil
from datetime import datetime, timezone
from pathlib import Path

FORBIDDEN = {"candidate_id", "target_win_rate", "split", "name", "team", "raw_input"}

def split_of(cid):
    return ("train", "valid", "test")[0 if int.from_bytes(hashlib.sha256(cid.encode()).digest()[:8], "big") % 100 < 80 else 1 if int.from_bytes(hashlib.sha256(cid.encode()).digest()[:8], "big") % 100 < 90 else 2]

def load(path, cols):
    with path.open(encoding="utf-8", newline="") as f:
        reader = csv.DictReader(f)
        needed = {"candidate_id", "target_win_rate", *cols}
        if not reader.fieldnames or needed - set(reader.fieldnames):
            raise ValueError(f"{path} 缺列：{sorted(needed-set(reader.fieldnames or []))}")
        rows = list(reader)
    for r in rows:
        r["target_win_rate"] = float(r["target_win_rate"])
        if not 0 <= r["target_win_rate"] <= 1: raise ValueError(f"非法胜率：{r['candidate_id']}")
        for c in cols: r[c] = float(r[c])
    return rows

def features(rows, cols): return [[r[c] for c in cols] for r in rows]

def metric(y, p):
    if not y: return None
    mae = sum(abs(a-b) for a,b in zip(y,p))/len(y)
    rmse = math.sqrt(sum((a-b)**2 for a,b in zip(y,p))/len(y))
    return {"n":len(y), "mae":mae, "rmse":rmse}

def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--new-data", type=Path, required=True, help="当天新增、已含 TSWN 胜率标签的 CSV")
    ap.add_argument("--day", required=True, help="YYYY-MM-DD，作为不可变训练分区名")
    ap.add_argument("--schema", type=Path, required=True)
    ap.add_argument("--base-model", type=Path, required=True)
    ap.add_argument("--dataset-dir", type=Path, required=True, help="每日分区和 candidate 注册表目录")
    ap.add_argument("--output-dir", type=Path, required=True)
    ap.add_argument("--replay-data", type=Path, help="固定小型历史标注回放集；建议 1–5 万行")
    ap.add_argument("--threads", type=int, default=40)
    ap.add_argument("--iterations", type=int, default=300)
    ap.add_argument("--seed", type=int, default=42)
    a = ap.parse_args()
    if a.threads < 1 or a.iterations < 1: raise SystemExit("threads/iterations 必须为正数")
    schema = json.loads(a.schema.read_text(encoding="utf-8")); cols = schema["feature_columns"]
    if set(cols) & FORBIDDEN: raise SystemExit("schema 含泄漏字段")
    new = load(a.new_data, cols)
    if not new: raise SystemExit("当天标签集为空；PBB 命中必须先经 tswn-core 生成 target_win_rate")
    if len({r['candidate_id'] for r in new}) != len(new): raise SystemExit("当天数据 candidate_id 重复")
    a.dataset_dir.mkdir(parents=True, exist_ok=True)
    registry = a.dataset_dir / "candidate_registry.txt"
    seen = set(registry.read_text(encoding="utf-8").splitlines()) if registry.exists() else set()
    overlap = sorted({r['candidate_id'] for r in new} & seen)
    if overlap: raise SystemExit(f"当天数据与既有训练集重复 {len(overlap)} 条，例如 {overlap[0]}")
    replay = load(a.replay_data, cols) if a.replay_data else []
    train_new = [r for r in new if split_of(r['candidate_id']) == 'train']
    valid_new = [r for r in new if split_of(r['candidate_id']) == 'valid']
    test_new = [r for r in new if split_of(r['candidate_id']) == 'test']
    if not train_new or not valid_new or not test_new: raise SystemExit("当天数据过少，稳定划分后 train/valid/test 不完整")
    try: from catboost import CatBoostRegressor
    except ImportError as e: raise SystemExit("缺少 CatBoost") from e
    base_model = CatBoostRegressor(); base_model.load_model(str(a.base_model))
    model = CatBoostRegressor(loss_function="RMSE", random_seed=a.seed)
    model.fit(features(replay + train_new, cols), [r['target_win_rate'] for r in replay + train_new],
              eval_set=(features(valid_new, cols), [r['target_win_rate'] for r in valid_new]),
              init_model=base_model, iterations=a.iterations, thread_count=a.threads,
              verbose=100, early_stopping_rounds=50, allow_writing_files=False)
    a.output_dir.mkdir(parents=True, exist_ok=True); model.save_model(str(a.output_dir / "strength_model.cbm"))
    daily = a.dataset_dir / f"daily_{a.day}.csv"
    if daily.exists(): raise SystemExit(f"日分区已存在，拒绝覆盖：{daily}")
    shutil.copyfile(a.new_data, daily)
    with registry.open("a", encoding="utf-8") as f:
        for r in new: f.write(r['candidate_id'] + "\n")
    pred = [max(0,min(1,float(x))) for x in model.predict(features(test_new, cols))]
    meta = {"created_at":datetime.now(timezone.utc).isoformat(), "day":a.day,
            "new_rows":len(new), "replay_rows":len(replay), "base_model":str(a.base_model),
            "daily_partition":str(daily), "test_metrics":metric([r['target_win_rate'] for r in test_new], pred),
            "feature_columns":cols, "note":"增量模型；PBB 特征不是胜率标签。"}
    (a.output_dir / "incremental_metadata.json").write_text(json.dumps(meta, ensure_ascii=False, indent=2)+"\n", encoding="utf-8")
    print(json.dumps(meta, ensure_ascii=False, indent=2))

if __name__ == "__main__": main()
