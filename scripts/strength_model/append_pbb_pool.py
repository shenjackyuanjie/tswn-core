#!/usr/bin/env python3
"""将一天的 PBB 完整特征快照去重追加到长期候选池，不重写旧分区。"""
import argparse, csv, json
from datetime import datetime, timezone
from pathlib import Path

def read(path):
    with path.open(encoding="utf-8", newline="") as f:
        r=csv.DictReader(f)
        if not r.fieldnames or "candidate_id" not in r.fieldnames: raise ValueError("缺少 candidate_id")
        return r.fieldnames, list(r)

def main():
    p=argparse.ArgumentParser(); p.add_argument("--input",type=Path,required=True); p.add_argument("--day",required=True)
    p.add_argument("--pool",type=Path,required=True); p.add_argument("--manifest",type=Path,required=True); a=p.parse_args()
    fields, incoming=read(a.input)
    if len({r['candidate_id'] for r in incoming})!=len(incoming): raise SystemExit("当天快照含重复 candidate_id")
    if a.pool.exists():
        old_fields, old=read(a.pool)
        if old_fields!=fields+["first_seen_day"]: raise SystemExit("候选池 schema 不匹配")
    else: old=[]
    seen={r['candidate_id'] for r in old}; new=[]
    for r in incoming:
        if r['candidate_id'] not in seen:
            new.append({**r,"first_seen_day":a.day}); seen.add(r['candidate_id'])
    a.pool.parent.mkdir(parents=True,exist_ok=True)
    with a.pool.open("w",encoding="utf-8",newline="") as f:
        w=csv.DictWriter(f,fieldnames=fields+["first_seen_day"]); w.writeheader(); w.writerows(old); w.writerows(new)
    history=json.loads(a.manifest.read_text(encoding="utf-8")) if a.manifest.exists() else []
    history.append({"day":a.day,"input":str(a.input),"input_rows":len(incoming),"new_rows":len(new),"pool_rows":len(old)+len(new),"at":datetime.now(timezone.utc).isoformat()})
    a.manifest.parent.mkdir(parents=True,exist_ok=True); a.manifest.write_text(json.dumps(history,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
    print(f"input={len(incoming)} new={len(new)} pool={len(old)+len(new)}")
if __name__=="__main__": main()
