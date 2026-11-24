#!/usr/bin/env python3
"""用网站公开榜单的全部实际上榜阵容及名次训练 single/duo/trio 排名模型。"""
import argparse, csv, hashlib, json, random, re, subprocess
from collections import defaultdict
from pathlib import Path

from skill_features import skill_cols_for, single_skill_features, group_skill_features

SCORES = ("pp", "pd", "qp", "qd", "sum")
FEATURES = ["tswn_" + x for x in SCORES]
PBB_CORE = ("pbb_sum", "pbb_score_qp", "pbb_score_qd", "pbb_score_pp", "pbb_score_pd")
KIND_FORMATS = {"single":{"1a","1b"}, "duo":{"2a","2b"}, "trio":{"3a"}}

def call(args): return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT)

def features(cli, group, cache, precomputed):
    if group in precomputed:
        return precomputed[group]
    if group not in cache:
        out=call([str(cli), "namer-pf", "-r", group, "-n", "16", "--keep-rq"])
        f={"tswn_"+k:float(v) for k,v in re.findall(r"\b(pp|pd|qp|qd|sum):(\d+(?:\.\d+)?)",out)}
        if set(f)!=set(FEATURES): raise RuntimeError("无法解析："+out)
        cache[group]=f
    return cache[group]

def deterministic_split(group):
    v=int.from_bytes(hashlib.sha256(group.encode()).digest()[:8],"big")%100
    return "train" if v<75 else "valid" if v<87 else "test"

def fit(kind, rows, outdir, threads, columns):
    from catboost import CatBoostRegressor
    buckets=defaultdict(list)
    for r in rows: buckets[deterministic_split(r["group"])].append(r)
    # 小榜单可能随机切分为空；固定种子回退，仍保留独立测试行。
    if not all(buckets.values()):
        rng=random.Random(20261019); rng.shuffle(rows); n=len(rows)
        buckets={"train":rows[:max(1,int(.75*n))], "valid":rows[max(1,int(.75*n)):max(2,int(.87*n))], "test":rows[max(2,int(.87*n)):]} 
    x=lambda part:[[float(r[c]) for c in columns] for r in buckets[part]]
    y=lambda part:[float(r["rank_score"]) for r in buckets[part]]
    m=CatBoostRegressor(loss_function="RMSE",iterations=600,depth=5,learning_rate=.04,l2_leaf_reg=12,
                         random_seed=20261019,thread_count=threads,verbose=False,allow_writing_files=False)
    m.fit(x("train"),y("train"),eval_set=(x("valid"),y("valid")),early_stopping_rounds=80)
    d=outdir/f"{kind}_model"; d.mkdir(parents=True,exist_ok=True); m.save_model(str(d/"model.cbm"))
    pred=m.predict(x("test")); mae=sum(abs(a-b) for a,b in zip(y("test"),pred))/len(pred)
    (d/"metrics.json").write_text(json.dumps({"rows":len(rows),"test_mae":mae,"features":columns,
        "splits":{k:len(v) for k,v in buckets.items()},"training_source":"website public ladder snapshots"},ensure_ascii=False,indent=2)+"\n",encoding="utf-8")

def main():
    ap=argparse.ArgumentParser(); ap.add_argument("--snapshots-dir",type=Path,required=True); ap.add_argument("--cli",type=Path,required=True)
    ap.add_argument("--output-dir",type=Path,required=True); ap.add_argument("--threads",type=int,default=20)
    ap.add_argument("--pbb-features",type=Path,required=True,help="由 export_website_pbb_features.py 导出的成员特征")
    ap.add_argument("--feature-cache",type=Path,default=None,
                    help="批量 namer-pf 预计算的组级五项评分 JSON（{组串: {tswn_pp..tswn_sum}}），命中时不逐组调用 CLI")
    a=ap.parse_args()
    raw=[]
    for p in sorted(a.snapshots_dir.glob("ladder_*.csv")):
        with p.open(encoding="utf-8",newline="") as f: raw.extend(csv.DictReader(f))
    seen=set(); unique=[]
    for row in raw:
        key=(row["snapshot_id"], row["format"], row["group"])
        if key not in seen:
            seen.add(key); unique.append(row)
    raw=unique
    if not raw: raise SystemExit("没有快照")
    sizes=defaultdict(int)
    for r in raw: sizes[(r["snapshot_id"],r["format"])]+=1
    with a.pbb_features.open(encoding="utf-8",newline="") as f:
        pbb={r["candidate_id"]:r for r in csv.DictReader(f)}
    cache={}; bykind=defaultdict(list)
    import json as _json
    precomputed = _json.loads(a.feature_cache.read_text(encoding="utf-8")) if a.feature_cache else {}
    pbb_cols=[f"{field}_{agg}" for field in PBB_CORE for agg in ("sum","mean","min","max")]
    for i,r in enumerate(raw,1):
        kind=next((k for k,fs in KIND_FORMATS.items() if r["format"] in fs),None)
        if not kind: continue
        n=sizes[(r["snapshot_id"],r["format"])]
        rank=int(r["rank"]); score=1.0 if n==1 else 1-(rank-1)/(n-1)
        members=r["group"].split("+")
        missing=[m for m in members if m not in pbb]
        if missing: raise RuntimeError(f"PBB 特征缺少网站成员：{missing[0]}")
        aggregates={}
        for field in PBB_CORE:
            values=[float(pbb[m][field]) for m in members]
            aggregates.update({f"{field}_sum":sum(values),f"{field}_mean":sum(values)/len(values),
                               f"{field}_min":min(values),f"{field}_max":max(values)})
        skill = (single_skill_features if kind == "single" else group_skill_features)(
            [pbb[m] for m in members])
        bykind[kind].append({**r,"rank_score":score,**features(a.cli,r["group"],cache,precomputed),**aggregates,**skill})
        if i%50==0: print(f"features {i}/{len(raw)}",flush=True)
    a.output_dir.mkdir(parents=True,exist_ok=True)
    for kind,rows in bykind.items():
        skill_cols = list(skill_cols_for(kind))
        with (a.output_dir/f"{kind}_website_training.csv").open("w",newline="",encoding="utf-8") as f:
            w=csv.DictWriter(f,fieldnames=["snapshot_id","source_url","format","rank","website_score","team","group","rank_score",*FEATURES,*pbb_cols,*skill_cols]);w.writeheader();w.writerows(rows)
        fit(kind,rows,a.output_dir,a.threads,FEATURES+pbb_cols+skill_cols)
        print(f"{kind}: {len(rows)} rows",flush=True)

if __name__=="__main__": main()
