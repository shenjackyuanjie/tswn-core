#!/usr/bin/env python3
"""每技能四榜报告：每技能四族评分和（pbb_score_qp+qd+pp+pd）前 N 名满测 TSWN，
pp/pd/qp/qd 四个指标各取前 M 名，输出 35×4 张榜。

口径：
- 熟练度 = 该名字 16 个技能槽（pbb_skill_0..15 / pbb_freq_0..15）中该技能 id
  出现槽位的 freq 之和（多个槽位相加，如「隐匿 16+16」计 32）。
- 「该技能熟练度最高的名字」= 该技能是这个名字自身所有技能中熟练度最高
  的技能；最高熟练度并列多个技能时该名字同时计入每个并列技能的表。
- 每技能按 PBB 四族评分（qp/qd/pp/pd）各族各自降序取前 --top-select 名、
  四族去重后一次性 namer-pf 默认满测（n=10000，--keep-rq），再按
  pp/pd/qp/qd 各取前 --board-size 名成榜。
- 单榜（pp/pd/qp/qd）只列对应评分一列；另加「总榜」列出 qp/qd/pp/pd 四评分
  与四评分总和（tswn_sum），按四评分总和降序取前 --board-size 名。
- 「白板榜」：所有技能熟练度均 <30 的名字（含无技能记录者），同样按四族
  评分各族前 --top-select 名去重预筛，满测后按四评分总和降序取前
  --board-size 名成榜。
- 「队友评分前 10」：fz/bc/wc/fs/pj 五类（刺评/辅评/无刺评/配件评/分身评），
  每类先按池里 PBB 族分（pbb_score_fp/cp/wc/fs/pj）降序取前 --pair-top-select
  预筛，只对这些候选跑 openbox-cli pair（加权胜率 × 队友系数取 top head 加和，
  --pair-count 场粗略判断），再按 pair 分降序取前 10，无需满测。
- 单榜内对应评分列最高值、总榜/白板榜内各列最高值加粗；名字属于 --batch-names
  （本批新名字）的加下划线。加粗/下划线仅作用于 Markdown，CSV 为纯数值。
- 报告表格中「熟练度」列写为「{技能}熟练度」（如「火球熟练度」）；白板榜无
  熟练度列。
"""
import argparse
import csv
import json
import re
import subprocess
from collections import defaultdict
from pathlib import Path

SKILL_CN = ["火球", "冰冻", "雷击", "地裂", "吸血", "投毒", "连击", "会心", "瘟疫", "命轮", "狂暴", "魅惑", "加速", "减速", "诅咒",
            "治愈", "苏生", "净化", "铁壁", "蓄力", "聚气", "潜行", "血祭", "分身", "幻术", "防御", "守护", "反弹", "护符", "护盾",
            "反击", "吞噬", "召灵", "垂死", "隐匿"]
METRICS = ("pp", "pd", "qp", "qd", "sum")
BOARDS = ("pp", "pd", "qp", "qd")
BOLD_COLS = ("qp", "qd", "pp", "pd", "sum")
TM_COLS = ("fp", "cp", "wc", "fs", "pj")
# 队友评分五类：(ds4 缩写, PBB 族名, 中文名)
# 族名必须按 PBB 语义对齐（scoring_1124.hpp：背刺=cp、辅助=fp、无刺=wc、人海=fs、配件=pj）：
# 刺评←背刺(cp)、辅评←辅助(fp)，两者常被按字母序误配，勿改。
TEAMMATE_FAMS = (("fz", "cp", "刺评"), ("bc", "fp", "辅评"), ("wc", "wc", "无刺评"),
                 ("fs", "fs", "配件评"), ("pj", "pj", "分身评"))
TEAMMATE_DESC = {
    "刺评": "名字配「刺评」类预设队友的强度分",
    "辅评": "名字配「辅评」类预设队友的强度分",
    "无刺评": "名字配「无刺评」类预设队友的强度分",
    "配件评": "名字配「配件评」类预设队友的强度分（引擎别名 rh=人海）",
    "分身评": "名字配「分身评」类预设队友的强度分",
}


