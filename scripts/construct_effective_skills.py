"""从名字或 DIY 技能组导出条件抽样配置。构造和特征导出都不运行对局。"""

import argparse
import json
import os
import re
import subprocess
import tempfile
from pathlib import Path


REPO = Path(__file__).resolve().parents[1]
OVERLAY_FIELDS = {
    "effective_skills": True,
    "effective_skill_redistribution": 1.0,
}
SUFFIX_TOKEN = re.compile(r'"(?:\\.|[^"\\])*"|\+')


def encode(value):
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"))


def construct(raw):
    identity, separator, suffix = raw.partition("+")
    segments = []
    if separator:
        start = 0
        for token in SUFFIX_TOKEN.finditer(suffix):
            if token[0] == "+":
                segments.append(suffix[start:token.start()])
                start = token.end()
        segments.append(suffix[start:])
    for index in range(len(segments) - 1, -1, -1):
        segment = segments[index].strip()
        if segment.startswith("ol:"):
            prefix, data = "ol:", segment[3:]
        elif segment.startswith("diy["):
            attrs, bracket, data = segment.partition("]")
            if not bracket:
                raise ValueError(f"DIY 属性缺少右方括号: {raw}")
            prefix = attrs + bracket
        else:
            continue
        overlay = json.loads(data) if data.strip() else {}
        if prefix.startswith("diy[") and data.strip() and not overlay:
            # 紧凑格式的空对象显式清除技能，转换为 OL 后继续保留这一语义。
            attrs = [int(value) for value in prefix[4:-1].split(",") if value.strip()]
            prefix, overlay = "ol:", {"attrs": attrs, "skills": {}}
        overlay.pop("effective_skill_priority", None)
        overlay.pop("effective_skill_adjustment", None)
        overlay.update(OVERLAY_FIELDS)
        segments[index] = prefix + encode(overlay)
        break
    else:
        segments.append("ol:" + encode(OVERLAY_FIELDS))
    return "+".join((identity, *segments))


def write_features(cli, names, constructed, output):
    if not names:
        output.write_text("", encoding="utf-8")
        return
    with tempfile.TemporaryDirectory(dir=constructed.resolve().parent) as directory:
        exported = Path(directory) / "prepared.txt"
        subprocess.run([cli.resolve(), "to-diy", "-f", constructed.resolve(), "-o", exported, "--no-details"],
                       stdout=subprocess.DEVNULL, check=True)
        prepared = exported.read_text(encoding="utf-8").splitlines()
    rows = []
    for name, built in zip(names, prepared, strict=True):
        overlay = json.loads(built.rsplit("+ol:", 1)[1])
        if "effective_skill_adjustment" not in overlay:
            raise ValueError(f"此名字没有条件抽样技能组: {name}")
        weights = overlay["effective_skills"]
        adjustment = overlay["effective_skill_adjustment"]
        rows.append({
            "name": name,
            "effective_skills": weights,
            "effective_skill_adjustment": adjustment,
            "effective_skill_redistribution": overlay["effective_skill_redistribution"],
        })
    output.write_text("".join(encode(row) + "\n" for row in rows), encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path, help="原始名字，每行一个，允许已有 DIY")
    parser.add_argument("output", type=Path, help="可直接交给模拟器的名单")
    parser.add_argument("--features", type=Path, help="另存按技能键索引的 35 个主值与最多 10 个动态参数（JSONL）")
    parser.add_argument("--cli", type=Path, default=REPO / "target" / "release-fast" /
                        ("tswn-cli.exe" if os.name == "nt" else "tswn-cli"))
    args = parser.parse_args()
    paths = [args.input, args.output] + ([args.features] if args.features else [])
    if len({path.resolve() for path in paths}) != len(paths):
        parser.error("输入、构造名单和特征须使用不同的文件")
    names = args.input.read_text(encoding="utf-8-sig").splitlines()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text("".join((construct(line) if line.strip() else "") + "\n" for line in names),
                           encoding="utf-8")
    if args.features:
        args.features.parent.mkdir(parents=True, exist_ok=True)
        write_features(args.cli, [line for line in names if line.strip()], args.output, args.features)


if __name__ == "__main__":
    main()
