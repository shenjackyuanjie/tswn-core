"""采集 tswn-winprob-dataset 生成与校验的规模基准。

本脚本只调用生成器、采样子进程资源占用并读取产物的 Parquet 元数据，
不改变数据集格式，也不重新实现任何抽样或校验逻辑。

采集项：
- 生成/校验墙钟时间，按分片日志估计的吞吐
- 子进程峰值 RSS、峰值线程数、CPU 时间与 CPU 利用率
- Parquet 实际文件大小、列块压缩/未压缩字节与压缩比
- 每对局、每样本字节数，以及嵌套 state 各字段的压缩字节占比
"""

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path


def resolve_binary(root: Path, explicit: str | None) -> Path:
    """定位 release 版生成器；找不到时直接报错，避免误用 debug 产物。"""
    if explicit:
        path = Path(explicit)
        if not path.is_file():
            raise SystemExit(f"找不到生成器：{path}")
        return path
    name = "tswn-winprob-dataset.exe" if sys.platform == "win32" else "tswn-winprob-dataset"
    path = root / "target" / "release" / name
    if not path.is_file():
        raise SystemExit(f"找不到 release 产物：{path}，请先构建")
    return path


def run_with_sampling(command: list[str], cwd: Path, interval: float, log_dir: Path, tag: str) -> dict:
    """运行命令并采样其资源占用；返回墙钟、峰值 RSS/线程、CPU 时间与退出码。

    子进程输出写入工作区内的日志文件而不是系统临时目录，便于事后复查。
    """
    import psutil

    log_dir.mkdir(parents=True, exist_ok=True)
    stdout_path = log_dir / f"{tag}.stdout.txt"
    stderr_path = log_dir / f"{tag}.stderr.txt"
    started = time.perf_counter()
    with stdout_path.open("w", encoding="utf-8") as out, stderr_path.open("w", encoding="utf-8") as err:
        proc = subprocess.Popen(command, cwd=cwd, stdout=out, stderr=err, text=True)
        process = psutil.Process(proc.pid)
        peak_rss = 0
        peak_threads = 0
        cpu_seconds = 0.0
        write_bytes = 0
        samples = 0
        while proc.poll() is None:
            try:
                peak_rss = max(peak_rss, process.memory_info().rss)
                peak_threads = max(peak_threads, process.num_threads())
                times = process.cpu_times()
                cpu_seconds = times.user + times.system
                write_bytes = max(write_bytes, process.io_counters().write_bytes)
            except psutil.Error:
                break
            samples += 1
            time.sleep(interval)
        returncode = proc.wait()
    wall = time.perf_counter() - started
    stdout = stdout_path.read_text(encoding="utf-8", errors="replace")
    stderr = stderr_path.read_text(encoding="utf-8", errors="replace")
    return {
        "command": command,
        "returncode": returncode,
        "wall_seconds": wall,
        "peak_rss_bytes": peak_rss,
        "peak_threads": peak_threads,
        "cpu_seconds": cpu_seconds,
        "write_bytes": write_bytes,
        "samples_taken": samples,
        "stdout_path": str(stdout_path),
        "stderr_path": str(stderr_path),
        "stdout": stdout,
        "stderr": stderr,
    }


def parquet_stats(path: Path) -> dict:
    """读取单个 Parquet 文件的元数据，按叶子列聚合压缩字节。"""
    import pyarrow.parquet as pq

    meta = pq.ParquetFile(path).metadata
    compressed = 0
    uncompressed = 0
    columns: dict[str, list[int]] = {}
    for group in range(meta.num_row_groups):
        row_group = meta.row_group(group)
        for index in range(row_group.num_columns):
            chunk = row_group.column(index)
            compressed += chunk.total_compressed_size
            uncompressed += chunk.total_uncompressed_size
            entry = columns.setdefault(chunk.path_in_schema, [0, 0])
            entry[0] += chunk.total_compressed_size
            entry[1] += chunk.total_uncompressed_size
    return {
        "path": str(path),
        "file_bytes": path.stat().st_size,
        "rows": meta.num_rows,
        "row_groups": meta.num_row_groups,
        "column_compressed_bytes": compressed,
        "column_uncompressed_bytes": uncompressed,
        "compression_ratio": (uncompressed / compressed) if compressed else None,
        "columns": {name: {"compressed": value[0], "uncompressed": value[1]} for name, value in columns.items()},
    }


