#!/usr/bin/env python3
"""生成固定随机种子的少量候选，用于端到端冒烟测试。"""
import argparse
import random
from pathlib import Path

def main():
    p=argparse.ArgumentParser(); p.add_argument("--output",type=Path,required=True); p.add_argument("--count",type=int,default=36); p.add_argument("--seed",type=int,default=20261017); p.add_argument("--team",default="fAIgFUL"); a=p.parse_args()
    rng=random.Random(a.seed); alphabet="abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
    names=[]
    while len(names)<a.count:
        value="".join(rng.choice(alphabet) for _ in range(9))
        if value not in names: names.append(value)
    a.output.parent.mkdir(parents=True,exist_ok=True)
    a.output.write_text("\n".join(name+"@"+a.team for name in names)+"\n",encoding="utf-8")
if __name__=="__main__": main()
