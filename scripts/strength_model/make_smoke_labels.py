#!/usr/bin/env python3
"""以真实 tswn-cli 1v1 批量对局生成极小冒烟标签集。"""
import argparse, csv, re, subprocess
from pathlib import Path

PATTERN=re.compile(r"胜率:\s*[\d.]+%\s*\((\d+)/(\d+)\)")
def wins(binary, left, right, games):
    out=subprocess.check_output([str(binary),"bench","win-rate","-r",left+"\n"+right,"-n",str(games),"--single-thread","--keep-rq"],text=True,stderr=subprocess.STDOUT)
    m=PATTERN.search(out)
    if not m: raise RuntimeError("无法解析 tswn-cli 输出:\n"+out)
    return int(m.group(1)),int(m.group(2))
def main():
    p=argparse.ArgumentParser(description=__doc__); p.add_argument("--features",type=Path,required=True,help="仅由 PBB 命中结果导出的特征 CSV"); p.add_argument("--cli",type=Path,required=True); p.add_argument("--output",type=Path,required=True); p.add_argument("--opponents",type=int,default=4); p.add_argument("--games",type=int,default=8); a=p.parse_args()
    with a.features.open(newline="",encoding="utf-8") as f: features={row["candidate_id"]:row for row in csv.DictReader(f) if row["pbb_flag"]=="1"}
    names=list(features)
    if len(names) <= a.opponents: raise RuntimeError("PBB 命中候选不足，至少需要 opponent 数量加 1")
    pool=names[:a.opponents]; candidates=names[a.opponents:]
    fields=list(next(iter(features.values())).keys())+["target_win_rate"]
    a.output.parent.mkdir(parents=True,exist_ok=True)
    with a.output.open("w",newline="",encoding="utf-8") as f:
        writer=csv.DictWriter(f,fieldnames=fields); writer.writeheader()
        for idx,candidate in enumerate(candidates,1):
            total=0.0
            for opponent in pool:
                forward,n=wins(a.cli,candidate,opponent,a.games)
                reverse,n2=wins(a.cli,opponent,candidate,a.games)
                if n!=a.games or n2!=a.games: raise RuntimeError("对局数不符")
                total+=(forward+(n2-reverse))/(n+n2)
            row=dict(features[candidate]); row["target_win_rate"]=f"{total/len(pool):.12g}"; writer.writerow(row)
            print(f"[{idx}/{len(candidates)}] {candidate}: {row['target_win_rate']}")
if __name__=="__main__": main()