def summarize_dataset(root: Path, top_n: int) -> dict:
    """聚合所有分片，给出总量、每对局/每样本字节与嵌套字段占比。"""
    shards = sorted(path for path in root.glob("shard-*") if path.is_dir())
    if not shards:
        raise SystemExit(f"{root} 下没有已完成分片")
    totals = {"shards": len(shards), "battles": 0, "samples": 0, "file_bytes": 0, "row_groups": 0}
    merged: dict[str, list[int]] = {}
    per_table: dict[str, dict] = {}
    for shard in shards:
        for name in ("battles.parquet", "samples.parquet"):
            stats = parquet_stats(shard / name)
            table = per_table.setdefault(
                name, {"file_bytes": 0, "rows": 0, "row_groups": 0, "compressed": 0, "uncompressed": 0}
            )
            table["file_bytes"] += stats["file_bytes"]
            table["rows"] += stats["rows"]
            table["row_groups"] += stats["row_groups"]
            table["compressed"] += stats["column_compressed_bytes"]
            table["uncompressed"] += stats["column_uncompressed_bytes"]
            totals["file_bytes"] += stats["file_bytes"]
            totals["row_groups"] += stats["row_groups"]
            for column, value in stats["columns"].items():
                entry = merged.setdefault(column, [0, 0])
                entry[0] += value["compressed"]
                entry[1] += value["uncompressed"]
    totals["battles"] = per_table["battles.parquet"]["rows"]
    totals["samples"] = per_table["samples.parquet"]["rows"]
    for table in per_table.values():
        table["compression_ratio"] = (table["uncompressed"] / table["compressed"]) if table["compressed"] else None
    sample_compressed = per_table["samples.parquet"]["compressed"]

    def group(depth: int) -> list[dict]:
        """按列路径前若干级聚合，定位压缩字节集中在哪个子树。"""
        buckets: dict[str, int] = {}
        for path, value in merged.items():
            key = ".".join(path.split(".")[:depth])
            buckets[key] = buckets.get(key, 0) + value[0]
        return [
            {"path": key, "compressed_bytes": size, "share_of_samples_columns": (size / sample_compressed) if sample_compressed else None}
            for key, size in sorted(buckets.items(), key=lambda item: item[1], reverse=True)
        ]

    top_columns = sorted(merged.items(), key=lambda item: item[1][0], reverse=True)[:top_n]
    return {
        "totals": totals,
        "tables": per_table,
        "bytes_per_battle": totals["file_bytes"] / totals["battles"] if totals["battles"] else None,
        "bytes_per_sample": totals["file_bytes"] / totals["samples"] if totals["samples"] else None,
        "samples_parquet_bytes_per_sample": per_table["samples.parquet"]["file_bytes"] / totals["samples"] if totals["samples"] else None,
        "groups": {"top1": group(1), "top2": group(2), "top3": group(3)},
        "top_columns": [
            {
                "path": path,
                "compressed_bytes": value[0],
                "uncompressed_bytes": value[1],
                "share_of_samples_columns": (value[0] / sample_compressed) if sample_compressed else None,
            }
            for path, value in top_columns
        ],
    }


def load_summary_json(root: Path) -> dict | None:
    """生成器写在 summary.json 里的对局统计。"""
    path = root / "summary.json"
    if not path.is_file():
        return None
    return json.loads(path.read_text(encoding="utf-8"))


def build_command(binary: Path, args: argparse.Namespace) -> list[str]:
    """把脚本参数翻译成生成器命令行。"""
    command = [str(binary), "generate", "--out", str(args.out), "--seed", args.seed]
    if args.input:
        command += ["--input", str(args.input)]
    if args.names:
        command += ["--names", str(args.names)]
    if args.team_sizes:
        command += ["--team-sizes", args.team_sizes]
    if args.matchups:
        command += ["--matchups", str(args.matchups)]
    command += [
        "--games-per-matchup", str(args.games_per_matchup),
        "--samples-per-game", str(args.samples_per_game),
        "--battles-per-shard", str(args.battles_per_shard),
        "--max-rounds", str(args.max_rounds),
        "--threads", str(args.threads),
    ]
    if args.resume:
        command.append("--resume")
    return command


