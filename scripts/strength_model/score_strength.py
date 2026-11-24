#!/usr/bin/env python3
"""用保存的静态强度模型批量给未见候选打分。"""
import argparse, csv, json, math
from pathlib import Path
def main():
    ap=argparse.ArgumentParser(description=__doc__); ap.add_argument("--features",type=Path,required=True); ap.add_argument("--model-dir",type=Path,required=True); ap.add_argument("--output",type=Path,required=True); a=ap.parse_args()
    try: from catboost import CatBoostRegressor
    except ImportError as e: raise SystemExit("缺少 CatBoost；执行 pip install -r scripts/strength_model/requirements.txt") from e
    cols=json.loads((a.model_dir/"metadata.json").read_text(encoding="utf-8"))["feature_columns"]
    with a.features.open(newline="",encoding="utf-8") as f:
        r=csv.DictReader(f)
        if not r.fieldnames or "candidate_id" not in r.fieldnames or set(cols)-set(r.fieldnames): raise SystemExit("features 缺 candidate_id 或训练期特征")
        rows=list(r)
    X=[[float(row[c]) for c in cols] for row in rows]
    if not all(math.isfinite(v) for x in X for v in x): raise SystemExit("features 含非有限数值")
    m=CatBoostRegressor(); m.load_model(str(a.model_dir/"strength_model.cbm")); scores=m.predict(X); a.output.parent.mkdir(parents=True,exist_ok=True)
    with a.output.open("w",newline="",encoding="utf-8") as f:
        w=csv.DictWriter(f,fieldnames=["candidate_id","predicted_win_rate","predicted_score"]); w.writeheader()
        for row,s in sorted(zip(rows,scores),key=lambda x:float(x[1]),reverse=True):
            s=max(0,min(1,float(s))); w.writerow({"candidate_id":row["candidate_id"],"predicted_win_rate":f"{s:.12g}","predicted_score":f"{100*s:.10g}"})
if __name__ == "__main__": main()
