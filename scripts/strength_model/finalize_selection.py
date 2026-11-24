#!/usr/bin/env python3
"""从已完成的 single/duo/trio 排序文件无损汇总最终五席，不重复计算 TSWN。"""
import argparse, csv
from pathlib import Path

KEEP=("kind","group","predicted_win_rate","expected_rank_in_format","expected_percentile_in_format",
      "expected_website_rank","expected_website_percentile","tswn_pp","tswn_pd","tswn_qp","tswn_qd","tswn_sum")

def main():
    ap=argparse.ArgumentParser(); ap.add_argument("--dir",type=Path,required=True); a=ap.parse_args()
    picks=[]
    for kind, count in (("single",2),("duo",2),("trio",1)):
        path=a.dir/f"{kind}_ranked.csv"
        with path.open(encoding="utf-8",newline="") as f: rows=list(csv.DictReader(f))
        if len(rows)<count: raise SystemExit(f"{path} 候选不足")
        picks.extend({k:row.get(k,"") for k in KEEP} for row in rows[:count])
    with (a.dir/"provisional_finalists.csv").open("w",encoding="utf-8",newline="") as f:
        w=csv.DictWriter(f,fieldnames=KEEP);w.writeheader();w.writerows(picks)
    (a.dir/"README.txt").write_text(
        "五席是网站真实榜单监督模型的预筛结果。expected_website_rank 是相对训练所用公开网站上榜样本的预测名次，"
        "不是网站实时官方名次；最终需通过实际参赛或同规则全量对局确认。\n",encoding="utf-8")
    print(a.dir/"provisional_finalists.csv")
if __name__=="__main__": main()