def main() -> int:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--root", type=Path, default=Path.cwd(), help="仓库根目录")
    parser.add_argument("--bin", dest="binary", help="生成器可执行文件；默认为 target/release 下的产物")
    parser.add_argument("--out", type=Path, required=True, help="数据集输出目录")
    parser.add_argument("--input", help="固定对局文件或目录")
    parser.add_argument("--names", help="名字池文件")
    parser.add_argument("--team-sizes", help="名字池模式的队伍人数，如 2,2,2")
    parser.add_argument("--matchups", type=int, default=0, help="名字池模式抽取的阵容数")
    parser.add_argument("--games-per-matchup", type=int, default=20)
    parser.add_argument("--samples-per-game", type=int, default=8)
    parser.add_argument("--battles-per-shard", type=int, default=1000)
    parser.add_argument("--max-rounds", type=int, default=20000)
    parser.add_argument("--threads", type=int, default=0)
    parser.add_argument("--seed", default="benchmark")
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--interval", type=float, default=0.2, help="资源采样间隔（秒）")
    parser.add_argument("--top", type=int, default=20, help="输出压缩占比最高的列数")
    parser.add_argument("--skip-generate", action="store_true", help="只分析已有产物")
    parser.add_argument("--label", default="run", help="结果标签")
    parser.add_argument("--json-out", type=Path, help="把完整结果写入该 JSON 文件")
    args = parser.parse_args()

    binary = resolve_binary(args.root, args.binary)
    result: dict = {"label": args.label, "binary": str(binary), "binary_bytes": binary.stat().st_size}
    log_dir = args.root / "target" / "benchmark-logs"

    if not args.skip_generate:
        command = build_command(binary, args)
        print(f"[benchmark] 生成：{' '.join(command)}", flush=True)
        generate = run_with_sampling(command, args.root, args.interval, log_dir, f"{args.label}-generate")
        result["generate"] = generate
        if generate["returncode"] != 0:
            print(generate["stderr"][-4000:], file=sys.stderr)
            print(f"[benchmark] 生成失败，退出码 {generate['returncode']}", file=sys.stderr)
            return generate["returncode"]
        result["summary_json"] = load_summary_json(args.out)

    validate_command = [str(binary), "validate", "--out", str(args.out)]
    print(f"[benchmark] 校验：{' '.join(validate_command)}", flush=True)
    result["validate"] = run_with_sampling(validate_command, args.root, args.interval, log_dir, f"{args.label}-validate")
    if result["validate"]["returncode"] != 0:
        print(result["validate"]["stderr"][-4000:], file=sys.stderr)
        return result["validate"]["returncode"]

    result["dataset"] = summarize_dataset(args.out, args.top)

    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")

    report = result
    totals = report["dataset"]["totals"]
    tables = report["dataset"]["tables"]
    print()
    print(f"## {args.label}")
    print()
    if "generate" in report:
        gen = report["generate"]
        print(f"- 生成墙钟：{gen['wall_seconds']:.2f} s，{totals['battles'] / gen['wall_seconds']:.1f} 对局/s，"
              f"{totals['samples'] / gen['wall_seconds']:.1f} 样本/s")
        print(f"- 生成峰值 RSS：{gen['peak_rss_bytes'] / 2**20:.1f} MiB，峰值线程：{gen['peak_threads']}，"
              f"CPU 时间：{gen['cpu_seconds']:.1f} s，CPU 利用率：{gen['cpu_seconds'] / gen['wall_seconds'] * 100:.0f}% 单核")
        print(f"- 进程写入字节：{gen['write_bytes'] / 2**20:.1f} MiB")
    val = report["validate"]
    print(f"- 校验墙钟：{val['wall_seconds']:.2f} s，{totals['battles'] / val['wall_seconds']:.1f} 对局/s，"
          f"峰值 RSS：{val['peak_rss_bytes'] / 2**20:.1f} MiB")
    print(f"- 对局 {totals['battles']}，样本 {totals['samples']}，分片 {totals['shards']}，行组 {totals['row_groups']}")
    print(f"- 总文件：{totals['file_bytes'] / 2**20:.1f} MiB，每对局 {report['dataset']['bytes_per_battle']:.0f} B，"
          f"每样本 {report['dataset']['bytes_per_sample']:.0f} B")
    for name, table in tables.items():
        ratio = table["compression_ratio"]
        ratio_text = f"{ratio:.2f}x" if ratio else "-"
        print(f"- `{name}`：{table['file_bytes'] / 2**20:.2f} MiB，{table['rows']} 行，{table['row_groups']} 行组，"
              f"列块压缩比 {ratio_text}")
    for depth in ("top1", "top2", "top3"):
        print()
        print(f"按路径前 {depth[-1]} 级聚合（samples 列块压缩字节）：")
        print()
        print("| 路径 | 压缩字节 | 占 samples 列块 |")
        print("| --- | --- | --- |")
        for item in report["dataset"]["groups"][depth][: args.top]:
            print(f"| `{item['path']}` | {item['compressed_bytes'] / 1024:.1f} KiB | {item['share_of_samples_columns'] * 100:.1f}% |")
    print()
    print(f"压缩字节最高的 {args.top} 个叶子列：")
    print()
    print("| 列路径 | 压缩字节 | 占 samples 列块 |")
    print("| --- | --- | --- |")
    for column in report["dataset"]["top_columns"]:
        share = column["share_of_samples_columns"]
        print(f"| `{column['path']}` | {column['compressed_bytes'] / 1024:.1f} KiB | {share * 100:.1f}% |")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
