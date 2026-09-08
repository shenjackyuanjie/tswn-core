#!/usr/bin/env python3
"""测量 release Python BattleSession API，包含现有 JSON DTO 转换的开销。

沿用 verify_py_cli_api 的独立导入目录构建并加载本地扩展。
可选传入 release Node WASM 包，在同机比较优化门槛 B。
本脚本只记录性能，不修改 DTO 实现；原始样本输出为 JSON。
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import gc
import hashlib
import importlib
import json
import math
from pathlib import Path
import platform
import subprocess
import sys
from time import get_clock_info, perf_counter_ns

from verify_py_cli_api import ROOT, prepare_import_tree

FIXTURES = [
    "1v1-0f92cb76cc37fdc5",
    "2v2-554f4128af707167",
    "ffa_8-16d11de1ebe1df41",
    "3v3v3-0ace5df17b84e26a",
]
OPTIONS = {"max_rounds": 20_000, "include_icons": False}


def percentile(values: list[float], quantile: float) -> float:
    """采用 nearest-rank 分位数，与现有网页基线保持一致。"""
    return sorted(values)[max(0, math.ceil(len(values) * quantile) - 1)]


def distribution(values: list[float]) -> dict:
    return {"p50": percentile(values, 0.50), "p95": percentile(values, 0.95), "max": max(values)}


def run_session(binding, raw: str) -> dict:
    begin = perf_counter_ns()
    session = binding.BattleSession(raw, **OPTIONS)
    created = perf_counter_ns()
    samples = []
    frames = 0
    while True:
        start = perf_counter_ns()
        frame = session.next_frame()
        elapsed_ms = (perf_counter_ns() - start) / 1e6
        # 包含最后一次返回 None 的调用；每帧对象的释放放在该次调用计时之外。
        samples.append(elapsed_ms)
        if frame is None:
            break
        frames += 1
        del frame
    end = perf_counter_ns()
    # 结果校验和 result 转换均放在报告的计时区间之外。
    result = session.result()
    assert session.is_done() and not session.is_failed()
    assert result["finished"] and result["stop_reason"] == "winner", result
    assert frames == result["frames_emitted"]
    return {
        "session_create_ms": (created - begin) / 1e6,
        "next_frame_wall_ms": samples,
        "total_session_ms": (end - begin) / 1e6,
        "frames": frames,
        "winner_ids": result["winner_ids"],
        "rounds_advanced": result["rounds_advanced"],
    }


def summarize(runs: list[dict]) -> dict:
    frames = {run["frames"] for run in runs}
    assert len(frames) == 1, f"同一输入的帧数不一致：{frames}"
    assert all((run["winner_ids"], run["rounds_advanced"]) ==
               (runs[0]["winner_ids"], runs[0]["rounds_advanced"]) for run in runs)
    calls = [sample for run in runs for sample in run["next_frame_wall_ms"]]
    return {
        "sessions": len(runs),
        "session_create_ms": distribution([run["session_create_ms"] for run in runs]),
        "next_frame_wall_ms": distribution(calls),
        "total_session_ms": distribution([run["total_session_ms"] for run in runs]),
        "frames_per_session": frames.pop(),
        "next_frame_calls": len(calls),
    }


def command_output(*args: str) -> str:
    return subprocess.check_output(args, cwd=ROOT, encoding="utf-8").strip()


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    # Windows 重定向输出时也统一使用 UTF-8，确保中文说明和日志可读。
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    parser = argparse.ArgumentParser(description=__doc__, add_help=False)
    parser.add_argument("-h", "--help", action="help", help="显示帮助并退出")
    parser.add_argument("--sessions", type=int, default=100, help="每类 fixture 正式测量的场数（至少 100）")
    parser.add_argument("--warmup", type=int, default=10, help="每类 fixture 不计入统计的预热场数")
    parser.add_argument("--output", type=Path, default=ROOT / "target/python_battle_session/baseline.json",
                        help="原始样本 JSON 的输出路径")
    parser.add_argument("--wasm-package", type=Path, help="可选 release wasm-bindgen Node 包，用于检查优化门槛 B")
    args = parser.parse_args()
    if args.sessions < 100 or args.warmup < 1:
        parser.error("要求 --sessions >= 100 且 --warmup >= 1")
    if args.wasm_package and not args.wasm_package.resolve().is_file():
        parser.error("--wasm-package 必须指向已生成的 release Node 包")

    prepare_import_tree(release=True)
    binding = importlib.import_module("tswn_py")
    extension = importlib.import_module("tswn_py.tswn_py")
    environment = {
        "timestamp_utc": datetime.now(timezone.utc).isoformat(),
        "git_revision": command_output("git", "rev-parse", "HEAD"),
        "benchmark_script_sha256": digest(Path(__file__)),
        "python": sys.version,
        "platform": platform.platform(),
        "cpu": platform.processor(),
        "rustc": command_output("rustc", "--version"),
        "build": "cargo build -p tswn_py --release （workspace release 配置）",
        "wrapper_version": binding.wrapper_version_str(),
        "core_version": binding.core_version_str(),
        "extension_sha256": digest(Path(extension.__file__)),
        "gc_enabled": gc.isenabled(),
        "timer_resolution_seconds": get_clock_info("perf_counter").resolution,
        "options": OPTIONS,
        "sessions_per_fixture": args.sessions,
        "warmup_sessions_per_fixture": args.warmup,
        "method": "nearest-rank；合并全部 next_frame 调用，包含末次 None/null；"
                  "不计入 initial_states/result 转换和模块加载；"
                  "总耗时包含循环、计时和每帧释放的开销；保持正常 GC",
    }
    records = []
    jobs = []
    for fixture in FIXTURES:
        path = ROOT / "crates/tswn_test/cases/runtime_stress" / f"{fixture}.txt"
        raw = path.read_text(encoding="utf-8")
        for _ in range(args.warmup):
            run_session(binding, raw)
        runs = [run_session(binding, raw) for _ in range(args.sessions)]
        summary = summarize(runs)
        records.append({"fixture": fixture, "input_sha256": digest(path), "python": summary, "python_runs": runs})
        jobs.append({"fixture": fixture, "raw": raw, "options": OPTIONS})
        print(f"{fixture}：{args.sessions} 场，每场 {summary['frames_per_session']} 帧，"
              f"next_frame p95={summary['next_frame_wall_ms']['p95']:.6f} ms", flush=True)

    if args.wasm_package:
        package = args.wasm_package.resolve()
        request = {"package": str(package), "sessions": args.sessions, "warmup": args.warmup, "jobs": jobs}
        process = subprocess.run(
            ["node", str(ROOT / "scripts/benchmark_wasm_battle_session.mjs")],
            input=json.dumps(request), capture_output=True, encoding="utf-8", check=True, cwd=ROOT,
        )
        comparison = json.loads(process.stdout)
        environment["wasm_comparison"] = {
            "node": comparison["node"],
            "package": str(package),
            "wasm_sha256": digest(package.with_name(package.stem + "_bg.wasm")),
            "build": "调用方提供的 release wasm-bindgen Node 包；不包含 DOM 或浏览器播放",
        }
        for record, wasm in zip(records, comparison["records"], strict=True):
            assert record["fixture"] == wasm["fixture"]
            assert all((run["frames"], run["winner_ids"], run["rounds_advanced"]) ==
                       (record["python_runs"][0]["frames"], record["python_runs"][0]["winner_ids"],
                        record["python_runs"][0]["rounds_advanced"]) for run in wasm["runs"])
            record["wasm"] = summarize(wasm["runs"])
            record["wasm_runs"] = wasm["runs"]

    for record in records:
        py_p95 = record["python"]["next_frame_wall_ms"]["p95"]
        ratio = py_p95 / record["wasm"]["next_frame_wall_ms"]["p95"] if "wasm" in record else None
        record["gates"] = {
            "A_python_p95_at_least_1ms": py_p95 >= 1.0,
            "B_python_at_least_4x_wasm": ratio >= 4.0 if ratio is not None else None,
            "python_to_wasm_p95_ratio": ratio,
            "C_conversion_cpu_at_least_25pct": None,
        }
    measured_gate = any(record["gates"]["A_python_p95_at_least_1ms"] or
                        record["gates"]["B_python_at_least_4x_wasm"] for record in records)
    result = {
        "environment": environment,
        "records": records,
        "conclusion": {
            "measured_gate_triggered": measured_gate,
            "action": "应另开独立优化任务" if measured_gate else "保留 serde_json -> json.loads",
            "limitation": "门槛 C 需要下一阶段真实任务的 CPU profiling；调用总耗时不能单独测出 DTO 的 CPU 占比。",
        },
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result["conclusion"], ensure_ascii=False))
    print(f"原始样本：{args.output}")


if __name__ == "__main__":
    main()
