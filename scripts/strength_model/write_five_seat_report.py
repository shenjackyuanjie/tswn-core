#!/usr/bin/env python3
"""将五席候选写成便于人工分析的 TSWN 技能与评分报告。"""
import argparse
import csv
import json
import re
import subprocess
from datetime import datetime
from pathlib import Path


METRIC_LABELS = (("qp", "强评"), ("qd", "强单"), ("pp", "普评"), ("pd", "普单"))
METRIC_RE = re.compile(r"\b(pp|pd|qp|qd|sum):([0-9]+(?:\.[0-9]+)?)")
SKILL_KEYS = {
    "sklfire": "火球", "sklice": "冰冻", "sklthunder": "雷击", "sklquake": "地裂", "sklabsorb": "吸血",
    "sklpoison": "投毒", "sklrapid": "连击", "sklcritical": "会心", "sklhalf": "瘟疫", "sklexchange": "命轮",
    "sklberserk": "狂暴", "sklcharm": "魅惑", "sklhaste": "加速", "sklslow": "减速", "sklcurse": "诅咒",
    "sklheal": "治愈", "sklrevive": "苏生", "skldisperse": "净化", "skliron": "铁壁", "sklcharge": "蓄力",
    "sklaccumulate": "聚气", "sklassassinate": "潜行", "sklsummon": "血祭", "sklclone": "分身", "sklshadow": "幻术",
    "skldefend": "防御", "sklprotect": "守护", "sklreflect": "反弹", "sklreraise": "护符", "sklshield": "护盾",
    "sklcounter": "反击", "sklmerge": "吞噬", "sklzombie": "召灵", "sklupgrade": "垂死", "sklhide": "隐匿",
}


def invoke(cli: Path, *args: str) -> str:
    result = subprocess.run([str(cli), *args], text=True, capture_output=True, check=False, timeout=120)
    if result.returncode:
        raise RuntimeError(f"tswn-cli {' '.join(args[:2])} 失败：{result.stderr.strip() or result.stdout.strip()}")
    return result.stdout


def score(cli: Path, group: str, rounds: int) -> dict[str, str]:
    output = invoke(cli, "namer-pf", "-r", group, "-n", str(rounds), "--keep-rq")
    result = dict(METRIC_RE.findall(output))
    absent = {key for key, _ in METRIC_LABELS} - set(result)
    if absent:
        raise RuntimeError(f"未能解析评分 {group}：缺少 {sorted(absent)}\n{output}")
    return result


def skills_by_member(cli: Path, group: str) -> list[list[str]]:
    parsed = []
    # 当前命令行版 to-diy 对含 + 的输入只输出 OL 串而不附详情，因此逐成员导出；
    # 这正是成员自身的技能熟练度，组队影响另由上方的 namer-pf 分数体现。
    for member in group.split("+"):
        output = invoke(cli, "to-diy", "-r", member)
        match = re.search(r"\+ol:(\{[^\r\n]+\})", output)
        if not match:
            raise RuntimeError(f"未能解析成员技能：{member}\n{output}")
        try:
            skill_values = json.loads(match.group(1)).get("skills", {})
        except json.JSONDecodeError as error:
            raise RuntimeError(f"技能 JSON 格式错误：{member}: {error}") from error
        skills = [f"{SKILL_KEYS.get(key, key)} {str(value).replace('*', '×')}" for key, value in skill_values.items()]
        parsed.append(skills or ["无主动技能"])
    return parsed


def fmt(value: str | None) -> str:
    if value is None or value == "":
        return "—"
    try:
        return f"{float(value):.4f}"
    except ValueError:
        return value


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--finalists", type=Path, required=True)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--rounds", type=int, default=10000, help="每项 TSWN 评分运行场数（10000 为完整默认口径）")
    args = parser.parse_args()
    with args.finalists.open(encoding="utf-8", newline="") as handle:
        rows = list(csv.DictReader(handle))
    expected = ["single", "single", "duo", "duo", "trio"]
    if [row.get("kind") for row in rows] != expected:
        raise SystemExit("五席名单格式错误：应依次为 2 single、2 duo、1 trio")

    out = [
        "# 每日五席：技能熟练度与 TSWN 评分",
        "",
        f"生成时间：{datetime.now().astimezone().strftime('%F %T %Z')}",
        f"候选来源：`{args.finalists}`", "",
        "评分口径：本机 `tswn-cli namer-pf -n %d --keep-rq`；四项保持 TSWN 原始万分制。强评=qp、强单=qd、普评=pp、普单=pd。" % args.rounds,
        "技能口径：本机 `tswn-cli to-diy` 导出的成员自身技能熟练度。多人协同（包括可能的组队加成）以同表的组级四项评分体现。", "",
    ]
    titles = {"single": "单人席", "duo": "双人席", "trio": "三人席"}
    ordinal = {"single": 0, "duo": 0, "trio": 0}
    for row in rows:
        kind, group = row["kind"], row["group"]
        ordinal[kind] += 1
        scores = score(args.cli, group, args.rounds)
        member_skills = skills_by_member(args.cli, group)
        out.extend((f"## {titles[kind]} {ordinal[kind]}", "", f"组合：`{group}`", ""))
        out.extend(("| 强评 | 强单 | 普评 | 普单 | 总分 |", "| ---: | ---: | ---: | ---: | ---: |",
                    f"| {scores['qp']} | {scores['qd']} | {scores['pp']} | {scores['pd']} | {scores.get('sum', '—')} |", ""))
        out.append("成员技能熟练度：")
        for member, skills in zip(group.split("+"), member_skills):
            out.append(f"- `{member}`：{'；'.join(skills)}")
        out.extend(("", f"模型预测胜率：{fmt(row.get('predicted_win_rate'))}；预期网站单榜名次（100 队换算）：{row.get('expected_website_rank', '—')}；来源日期：{row.get('source_days', '—')}", ""))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text("\n".join(out) + "\n", encoding="utf-8")
    print(f"已写入 {args.output}")


if __name__ == "__main__":
    main()
