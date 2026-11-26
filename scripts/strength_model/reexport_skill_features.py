#!/usr/bin/env python3
"""用修复后的 pbb_feature_export 重导出技能列（原导出器单数 score_full 漏填
result.skills/freqs，导致 16 槽技能列全零；修复后与 tswn-cli to-diy 逐字一致）。

对每个输入 features.csv：提取 name/team 列生成 TSV，喂给导出器重算全部列，
校验「行数、candidate_id 顺序、非技能列逐位一致」后写入新文件（旧文件备份到
output-dir 下 backup/）。windowsup 输入同样处理，输出单独一个同名格式文件。
"""
import argparse
import csv
import shutil
import subprocess
import sys
from pathlib import Path

SKILL_COLS = ["pbb_skill_%d" % i for i in range(16)] + ["pbb_freq_%d" % i for i in range(16)]
NON_SKILL = None  # 由首行字段计算


def read_rows(path):
    with path.open(encoding="utf-8", newline="") as f:
        reader = csv.DictReader(f)
        return reader.fieldnames, list(reader)


def reexport(exporter, rows, workdir, tmp_prefix):
    """TSV（名字<TAB>队伍）喂给导出器，解析回 dict 行。"""
    tmp = Path(workdir) / (tmp_prefix + "_names.tsv")
    with tmp.open("w", encoding="utf-8", newline="") as f:
        for r in rows:
            f.write("%s\t%s\n" % (r["name"], r["team"]))
    out = subprocess.check_output([str(exporter)], stdin=tmp.open("rb")).decode("utf-8")
    parsed = list(csv.DictReader(out.splitlines()))
    if len(parsed) != len(rows):
        raise SystemExit("%s: 行数不符 %d != %d" % (tmp_prefix, len(parsed), len(rows)))
    for old, new in zip(rows, parsed):
        if new["candidate_id"] != old["candidate_id"]:
            raise SystemExit("%s: candidate_id 顺序不符" % tmp_prefix)
    return parsed


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--exporter", type=Path, required=True)
    ap.add_argument("--inputs", type=Path, nargs="+", required=True,
                    help="旧 features.csv 列表（技能列全零的版本）")
    ap.add_argument("--output-dir", type=Path, required=True,
                    help="新文件输出目录；旧文件备份到该目录下 backup/")
    ap.add_argument("--windowsup-input", type=Path, default=None,
                    help="windowsup 旧特征 CSV（单开一个文件输出）")
    ap.add_argument("--windowsup-output", type=Path, default=None,
                    help="windowsup 新文件名（写入 output-dir）")
    a = ap.parse_args()
    a.output_dir.mkdir(parents=True, exist_ok=True)
    backup = a.output_dir / "backup"
    backup.mkdir(exist_ok=True)
    workdir = a.output_dir / "reexport_tmp"
    workdir.mkdir(exist_ok=True)

    def process(path, out_name):
        fields, rows = read_rows(path)
        if "name" not in fields or "team" not in fields:
            raise SystemExit("%s: 缺少 name/team 列" % path)
        non_skill = [c for c in fields if c not in SKILL_COLS]
        new = reexport(a.exporter, rows, workdir, path.stem)
        n_skill_rows = 0
        for old, r in zip(rows, new):
            for c in non_skill:
                if old.get(c) != r.get(c):
                    raise SystemExit("%s: 非技能列 %s 重算后不一致" % (path, c))
            if any(r.get(c) not in ("", "0") for c in SKILL_COLS):
                n_skill_rows += 1
        # 旧文件带技能错误，备份后覆盖；备份只保留一次。
        # rdxfs 不支持 xattr，copy2 的 copystat 会 PermissionError，用 copyfile 只复制内容。
        if path.parent == a.output_dir and path.exists():
            shutil.copyfile(str(path), str(backup / path.name))
        target = a.output_dir / out_name
        with target.open("w", encoding="utf-8", newline="") as f:
            w = csv.DictWriter(f, fieldnames=fields)
            w.writeheader()
            w.writerows(new)
        print("%s -> %s 行=%d 技能非零行=%d" % (path, target, len(new), n_skill_rows))

    for p in a.inputs:
        process(p, p.name)
    if a.windowsup_input and a.windowsup_output:
        process(a.windowsup_input, a.windowsup_output.name)
    print("完成；旧文件备份在 %s" % backup)


if __name__ == "__main__":
    sys.exit(main())
