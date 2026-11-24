#!/usr/bin/env python3
"""从混杂的 Windowsup 留档中保守提取可审计的 PBB 候选。"""
import argparse, csv, os, re
from pathlib import Path

SKIP_SUFFIX={".exe",".dll",".zip",".jar",".pyd",".pak",".dat",".js",".json",".png",".jpg",".gif",".ico"}
SKIP_PARTS={"Chromium","namerena","lang","locales","node_modules",".git"}
LINE=re.compile(r"^(?P<candidate>[^\s@]+@[^\s@]+)(?P<tail>(?:\s+(?:-?\d+|![^\s]+))*)\s*$")

def read_text(path):
    raw=path.read_bytes()
    for encoding in ("utf-8-sig","gb18030"):
        try: return raw.decode(encoding),encoding
        except UnicodeDecodeError: pass
    return None,None

def main():
    p=argparse.ArgumentParser(description=__doc__); p.add_argument("--root",type=Path,required=True); p.add_argument("--output",type=Path,required=True); a=p.parse_args()
    hits={}; scanned=matched=0
    for root,dirs,files in os.walk(a.root):
        dirs[:]=[d for d in dirs if d not in SKIP_PARTS]
        for filename in files:
            path=Path(root)/filename
            if path.suffix.lower() in SKIP_SUFFIX or path.stat().st_size>80_000_000: continue
            text,encoding=read_text(path)
            if text is None: continue
            scanned+=1; relative=str(path.relative_to(a.root)); is_out="out" in path.parts
            for lineno,line in enumerate(text.splitlines(),1):
                match=LINE.fullmatch(line)
                if not match: continue
                tail=match.group("tail").split(); scores=[x for x in tail if x.lstrip("-").isdigit()]
                # 无分数的候选仅接受位于 out/ 的结果文件；避免把名字池或配置误收入。
                if len(scores)<2 and not (is_out and not tail): continue
                candidate=match.group("candidate")
                evidence="scored" if len(scores)>=2 else "out_name_only"
                matched+=1
                if candidate not in hits:
                    hits[candidate]={"candidate_id":candidate,"source_path":relative,"source_line":lineno,"encoding":encoding,"evidence":evidence,"legacy_scores":" ".join(scores),"duplicate_records":1}
                else: hits[candidate]["duplicate_records"]+=1
    a.output.parent.mkdir(parents=True,exist_ok=True)
    with a.output.open("w",newline="",encoding="utf-8") as f:
        fields=["candidate_id","source_path","source_line","encoding","evidence","legacy_scores","duplicate_records"]
        w=csv.DictWriter(f,fieldnames=fields,delimiter="\t"); w.writeheader(); w.writerows(sorted(hits.values(),key=lambda x:x["candidate_id"]))
    print("scanned_text_files=%d matched_records=%d unique_candidates=%d output=%s"%(scanned,matched,len(hits),a.output))
if __name__=="__main__": main()
