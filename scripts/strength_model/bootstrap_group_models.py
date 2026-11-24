#!/usr/bin/env python3
"""以真实 TSWN 组队构建和组对组胜率，生成 single/duo/trio 的小型初训集并训练三个模型。"""
import argparse, csv, random, re, subprocess
from pathlib import Path

SCORES = ("pp", "pd", "qp", "qd", "sum")

def call(args):
    return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT)

def group_scores(cli, group):
    out = call([str(cli), "namer-pf", "-r", group, "-n", "16", "--keep-rq"])
    found = {k: float(v) for k, v in re.findall(r"\b(pp|pd|qp|qd|sum):(\d+(?:\.\d+)?)", out)}
    if set(found) != set(SCORES): raise RuntimeError(f"无法解析组级 TSWN 特征：{out}")
    return found

def rate(cli, target, opponents, games):
    args = [str(cli), "bench", "group-win-rate", "-l", target, "-n", str(games), "--single-thread", "--keep-rq"]
    for x in opponents: args += ["-a", x]
    out = call(args)
    m = re.search(r"平均胜率:\s*([0-9.]+)%", out)
    if not m: raise RuntimeError(f"无法解析组胜率：{out}")
    return float(m.group(1)) / 100

def train(rows, outdir, threads):
    from catboost import CatBoostRegressor
    cols = ["tswn_"+x for x in SCORES]
    rnd = random.Random(42); rnd.shuffle(rows)
    n = len(rows); train_rows, valid_rows, test_rows = rows[:int(.75*n)], rows[int(.75*n):int(.875*n)], rows[int(.875*n):]
    x = lambda rs: [[float(r[c]) for c in cols] for r in rs]
    y = lambda rs: [float(r["target_win_rate"]) for r in rs]
    model = CatBoostRegressor(loss_function="RMSE", iterations=300, depth=5, learning_rate=.05, l2_leaf_reg=10, thread_count=threads, verbose=False, allow_writing_files=False)
    model.fit(x(train_rows), y(train_rows), eval_set=(x(valid_rows), y(valid_rows)), early_stopping_rounds=40)
    model.save_model(str(outdir / "model.cbm"))
    pred = model.predict(x(test_rows)); mae = sum(abs(a-b) for a,b in zip(y(test_rows),pred))/len(test_rows)
    (outdir / "metrics.txt").write_text(f"rows={n}\ntest_mae={mae:.6f}\nfeatures={','.join(cols)}\n", encoding="utf-8")

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--candidates", type=Path, required=True, help="PBB 原始命中 TXT")
    ap.add_argument("--cli", type=Path, required=True)
    ap.add_argument("--output-dir", type=Path, required=True)
    ap.add_argument("--samples", type=int, default=48)
    ap.add_argument("--opponents", type=int, default=3)
    ap.add_argument("--games", type=int, default=32)
    ap.add_argument("--threads", type=int, default=20)
    ap.add_argument("--seed", type=int, default=20261019)
    a = ap.parse_args(); a.output_dir.mkdir(parents=True, exist_ok=True)
    names = [line.split()[0] for line in a.candidates.read_text(encoding="utf-8").splitlines() if "@" in line]
    if len(names) < a.samples * 6: raise SystemExit("候选不足")
    rng = random.Random(a.seed)
    for size, kind in ((1,"single"),(2,"duo"),(3,"trio")):
        groups = ["+".join(rng.sample(names, size)) for _ in range(a.samples + a.opponents)]
        opponents, targets = groups[:a.opponents], groups[a.opponents:]
        rows = []
        for i, group in enumerate(targets, 1):
            feat = group_scores(a.cli, group); label = rate(a.cli, group, opponents, a.games)
            rows.append({"group_id":group, "target_win_rate":label, **{"tswn_"+k:v for k,v in feat.items()}})
            print(f"{kind} {i}/{len(targets)} {label:.4f}", flush=True)
        path = a.output_dir / f"{kind}_bootstrap.csv"
        with path.open("w", newline="", encoding="utf-8") as f:
            w=csv.DictWriter(f, fieldnames=["group_id","target_win_rate",*["tswn_"+x for x in SCORES]]); w.writeheader(); w.writerows(rows)
        model_dir = a.output_dir / f"{kind}_model"; model_dir.mkdir(exist_ok=True); train(rows, model_dir, a.threads)

if __name__ == "__main__": main()
