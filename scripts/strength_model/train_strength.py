#!/usr/bin/env python3
"""按候选角色隔离训练 CPU CatBoost 1v1 静态强度回归模型。"""
import argparse, csv, hashlib, json, math, platform, sys
from datetime import datetime
from pathlib import Path

FORBIDDEN = {"candidate_id", "target_win_rate", "split", "seed", "raw_input", "name", "display_name"}

def load_csv(path: Path):
    with path.open(newline="", encoding="utf-8") as f:
        r = csv.DictReader(f)
        if not r.fieldnames: raise ValueError(f"{path} 缺少 CSV 表头")
        return r.fieldnames, list(r)

def split_of(candidate_id: str) -> str:
    bucket = int.from_bytes(hashlib.sha256(candidate_id.encode()).digest()[:8], "big") % 100
    return "train" if bucket < 80 else "valid" if bucket < 90 else "test"

def metrics(y, p):
    mae = sum(abs(a-b) for a,b in zip(y,p))/len(y)
    rmse = math.sqrt(sum((a-b)**2 for a,b in zip(y,p))/len(y))
    ma, mp = sum(y)/len(y), sum(p)/len(p)
    den = math.sqrt(sum((x-ma)**2 for x in y)*sum((x-mp)**2 for x in p))
    return {"n":len(y), "mae":mae, "rmse":rmse, "pearson":sum((a-ma)*(b-mp) for a,b in zip(y,p))/den if den else None}

def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--data", type=Path, required=True); ap.add_argument("--schema", type=Path, required=True)
    ap.add_argument("--output-dir", type=Path, required=True); ap.add_argument("--threads", type=int, default=40)
    ap.add_argument("--iterations", type=int, default=2000); ap.add_argument("--seed", type=int, default=42)
    ap.add_argument("--use-input-split", action="store_true")
    a = ap.parse_args()
    if a.threads < 1: raise SystemExit("--threads 必须至少为 1")
    try: from catboost import CatBoostRegressor
    except ImportError as e: raise SystemExit("缺少 CatBoost；执行 pip install -r scripts/strength_model/requirements.txt") from e
    schema = json.loads(a.schema.read_text(encoding="utf-8")); cols = schema.get("feature_columns")
    if not isinstance(cols,list) or not cols or not all(isinstance(x,str) for x in cols): raise SystemExit("schema.feature_columns 必须是非空字符串列表")
    if set(cols)&FORBIDDEN: raise SystemExit(f"特征白名单含泄漏列：{sorted(set(cols)&FORBIDDEN)}")
    fields, rows = load_csv(a.data); required={"candidate_id","target_win_rate",*cols}
    if a.use_input_split: required.add("split")
    if required-set(fields): raise SystemExit(f"数据缺列：{sorted(required-set(fields))}")
    sets={"train":[],"valid":[],"test":[]}; seen=set()
    for row in rows:
        cid=row["candidate_id"]
        if cid in seen: raise SystemExit(f"candidate_id 重复：{cid}")
        seen.add(cid); split=row["split"] if a.use_input_split else split_of(cid)
        if split not in sets: raise SystemExit(f"非法 split：{split}")
        try: y=float(row["target_win_rate"]); x=[float(row[c]) for c in cols]
        except ValueError as e: raise SystemExit(f"{cid} 标签或特征非数值：{e}")
        if not 0<=y<=1 or not all(math.isfinite(v) for v in x): raise SystemExit(f"{cid} 含非法数值")
        sets[split].append((row,x,y))
    if not all(sets.values()): raise SystemExit("train、valid、test 都必须非空")
    X=lambda k:[x for _,x,_ in sets[k]]; Y=lambda k:[y for _,_,y in sets[k]]
    model=CatBoostRegressor(loss_function="RMSE",iterations=a.iterations,depth=7,learning_rate=.04,l2_leaf_reg=10,random_seed=a.seed,thread_count=a.threads,verbose=100,allow_writing_files=False)
    model.fit(X("train"),Y("train"),eval_set=(X("valid"),Y("valid")),early_stopping_rounds=100,use_best_model=True)
    a.output_dir.mkdir(parents=True,exist_ok=True); model.save_model(str(a.output_dir/"strength_model.cbm"))
    pred={k:[max(0,min(1,float(v))) for v in model.predict(X(k))] for k in sets}
    meta={"created_at":datetime.utcnow().isoformat()+"Z","python":sys.version,"platform":platform.platform(),"schema_version":schema.get("schema_version"),"feature_columns":cols,"thread_count":a.threads,"seed":a.seed,"best_iteration":model.get_best_iteration(),"splits":{k:metrics(Y(k),pred[k]) for k in sets}}
    (a.output_dir/"metadata.json").write_text(json.dumps(meta,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
    with (a.output_dir/"test_predictions.csv").open("w",newline="",encoding="utf-8") as f:
        w=csv.DictWriter(f,fieldnames=["candidate_id","target_win_rate","predicted_win_rate","predicted_score"]); w.writeheader()
        for (row,_,y),p in zip(sets["test"],pred["test"]): w.writerow({"candidate_id":row["candidate_id"],"target_win_rate":y,"predicted_win_rate":p,"predicted_score":100*p})
    print(json.dumps(meta["splits"],ensure_ascii=False,indent=2))
if __name__ == "__main__": main()