def collect_name_proficiency(pool_path):
    """流式读池：每名字的 {技能: 熟练度} 与 PBB 四族评分加和；名字含 + 的跳过。"""
    names = {}
    skipped_plus = 0
    with pool_path.open(encoding="utf-8", newline="") as f:
        for row in csv.DictReader(f):
            cid = row["candidate_id"]
            if "+" in cid:
                skipped_plus += 1
                continue  # 名字含 + 会被 namer-pf 当作组分隔，与现有管线同样跳过
            try:
                scores = {c: float(row["pbb_score_" + c]) for c in BOARDS + TM_COLS}
            except (KeyError, ValueError):
                continue
            prof = defaultdict(float)
            for i in range(16):
                sk = row.get(f"pbb_skill_{i}", "").strip()
                if not sk:
                    continue
                try:
                    sk = int(sk)
                    freq = float(row[f"pbb_freq_{i}"])
                except ValueError:
                    continue
                if 0 <= sk < 35:
                    prof[sk] += freq
            names[cid] = (dict(prof), scores)
    if skipped_plus:
        print(f"跳过名字含 + 的候选 {skipped_plus} 个（namer-pf 组分隔限制）")
    return names, skipped_plus


def build_skill_lists(names):
    """每个技能收集「该技能为本人最高熟练度技能之一」的名字；并列最高者同时计入
    每个并列技能的表。"""
    per_skill = defaultdict(list)
    tied = 0
    for cid, (prof, scores) in names.items():
        if not prof:
            continue
        top = max(prof.values())
        top_skills = [s for s, v in prof.items() if v == top]
        if len(top_skills) > 1:
            tied += 1
        for s in top_skills:
            per_skill[s].append((top, scores, cid))
    return per_skill, tied


def pick_top(entries, n, family):
    """按 PBB 该族评分降序取前 n 名（并列按名字字典序保证确定性）。"""
    return sorted(entries, key=lambda e: (-e[1][family], e[2]))[:n]


def load_score_cache(path):
    """读评分缓存 TSV（name\tpp\tpd\tqp\tqd\tsum），返回 {名字: {tswn_pp..tswn_sum}}。"""
    cache = {}
    if not path or not path.exists():
        return cache
    with path.open(encoding="utf-8", newline="") as f:
        for row in csv.DictReader(f, delimiter="\t"):
            cid = row["name"]
            cache[cid] = {"tswn_" + m: float(row[m]) for m in METRICS}
    return cache


def save_score_cache(path, cache):
    """合并写回缓存（保留历史名字，按名字排序，TSV 格式）。"""
    if not path:
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="") as f:
        f.write("name\tpp\tpd\tqp\tqd\tsum\n")
        for cid in sorted(cache):
            f.write(cid + "\t" + "\t".join(fmt_num(cache[cid]["tswn_" + m]) for m in METRICS) + "\n")


def batch_namer_pf(cli, groups, threads, workdir, cache):
    """批量 namer-pf 默认满测（n=10000 --keep-rq），只测缓存中没有的名字；
    新测值写入 cache，返回 {组串: {tswn_pp..}}（缓存值 + 新测值合并）。"""
    groups = sorted(set(groups))
    missing = [g for g in groups if g not in cache]
    if missing:
        path = Path(workdir) / "skill_top10_batch_input.txt"
        path.write_text("\n".join(missing) + "\n", encoding="utf-8")
        out = subprocess.check_output(
            [str(cli), "namer-pf", "-f", str(path), "--keep-rq", "-t", str(threads)],
            universal_newlines=True, stderr=subprocess.STDOUT)
        for line in out.splitlines():
            m = re.match(r"^(.*) (pp|pd|qp|qd|sum):(\d+(?:\.\d+)?)$", line)
            if not m:
                raise RuntimeError("namer-pf 输出无法解析：" + line)
            cache.setdefault(m.group(1), {})["tswn_" + m.group(2)] = float(m.group(3))
    feats = {}
    for g in groups:
        if len(cache.get(g, {})) != len(METRICS):
            raise RuntimeError("namer-pf 缺少组合：" + g)
        feats[g] = cache[g]
    return feats


