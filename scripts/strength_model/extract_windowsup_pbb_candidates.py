#!/usr/bin/env python3
"""从 Windowsup 的历史“结果文件”精确提取 fAIgFUL 名字，排除输入与网页脚本。"""
import argparse
import json
import re
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path

# 仅限明确的历史输出位置；刻意不读取 in/Input、网页 .js、候选名单或二进制文件。
RESULT_PATHS = (
    "15净化/out.txt",
    "HX/out.txt",
    "HX/Outputs/*.txt",
    "menu/out/*.txt",
    "测号原程序/七位筛2/out.txt",
    "测号原程序/汉字/[0-9]*.txt",
    "测号原程序/汉字/已合并",
    "测号原程序/汉字/已合并.txt",
    "留档/*.txt",
    "留档/已合并/*",
)
TEAM = "fAIgFUL"
AT = "@" + TEAM


def sources(root: Path):
    answer = set()
    for pattern in RESULT_PATHS:
        answer.update(path for path in root.glob(pattern) if path.is_file())
    return sorted(answer)


def clean_name(value: str) -> str | None:
    value = value.strip().strip("'\"")
    # 部分旧输出在名前带两项评分；这两项不属于名字本身。
    value = re.sub(r"^\d+\s+\d+\s+", "", value)
    # 旧 Windows 程序的乱码行无法无损恢复 UTF-8 原名，宁可跳过而不污染训练集。
    if "�" in value or not value or len(value) > 96 or any(ord(c) < 32 for c in value):
        return None
    return value


def names_from_line(line: str):
    # 组合结果使用 + 连接，逐段取出，避免把整个组合错误地作为单名导入。
    for segment in line.rstrip("\r\n").split("+"):
        index = segment.find(AT)
        if index < 0:
            continue
        name = clean_name(segment[:index])
        if name is not None:
            yield name


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True, help="输出精确 name<TAB>team TSV")
    parser.add_argument("--manifest", type=Path, required=True)
    args = parser.parse_args()
    counts, chosen = Counter(), set()
    files = sources(args.root)
    for path in files:
        # 流式读取：历史留档可能很大，不能一次性展开数百万行。
        with path.open(encoding="utf-8", errors="replace") as handle:
            for line in handle:
                if AT not in line:
                    continue
                for name in names_from_line(line):
                    chosen.add(name); counts[str(path.relative_to(args.root))] += 1
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text("".join(f"{name}\t{TEAM}\n" for name in sorted(chosen)) , encoding="utf-8")
    manifest = {"root": str(args.root), "team": TEAM, "files": [str(p.relative_to(args.root)) for p in files],
                "raw_accepted_by_file": dict(counts), "unique_names": len(chosen),
                "created_at": datetime.now(timezone.utc).isoformat()}
    args.manifest.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"files={len(files)} unique_names={len(chosen)}")


if __name__ == "__main__":
    main()
