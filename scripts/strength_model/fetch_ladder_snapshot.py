#!/usr/bin/env python3
"""抓取公开天梯首页的全部活跃席位，作为网站排名监督训练的原始快照。

不登录、不提交队伍；保留页面 HTML 和逐席位 CSV，保证以后可审计解析结果。
"""
import argparse, csv, html, re
from datetime import datetime, timezone
from pathlib import Path
from urllib.request import Request, urlopen

URL = "https://shenjack.top:10007/"
FORMATS = ("1A", "1B", "2A", "2B", "3A")


def attr(fragment, name):
    m = re.search(r'\b' + re.escape(name) + r'\s*=\s*(["\'])(.*?)\1', fragment, re.S | re.I)
    return html.unescape(m.group(2).strip()) if m else ""


def parse(page):
    # 只解析 rankings-table 的 tbody；页面使用固定服务端 HTML，CSV 另有原页留档。
    table = re.search(r'<table\s+class="rankings-table".*?<tbody>(.*?)</tbody>', page, re.S | re.I)
    if not table:
        raise ValueError("未找到 rankings-table")
    rows = []
    for tr in re.findall(r'<tr[^>]*>(.*?)</tr>', table.group(1), re.S | re.I):
        cells = re.findall(r'<td\b[^>]*>(.*?)</td>', tr, re.S | re.I)
        if len(cells) < 2:
            continue
        rank_text = re.sub(r'<[^>]+>', '', cells[0]).strip()
        if not rank_text.isdigit():
            continue
        for fmt, cell in zip(FORMATS, cells[1:]):
            # 一个席位的 title 是实际参赛成员（单人或以 + 拼接的多人组）。
            team = re.search(r'<a\b([^>]*\bclass="[^"]*team-name[^"]*"[^>]*)>', cell, re.S | re.I)
            score = re.search(r'<a\b[^>]*\bclass="score"[^>]*>(.*?)</a>', cell, re.S | re.I)
            if not team or not score:
                continue
            group = attr(team.group(1), "title")
            score_text = re.sub(r'<[^>]+>', '', score.group(1)).strip()
            if not group or not re.fullmatch(r'-?\d+(?:\.\d+)?', score_text):
                continue
            rows.append({"format":fmt.lower(), "rank":int(rank_text), "group":group,
                         "website_score":float(score_text), "team":re.sub(r'<[^>]+>', '', team.group(0)).strip()})
    if not rows:
        raise ValueError("未解析到任何活跃席位")
    return rows


def main():
    ap = argparse.ArgumentParser(); ap.add_argument("--output-dir", type=Path, required=True)
    ap.add_argument("--url", default=URL); a = ap.parse_args()
    req = Request(a.url, headers={"User-Agent":"name-arena-model-research/1.0"})
    with urlopen(req, timeout=40) as r:
        page = r.read().decode("utf-8")
    captured = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H-%M-%SZ")
    a.output_dir.mkdir(parents=True, exist_ok=True)
    (a.output_dir / f"ladder_{captured}.html").write_text(page, encoding="utf-8")
    rows = parse(page)
    for row in rows:
        row["snapshot_id"] = captured
        row["source_url"] = a.url
    path = a.output_dir / f"ladder_{captured}.csv"
    with path.open("w", newline="", encoding="utf-8") as f:
        w = csv.DictWriter(f, fieldnames=["snapshot_id","source_url","format","rank","website_score","team","group"])
        w.writeheader(); w.writerows(rows)
    counts = {fmt:sum(r["format"] == fmt.lower() for r in rows) for fmt in FORMATS}
    print(f"snapshot={captured} rows={len(rows)} " + " ".join(f"{k}={v}" for k,v in counts.items()))
    print(path)


if __name__ == "__main__":
    main()