def batch_pair_score(cli, cwd, candidates_by_abbr, count, threads, workdir):
    """对每类队友候选跑 openbox-cli pair（GUI 对应预设），返回 {abbr: {名字: 分数}}。

    候选 = 每类 PBB 族分降序前 --pair-top-select 的名字；pair 用 --pair-count 场
    粗略判断（默认 1000，GUI 10% 档）。输出用 --log JSONL（label/score 字段），
    避免名字含空格/特殊字符时按空格切列出错。解析失败的名字按 0 分垫底。
    增量缓存：workdir 下已有 skill_top10_pair_{abbr}.jsonl 时按名字复用历史分数，
    只对缺失名字补算（pair 每名独立对战，分数与候选集合无关，增量安全），
    补算后与历史行合并全量重写缓存。
    """
    pair_scores = {abbr: {} for abbr, _, _ in TEAMMATE_FAMS}
    for abbr, fam, cn in TEAMMATE_FAMS:
        cands = sorted(candidates_by_abbr[abbr])
        if not cands:
            continue
        # 输入/输出必须是绝对路径：subprocess cwd 已切到 settings.toml 所在目录，
        # 相对路径会在那里解析而找不到文件（openbox-cli exit 2）。
        input_path = (Path(workdir) / f"skill_top10_pair_input_{abbr}.txt").resolve()
        out_path = (Path(workdir) / f"skill_top10_pair_{abbr}.jsonl").resolve()
        # 历史缓存行（label → 原始 JSON 行 dict），损坏行跳过
        cached = {}
        if out_path.exists():
            for line in out_path.read_text(encoding="utf-8").splitlines():
                try:
                    row = json.loads(line)
                except ValueError:
                    continue
                if row.get("label"):
                    cached[row["label"]] = row
        missing = [cid for cid in cands if cid not in cached]
        if missing:
            print(f"{cn}（{abbr}）pair 缓存命中 {len(cands) - len(missing)}/"
                  f"{len(cands)}，补算 {len(missing)} 个…")
            input_path.write_text("\n".join(missing) + "\n", encoding="utf-8")
            subprocess.run(
                [str(cli), "pair", "-p", str(input_path), "--teammate-preset", cn,
                 "-n", str(count), "--detail", "none", "--log",
                 "-o", str(out_path), "-t", str(threads)],
                cwd=str(cwd), check=True)
            for line in out_path.read_text(encoding="utf-8").splitlines():
                try:
                    row = json.loads(line)
                except ValueError:
                    continue
                if row.get("label"):
                    cached[row["label"]] = row
            # 历史 + 新测合并后全量重写缓存（保持 label/score 等原始字段）
            with out_path.open("w", encoding="utf-8") as fo:
                for row in cached.values():
                    fo.write(json.dumps(row, ensure_ascii=False) + "\n")
        pair_scores[abbr] = {label: float(row.get("score", 0.0))
                             for label, row in cached.items() if label in cands}
        missing = [cid for cid in cands if cid not in pair_scores[abbr]]
        if missing:
            print(f"{cn}（{abbr}）pair 解析缺失 {len(missing)} 个名字，按 0 分垫底")
            for cid in missing:
                pair_scores[abbr][cid] = 0.0
    return pair_scores


