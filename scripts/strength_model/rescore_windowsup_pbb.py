#!/usr/bin/env python3
"""用当前 Linux PBB 并行复算 Windowsup 历史名字，并按宽松近阈值保留。"""
import argparse
import csv
import shutil
import subprocess
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

# 当前 PBB 的默认阈值乘 0.80：保留接近当前筛选线的老 PBB 成果，排除明显随机低分。
RELAXED = {"pbb_score_qp": 4320, "pbb_score_qd": 4960, "pbb_score_cqd": 3200,
           "pbb_score_cp": 3280, "pbb_score_fp": 3360, "pbb_score_wc": 3200,
           "pbb_score_fs": 3120, "pbb_score_pj": 3280}


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--input", type=Path, required=True, help="name<TAB>team TSV")
    ap.add_argument("--exporter", type=Path, required=True)
    ap.add_argument("--output", type=Path, required=True, help="所有名字的当前 PBB 特征")
    ap.add_argument("--kept", type=Path, required=True, help="近阈值保留后的特征")
    ap.add_argument("--work-dir", type=Path, required=True)
    ap.add_argument("--workers", type=int, default=24)
    ap.add_argument("--cpu-set", default="0-11,28-39")
    args = ap.parse_args()
    if args.workers < 1 or args.workers > 24:
        raise SystemExit("workers 必须在 1--24")
    lines = args.input.read_text(encoding="utf-8").splitlines(keepends=True)
    if not lines: raise SystemExit("输入为空")
    args.work_dir.mkdir(parents=True, exist_ok=True)
    for path in args.work_dir.glob("part-*"):
        path.unlink()
    chunks = [[] for _ in range(args.workers)]
    for index, line in enumerate(lines): chunks[index % args.workers].append(line)

    cpu_ids = [*range(0, 12), *range(28, 40)]
    def run(index):
        source = args.work_dir / f"part-{index:02d}.tsv"
        result = args.work_dir / f"part-{index:02d}.csv"
        source.write_text("".join(chunks[index]), encoding="utf-8")
        with source.open("rb") as incoming, result.open("wb") as outgoing:
            command = ["taskset", "-c", str(cpu_ids[index]), str(args.exporter)]
            subprocess.run(command, stdin=incoming, stdout=outgoing, check=True)
        return result

    print(f"重算 {len(lines)} 个名字：{args.workers} 个物理核", flush=True)
    with ThreadPoolExecutor(max_workers=args.workers) as pool:
        futures = [pool.submit(run, i) for i in range(args.workers)]
        parts = [future.result() for future in as_completed(futures)]
    parts.sort()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("w", encoding="utf-8", newline="") as out:
        header = None
        for part in parts:
            with part.open(encoding="utf-8", newline="") as source:
                first = source.readline()
                if header is None: header = first; out.write(first)
                elif first != header: raise RuntimeError(f"特征表头不一致：{part}")
                shutil.copyfileobj(source, out)
    with args.output.open(encoding="utf-8", newline="") as source:
        reader = csv.DictReader(source)
        rows = [row for row in reader if any(int(row[key]) >= value for key, value in RELAXED.items())]
        fields = reader.fieldnames
    with args.kept.open("w", encoding="utf-8", newline="") as out:
        writer = csv.DictWriter(out, fieldnames=fields); writer.writeheader(); writer.writerows(rows)
    print(f"all={len(lines)} kept_relaxed={len(rows)} thresholds={RELAXED}", flush=True)


if __name__ == "__main__":
    main()
