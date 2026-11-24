#!/usr/bin/env python3
"""从网站榜单快照提取全部实际成员，并用 PBB 特征导出器逐一计算属性。"""
import argparse, csv, subprocess
from pathlib import Path

def main():
    ap=argparse.ArgumentParser(); ap.add_argument("--snapshots-dir",type=Path,required=True)
    ap.add_argument("--exporter",type=Path,required=True); ap.add_argument("--output",type=Path,required=True); a=ap.parse_args()
    members=set()
    for path in sorted(a.snapshots_dir.glob("ladder_*.csv")):
        with path.open(encoding="utf-8",newline="") as f:
            for row in csv.DictReader(f):
                members.update(x for x in row["group"].split("+") if "@" in x)
    if not members: raise SystemExit("快照中没有成员")
    a.output.parent.mkdir(parents=True,exist_ok=True)
    # 使用导出器的精确 TSV 输入模式，防止网站队名本身的空格被误截断。
    raw="\n".join(x.rsplit("@", 1)[0]+"\t"+x.rsplit("@", 1)[1] for x in sorted(members))+"\n"
    result=subprocess.run([str(a.exporter)],input=raw,text=True,capture_output=True,check=True)
    a.output.write_text(result.stdout,encoding="utf-8")
    print(f"members={len(members)} output={a.output}")

if __name__=="__main__": main()