def fmt_num(v):
    return f"{v:g}"


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--pool", type=Path, required=True, help="总名字特征 CSV")
    ap.add_argument("--cli", type=Path, required=True, help="tswn-cli 路径")
    ap.add_argument("--output-dir", type=Path, required=True,
                    help="主输出目录（skill_top10.csv / skill_top10_report.md，覆盖写）")
    ap.add_argument("--archive-dir", type=Path, default=None,
                    help="可选：日期归档目录（带日期的文件名副本）")
    ap.add_argument("--batch-names", type=Path, required=True,
                    help="本批新名字清单（每行 name@team），命中者下划线")
    ap.add_argument("--score-cache", type=Path, default=None,
                    help="已测名字评分缓存 TSV（缺省 data/skill_reports/tswn_scores_cache.tsv）")
    ap.add_argument("--batch-label", default="本批",
                    help="报告头中本批新名字的标签（如 2026-10-24-hx2）")
    ap.add_argument("--threads", type=int, default=40)
    ap.add_argument("--top-select", type=int, default=100)
    ap.add_argument("--board-size", type=int, default=10)
    ap.add_argument("--pair-cli", type=Path, default=None,
                    help="openbox-cli 路径（队友评分 pair 口径用）")
    ap.add_argument("--pair-cwd", type=Path, default=None,
                    help="pair 运行目录（settings.toml 所在处，如 ds4_workspace/setting）")
    ap.add_argument("--pair-top-select", type=int, default=20,
                    help="每类队友榜按 PBB 族分预筛进入 pair 计算的候选数（默认 20）")
    ap.add_argument("--pair-count", type=int, default=1000,
                    help="pair 每组合运行场数（默认 1000，GUI 10% 档，粗略判断）")
    ap.add_argument("--pair-threads", type=int, default=40,
                    help="pair benchmark 线程数")
    a = ap.parse_args()
    a.output_dir.mkdir(parents=True, exist_ok=True)
    # 中间产物（批输入、pair 输入/输出、csv）写入 cache 子目录，
    # reports 根目录只留最终汇报文件（skill_top10_report.md）。
    cache_dir = a.output_dir / "cache"
    cache_dir.mkdir(parents=True, exist_ok=True)
    if a.pair_cli:
        a.pair_cli = a.pair_cli.resolve()
    if a.pair_cwd:
        a.pair_cwd = a.pair_cwd.resolve()

    batch = set()
    with a.batch_names.open(encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if line:
                batch.add(line)

    names, skipped_plus = collect_name_proficiency(a.pool)
    per_skill, tied = build_skill_lists(names)
    # 白板：所有技能熟练度均 <30（含无技能记录者）
    blank_entries = [(0.0, scores, cid) for cid, (prof, scores) in names.items()
                     if all(v < 30 for v in prof.values())]
    print(f"最高熟练度并列、同时计入所有并列技能的名字 {tied} 个")
    # 预筛：四族评分各族各自取前 --top-select 名、四族去重（该族零分者自然垫底）
    selected = {}
    for sk in range(35):
        picked = {}
        for fam in BOARDS:
            for e in pick_top(per_skill.get(sk, []), a.top_select, fam):
                picked.setdefault(e[2], e)
        selected[sk] = [picked[cid] for cid in sorted(picked)]
    blank_picked = {}
    for fam in BOARDS:
        for e in pick_top(blank_entries, a.top_select, fam):
            blank_picked.setdefault(e[2], e)
    blank_pick = [blank_picked[cid] for cid in sorted(blank_picked)]
    union = sorted({cid for rows in selected.values() for (_, _, cid) in rows}
                   | {cid for (_, _, cid) in blank_pick})
    # 队友评分前 10：每类按 PBB 族分降序取前 --pair-top-select 预筛，只对这些候选
    # 跑 pair（加权胜率 × 队友系数取 top head 加和 ≈200 口径），再按 pair 分取前 10。
    pair_candidates = {}
    for abbr, fam, cn in TEAMMATE_FAMS:
        rows = sorted(((scores, cid) for cid, (prof, scores) in names.items()
                       if scores[fam] > 0), key=lambda e: (-e[0][fam], e[1]))
        pair_candidates[abbr] = [cid for (scores, cid) in rows[:a.pair_top_select]]
    if a.pair_cli and a.pair_cwd:
        print(f"队友评分 pair 计算：每类前 {a.pair_top_select} 候选（n={a.pair_count}，"
              f"-t {a.pair_threads}）…")
        pair_scores = batch_pair_score(a.pair_cli, a.pair_cwd, pair_candidates,
                                       a.pair_count, a.pair_threads, cache_dir)
    else:
        pair_scores = None
    tm_boards = {}
    for abbr, fam, cn in TEAMMATE_FAMS:
        cands = pair_candidates[abbr]
        if pair_scores is not None:
            scored = [(pair_scores[abbr].get(cid, 0.0), cid) for cid in cands]
            rows = sorted(scored, key=lambda e: (-e[0], e[1]))[:10]
            tm_boards[abbr] = [(names[cid][1], cid, score) for score, cid in rows]
        else:
            rows = sorted(((scores, cid) for cid, (prof, scores) in names.items()
                           if scores[fam] > 0), key=lambda e: (-e[0][fam], e[1]))
            tm_boards[abbr] = [(scores, cid, scores[fam]) for scores, cid in rows[:10]]
    cache_path = a.score_cache or (
        Path(__file__).resolve().parents[2] / "data" / "skill_reports" / "tswn_scores_cache.tsv")
    cache = load_score_cache(cache_path)
    n_cached = sum(1 for cid in union if cid in cache)
    print(f"入选名字去重后 {len(union)} 个，缓存已有 {n_cached} 个，"
          f"需新测 {len(union) - n_cached} 个，批量 namer-pf 满测中（n=10000, -t {a.threads}）…")
    feats = batch_namer_pf(a.cli, union, a.threads, cache_dir, cache)

    # 每技能每指标成榜：按该 TSWN 指标降序（并列按四族和、名字字典序）。
    boards = {}
    for sk in range(35):
        rows = selected[sk]
        for board in BOARDS:
            ranked = sorted(rows, key=lambda e: (-feats[e[2]]["tswn_" + board],
                                                 -sum(e[1][b] for b in BOARDS), e[2]))
            boards[(sk, board)] = ranked[:a.board_size]
        # 总榜：四评分总和降序
        ranked = sorted(rows, key=lambda e: (-feats[e[2]]["tswn_sum"],
                                             -sum(e[1][b] for b in BOARDS), e[2]))
        boards[(sk, "all")] = ranked[:a.board_size]
    # 白板榜：跨技能，按四评分总和降序
    blank_ranked = sorted(blank_pick, key=lambda e: (-feats[e[2]]["tswn_sum"],
                                                     -sum(e[1][b] for b in BOARDS), e[2]))
    blank_board = blank_ranked[:a.board_size]

    # 缓存裁剪：只保留本次入选名单（每技能各族前 100 ∪ 白板各族前 100）与最终
    # 上榜名字（单榜/总榜/白板榜前 10）的并集分数。被新名字挤出前 100 且未上榜的
    # 名字，之后任何一天都无法再进入榜单，其分数没有保留价值。
    keep = set(union)
    for sk in range(35):
        for board in list(BOARDS) + ["all"]:
            keep.update(e[2] for e in boards[(sk, board)])
    keep.update(e[2] for e in blank_board)
    cache = {cid: v for cid, v in cache.items() if cid in keep}
    save_score_cache(cache_path, cache)
    print(f"缓存裁剪后保留 {len(cache)} 个名字的分数")

    csv_path = cache_dir / "skill_top10.csv"
    md_path = a.output_dir / "skill_top10_report.md"
    header = ["技能", "技能id", "榜", "名次", "名字", "队伍", "技能熟练度",
              "qp", "qd", "pp", "pd", "四评分总和"]
    with csv_path.open("w", newline="", encoding="utf-8") as fcsv, \
         md_path.open("w", encoding="utf-8") as fmd:
        w = csv.writer(fcsv)
        w.writerow(header)
        fmd.write("# 每技能报告：四族评分各前 100 去重满测，pp/pd/qp/qd 单榜各取前 10 + 四评分总榜前 10\n\n")
        import datetime
        n_pool = len(names) + skipped_plus
        fmd.write(f"生成时间：{datetime.datetime.now():%Y-%m-%d %H:%M}（总名字池 {n_pool:,} 行，"
                  f"本批 {len(batch):,} 条：{a.batch_label}）\n\n")
        fmd.write("口径：熟练度=16 技能槽中该技能 freq 之和；入选者=该技能为本人最高熟练度技能的名字，")
        fmd.write("最高熟练度并列多个技能时该名字同时计入每个并列技能的表。\n")
        fmd.write("每技能按 PBB 四族评分（pbb_score_qp/qd/pp/pd）各族各自降序取前 100 名、四族去重后满测，")
        fmd.write("再按 pp/pd/qp/qd 各自降序取前 10 成单榜；总榜按四评分总和（tswn_sum）降序取前 10。\n")
        fmd.write("pp/pd/qp/qd/sum 为 `tswn-cli namer-pf` 默认满测（n=10000，--keep-rq）的 TSWN 原始万分制。\n")
        fmd.write("单榜只列对应评分；总榜列出 qp/qd/pp/pd 四评分与四评分总和；")
        fmd.write("白板榜=所有技能熟练度均 <30 的名字（同样四族各前 100 去重预筛），按四评分总和降序取前 10。")
        fmd.write("单榜对应评分列最高值、总榜各列最高值**加粗**，"
                  f"{a.batch_label}新进榜名字加下划线。\n\n")
        for sk in range(35):
            fmd.write(f"## {SKILL_CN[sk]}（id={sk}）\n\n")
            for board in BOARDS:
                rows = boards[(sk, board)]
                fmd.write(f"### {board} 榜\n\n")
                if not rows:
                    fmd.write("（池中没有以该技能为最高熟练度的名字）\n\n")
                    continue
                fmd.write(f"| 名次 | 名字 | 队伍 | {SKILL_CN[sk]}熟练度 | {board} |\n")
                fmd.write("| ---: | --- | --- | ---: | ---: |\n")
                col_max = max(feats[e[2]]["tswn_" + board] for e in rows)
                for rank, (freq, model_sum, cid) in enumerate(rows, 1):
                    name, _, team = cid.partition("@")
                    v = feats[cid]["tswn_" + board]
                    md_name = f"<u>{name}</u>" if cid in batch else name
                    cell = f"**{fmt_num(v)}**" if v == col_max else fmt_num(v)
                    fmd.write(f"| {rank} | {md_name} | {team} | {freq:g} | {cell} |\n")
                    csv_vals = {"qp": "", "qd": "", "pp": "", "pd": "", "sum": ""}
                    csv_vals[board] = fmt_num(v)
                    w.writerow([SKILL_CN[sk], sk, board, rank, name, team, f"{freq:g}",
                                csv_vals["qp"], csv_vals["qd"], csv_vals["pp"],
                                csv_vals["pd"], csv_vals["sum"]])
                fmd.write("\n")
            # 总榜：qp/qd/pp/pd 四评分与四评分总和
            rows = boards[(sk, "all")]
            fmd.write("### 总榜（qp/qd/pp/pd 四评分与总和）\n\n")
            if not rows:
                fmd.write("（池中没有以该技能为最高熟练度的名字）\n\n")
                continue
            fmd.write(f"| 名次 | 名字 | 队伍 | {SKILL_CN[sk]}熟练度 | qp | qd | pp | pd | 四评分总和 |\n")
            fmd.write("| ---: | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |\n")
            col_max = {c: max(feats[e[2]]["tswn_" + c] for e in rows) for c in BOLD_COLS}
            for rank, (freq, model_sum, cid) in enumerate(rows, 1):
                name, _, team = cid.partition("@")
                f = feats[cid]
                md_name = f"<u>{name}</u>" if cid in batch else name
                cells = {c: (f"**{fmt_num(f['tswn_' + c])}**" if f["tswn_" + c] == col_max[c]
                             else fmt_num(f["tswn_" + c])) for c in BOLD_COLS}
                fmd.write(f"| {rank} | {md_name} | {team} | {freq:g} | "
                          f"{cells['qp']} | {cells['qd']} | {cells['pp']} | {cells['pd']} | {cells['sum']} |\n")
                w.writerow([SKILL_CN[sk], sk, "all", rank, name, team, f"{freq:g}",
                            f["tswn_qp"], f["tswn_qd"], f["tswn_pp"], f["tswn_pd"], f["tswn_sum"]])
            fmd.write("\n")

        # 白板榜：所有技能熟练度均 <30 的名字（跨技能，报告末尾统一列出）
        fmd.write("## 白板榜（所有技能熟练度均 <30）\n\n")
        if not blank_board:
            fmd.write("（池中没有白板名字）\n\n")
        else:
            fmd.write("| 名次 | 名字 | 队伍 | qp | qd | pp | pd | 四评分总和 |\n")
            fmd.write("| ---: | --- | --- | ---: | ---: | ---: | ---: | ---: |\n")
            col_max = {c: max(feats[e[2]]["tswn_" + c] for e in blank_board) for c in BOLD_COLS}
            for rank, (freq, model_sum, cid) in enumerate(blank_board, 1):
                name, _, team = cid.partition("@")
                f = feats[cid]
                md_name = f"<u>{name}</u>" if cid in batch else name
                cells = {c: (f"**{fmt_num(f['tswn_' + c])}**" if f["tswn_" + c] == col_max[c]
                             else fmt_num(f["tswn_" + c])) for c in BOLD_COLS}
                fmd.write(f"| {rank} | {md_name} | {team} | "
                          f"{cells['qp']} | {cells['qd']} | {cells['pp']} | {cells['pd']} | {cells['sum']} |\n")
                w.writerow(["白板", -1, "blank", rank, name, team, "",
                            f["tswn_qp"], f["tswn_qd"], f["tswn_pp"], f["tswn_pd"], f["tswn_sum"]])
            fmd.write("\n")
    
        # 队友评分前 10：缩写 fz/bc/wc/fs/pj，各配中文名与含义
        fmd.write("## 队友评分前 10（fz/bc/wc/fs/pj）\n\n")
        fmd.write("对一个名字搭配某类预设队友的强度分：与预设队友逐个组成二人组（各自单独构建、")
        fmd.write("同公会加成不计），取 cqp 最高的前 head 个求和。本榜分数来自 `openbox-cli pair`")
        fmd.write("（加权胜率 × 队友系数），每类先按 PBB 族分降序取前 "
                  f"{a.pair_top_select} 预筛，只对这些候选跑 pair"
                  f"（每组合 {a.pair_count} 场粗略判断），再按 pair 分降序取前 10。\n\n")
        for abbr, fam, cn in TEAMMATE_FAMS:
            rows = tm_boards[abbr]
            fmd.write(f"### {cn} {abbr}（PBB 族 {fam}）\n\n")
            fmd.write(f"含义：{TEAMMATE_DESC[cn]}。\n\n")
            if not rows:
                fmd.write("（无）\n\n")
                continue
            fmd.write("| 名次 | 名字 | 队伍 | 分数 |\n")
            fmd.write("| ---: | --- | --- | ---: |\n")
            col_max = max(e[2] for e in rows)
            for rank, (scores, cid, v) in enumerate(rows, 1):
                name, _, team = cid.partition("@")
                md_name = f"<u>{name}</u>" if cid in batch else name
                cell = f"**{fmt_num(v)}**" if v == col_max else fmt_num(v)
                fmd.write(f"| {rank} | {md_name} | {team} | {cell} |\n")
                w.writerow([f"队友-{cn}", -2, abbr, rank, name, team, "",
                            fmt_num(v), "", "", "", ""])
            fmd.write("\n")
    print("已写入", csv_path)
    print("已写入", md_path)
    if a.archive_dir:
        a.archive_dir.mkdir(parents=True, exist_ok=True)
        import shutil
        # rdxfs 不支持 xattr，copy2 的 copystat 会 PermissionError；copyfile 只拷内容。
        shutil.copyfile(csv_path, a.archive_dir / "skill_top10_2026-10-23.csv")
        shutil.copyfile(md_path, a.archive_dir / "skill_top10_report_2026-10-23.md")
        print("已归档", a.archive_dir)


if __name__ == "__main__":
    main()
