#!/usr/bin/env python3
"""以隔离的预构建产物运行维护前后同机交替 A/B；不在测量中构建或安装依赖。

先按维护报告准备 baseline/current 的 native、python、wasm-node、wasm-web 目录。
每个配置各预热一轮，再奇数轮旧→新、偶数轮新→旧；原始 stdout/stderr/JSON
均保留在 --bench-dir。pwp_encode 单独执行前须先完成 pwp 阶段。--resume 只复用已成功且有元数据的完整调用，不混入失败样本。
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import itertools
import json
import os
from pathlib import Path
import re
import shutil
import statistics
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ["1v1-0f92cb76cc37fdc5", "2v2-554f4128af707167", "ffa_8-16d11de1ebe1df41", "3v3v3-0ace5df17b84e26a"]
PLAYERS = "docs/perf/cqp/sqp6000_first20.txt"
TARGET1 = "crates/tswn_openbox_backend/assets/targets/target1.txt"
TARGET2 = "crates/tswn_openbox_backend/assets/targets/target2.txt"
TEAMMATES = "crates/tswn_openbox_backend/assets/teammates/teammate_fz.txt"


def load(path: Path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def save(path: Path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def signature(value) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, ensure_ascii=False).encode()).hexdigest()


def metric(key, seconds, sig, operations=None, **extra):
    return {"key": key, "seconds": seconds, "signature": sig, "operations": operations, **extra}


def runtime_metrics(name, rows):
    result = []
    for row in rows:
        assert row["errors"] == 0, row
        case = Path(row["input"]).name
        sig = signature({k: row[k] for k in ("group_sizes", "total", "wins", "errors", "guard_exhausted")})
        result.append(metric(f"{name}/case/{case}", row["elapsed_seconds"], sig, row["total"],
                             wins=row["wins"], guard_exhausted=row["guard_exhausted"],
                             init_seconds=row["init_nanos"] / 1e9, fight_seconds=row["fight_nanos"] / 1e9))
    groups = {"overall": rows}
    if name.startswith("fixed30"):
        assert len(rows) == 30, len(rows)
        groups.update({
            "core_1v1_2v2": [r for r in rows if re.match(r"^\d+_(1v1|2v2)-", Path(r["input"]).name)],
            "1v1": [r for r in rows if re.match(r"^\d+_1v1-", Path(r["input"]).name)],
            "2v2": [r for r in rows if re.match(r"^\d+_2v2-", Path(r["input"]).name)],
            "multi": [r for r in rows if not re.match(r"^\d+_(1v1|2v2)-", Path(r["input"]).name)],
        })
    for group, selected in groups.items():
        sig = signature([{k: r[k] for k in ("group_sizes", "total", "wins", "errors", "guard_exhausted")} for r in selected])
        result.append(metric(f"{name}/{group}", sum(r["elapsed_seconds"] for r in selected), sig,
                             sum(r["total"] for r in selected), cases=len(selected),
                             wins=sum(r["wins"] for r in selected), guard_exhausted=sum(r["guard_exhausted"] for r in selected),
                             init_seconds=sum(r["init_nanos"] for r in selected) / 1e9,
                             fight_seconds=sum(r["fight_nanos"] for r in selected) / 1e9))
    return result


class Runner:
    def __init__(self, args):
        self.args = args
        self.base = args.bench_dir.resolve()
        self.raw = self.base / "raw"
        self.inputs = self.base / "inputs"
        self.raw.mkdir(parents=True, exist_ok=True)
        self.inputs.mkdir(parents=True, exist_ok=True)
        self.environment = load(self.base / "environment.json")
        self.output_paths = {}

    def prepare_inputs(self):
        for count in (2, 8):
            lines = (ROOT / PLAYERS).read_text(encoding="utf-8-sig").splitlines()
            (self.inputs / f"players{count}.txt").write_text("\n".join(lines[:count]) + "\n", encoding="utf-8")
        lines = (ROOT / TEAMMATES).read_text(encoding="utf-8-sig").splitlines()
        (self.inputs / "teammates4.txt").write_text("\n".join(lines[:4]) + "\n", encoding="utf-8")
        (self.inputs / "target1.txt").write_text((ROOT / TARGET1).read_text(encoding="utf-8-sig").splitlines()[0] + "\n", encoding="utf-8")
        for name, raw in {"simple": "aaa\n\nbbb\n", "complex": "喘际瞬爆@昀澤\n\n蕾蒂·怀特洛可-65HEZHB264LFPFQ@Squall\n"}.items():
            (self.inputs / f"{name}.txt").write_text(raw, encoding="utf-8")
        # 历史双人32组未跟踪；从固定20选手的索引组合顺序派生，不冒充历史输入。
        names = (ROOT / PLAYERS).read_text(encoding="utf-8-sig").splitlines()
        pairs = list(itertools.islice(itertools.combinations(names, 2), 32))
        self.double_lines = self.inputs / "derived-doubles32-lines.txt"
        self.double_groups = self.inputs / "derived-doubles32-groups.txt"
        self.double_lines.write_text("\n".join("+".join(pair) for pair in pairs) + "\n", encoding="utf-8")
        self.double_groups.write_text("\n\n".join("\n".join(pair) for pair in pairs) + "\n", encoding="utf-8")
        tracked = sorted((ROOT / "docs/perf/fixed_cases_30").glob("*.txt"))
        tracked += sorted((ROOT / "docs/perf/score").glob("*.txt"))
        tracked += sorted((ROOT / "docs/perf/cqp").glob("*.txt"))
        tracked += [ROOT / p for p in (TARGET1, TARGET2, TEAMMATES, "crates/tswn_openbox_backend/assets/teammates/teammate_fz.toml")]
        tracked += [ROOT / "crates/tswn_test/cases/runtime_stress" / f"{f}.txt" for f in FIXTURES]
        records = []
        for path in tracked:
            relative = path.relative_to(ROOT)
            base = self.base / "baseline-src" / relative
            # Git worktree可能按autocrlf检出；实际两版共读ROOT输入，另核对LF规范文本一致。
            assert base.is_file() and base.read_text(encoding="utf-8-sig") == path.read_text(encoding="utf-8-sig"), f"两版固定输入不一致：{relative}"
            normalized = path.read_text(encoding="utf-8-sig").encode("utf-8")
            records.append({"path": relative.as_posix(), "sha256": digest(path), "baseline_checkout_sha256": digest(base), "lf_sha256": hashlib.sha256(normalized).hexdigest()})
        records += [{"path": os.path.relpath(p, ROOT), "sha256": digest(p), "derived": True} for p in sorted(self.inputs.glob("*.txt"))]
        save(self.base / "input-hashes.json", records)

    def executable(self, variant, name):
        return str(self.base / variant / "native" / f"{name}.exe")

    def invoke(self, name, variant, round_no, command, parse, env=None, cwd=ROOT):
        tag = f"{name}--{variant}--r{round_no}"
        metadata = self.raw / f"{tag}.json"
        overrides = {"PYTHONUTF8": "1", **(env or {})}
        identity = {"command": [str(a) for a in command], "cwd": str(cwd),
                    "code_revision": self.environment[variant], "driver_sha256": digest(Path(__file__)),
                    "environment_overrides": overrides, "input_hashes_sha256": digest(self.base / "input-hashes.json"),
                    "artifact_hashes_sha256": digest(self.base / variant / "artifact-hashes.json")}
        if self.args.resume and metadata.exists():
            previous = load(metadata)
            if previous.get("returncode") == 0 and "metrics" in previous:
                if not all(previous.get(key) == value for key, value in identity.items()):
                    raise RuntimeError(f"{tag} 的驱动/输入/产物/参数身份变化，请使用新的bench目录，不复用旧样本")
                print(f"复用 {tag}", flush=True)
                return
        stdout = self.raw / f"{tag}.stdout.log"
        stderr = self.raw / f"{tag}.stderr.log"
        started_at = datetime.now(timezone.utc).isoformat()
        start = time.perf_counter()
        with stdout.open("wb") as out, stderr.open("wb") as err:
            process = subprocess.run([str(a) for a in command], cwd=cwd, env={**os.environ, **overrides}, stdout=out, stderr=err, timeout=3600)
        elapsed = time.perf_counter() - start
        record = {"name": name, "variant": variant, "round": round_no, "warmup": round_no == 0,
                  "code_revision": self.environment[variant], "driver_base_revision": self.environment["current"], "driver_sha256": digest(Path(__file__)),
                  "started_at_utc": started_at, "command": [str(a) for a in command], "cwd": str(cwd),
                  "environment_overrides": overrides, "returncode": process.returncode, "process_seconds": elapsed,
                  "stdout": str(stdout.relative_to(self.base)), "stderr": str(stderr.relative_to(self.base))}
        record.update(identity)
        if process.returncode:
            save(metadata, record)
            raise RuntimeError(f"{tag} 失败（退出 {process.returncode}）：{stderr.read_text(encoding='utf-8', errors='replace')[-4000:]}")
        record["metrics"] = parse(stdout.read_text(encoding="utf-8-sig"), stderr.read_text(encoding="utf-8-sig"))
        save(metadata, record)
        print(f"完成 {tag}，进程墙钟 {elapsed:.3f}s，{len(record['metrics'])} 项指标", flush=True)

    def fresh_output(self, prefix, variant, round_no, task):
        """失败写入后换新目录；成功续跑使用元数据中的原路径，不把零工作量当基准。"""
        key = (prefix, variant, round_no)
        if key in self.output_paths:
            return self.output_paths[key]
        original = self.raw / f"{prefix}--{variant}--r{round_no}"
        metadata = self.raw / f"{task}--{variant}--r{round_no}.json"
        previous = load(metadata) if metadata.exists() else {}
        if self.args.resume and previous.get("returncode") == 0 and "metrics" in previous:
            command = previous["command"]
            path = Path(command[command.index("--out") + 1])
        else:
            path = original
            attempt = 1
            while path.exists():
                attempt += 1
                path = original.with_name(original.name + f"-attempt{attempt}")
        self.output_paths[key] = path
        return path

    def paired(self, name, command, parse, env=None):
        for round_no in range(self.args.rounds + 1):
            order = ("baseline", "current") if round_no % 2 else ("current", "baseline")
            for variant in order:
                self.invoke(name, variant, round_no, command(variant, round_no),
                            lambda out, err, v=variant, r=round_no: parse(out, err, v, r),
                            env(variant, round_no) if env else None)

    def runtime(self):
        for threads in (1, 0):
            for timed in (False, True):
                name = f"fixed30-t{threads}-{'timed' if timed else 'hot'}"
                self.paired(name, lambda v, r: [self.executable(v, "perf_runtime"), "-n", "13000", "--threads", str(threads), *(["--timed"] if timed else [])],
                            lambda out, err, v, r: runtime_metrics(name, [json.loads(line) for line in out.splitlines() if line]))
        for label, path, runs in (("mario", "docs/perf/score/mario.txt", 13000), ("cqp", PLAYERS, 1000), ("derived-cqd32", self.double_groups, 1000)):
            for timed in (False, True):
                name = f"score-{label}-{'timed' if timed else 'hot'}"
                self.paired(name, lambda v, r: [self.executable(v, "perf_runtime"), "--mode", "score", "-i", path, "-n", str(runs), "--threads", "1", *(["--timed"] if timed else [])],
                            lambda out, err, v, r: runtime_metrics(name, [json.loads(line) for line in out.splitlines() if line]))

    def cli(self):
        for label in ("simple", "complex"):
            for single in (True, False):
                name = f"cli-win-{label}-{'single' if single else 'auto'}"
                def parse(out, err, v, r):
                    wins = re.search(r"胜率:.*?\((\d+)/(\d+)\)", out)
                    assert wins and int(wins[2]) == 1_000_000, out
                    # 普通 CLI 不输出高精度墙钟，吞吐由完整进程墙钟另行保留。
                    return [metric(name, 0, signature(wins.groups()), int(wins[2]), wins=int(wins[1]), use_process_wall=True)]
                self.paired(name, lambda v, r: [self.executable(v, "tswn-cli"), "bench", "win-rate", "-f", self.inputs / f"{label}.txt", "-n", "1000000", *(["-s"] if single else [])], parse)
        for subcommand, extra in (("auto", ["-f", "docs/perf/score/mario.txt"]), ("group-win-rate", ["-l", "mario", "--against", "luigi", "--against", "peach"])):
            name = f"cli-{subcommand}"
            def parse(out, err, v, r):
                values = re.findall(r"\((\d+)/(\d+)\)", out)
                assert values, out
                return [metric(name, 0, signature(values), use_process_wall=True)]
            self.paired(name, lambda v, r: [self.executable(v, "tswn-cli"), "bench", subcommand, *extra, "-n", "13000", "-s"], parse)
        for pair in (False, True):
            name = "cli-pair" if pair else "cli-batch-rate"
            extra = ["--teammate-list", self.inputs / "teammates4.txt", "--head", "3"] if pair else []
            self.paired(name, lambda v, r: [self.executable(v, "tswn-cli"), "bench", "pair" if pair else "batch-rate", "-p", self.inputs / "players8.txt", "-l", TARGET2, *extra, "-n", "1000", "--keep-rq"],
                        lambda out, err, v, r: [metric(name, 0, signature(re.sub(r"\t?用时: [^\n]*", "", out)), use_process_wall=True)])

    def openbox(self):
        for label, players, target in (("cqp", PLAYERS, TARGET1), ("derived-cqd32", self.double_lines, TARGET2)):
            for count in (100, 1000, 10000):
                name = f"openbox-{label}-{count}"
                def parse(out, err, v, r):
                    values = re.findall(r"t=([0-9.]+)s rss_kb=(\d+) progress=(\d+)/(\d+)", err)
                    assert values and 'done=Ok(' in err, err
                    wall, rss, done, total = values[-1]
                    assert done == total and int(total) > 0, err
                    return [metric(name, float(wall), signature((done, total)), int(total) * count, rss_kib=int(rss), matchups=int(total))]
                self.paired(name, lambda v, r: [self.executable(v, "openbox_mem_probe"), "--players", players, "--targets", target, "--limit", "all", "--target-limit", "all", "--count", str(count), "--threads", "0", "--report-ms", "3600000"], parse)
        for count in (100, 1000, 10000):
            name = f"openbox-pair-{count}"
            player = PLAYERS if count == 100 else str(self.inputs / "players8.txt")
            teammate = TEAMMATES if count == 100 else str(self.inputs / "teammates4.txt")
            self.pair_probe(name, player, teammate, count)
        self.pair_probe("openbox-pair-weighted", str(self.inputs / "players8.txt"), "crates/tswn_openbox_backend/assets/teammates/teammate_fz.toml", 100, ["--teammate-factored"])
        self.pair_probe("openbox-pair-live", str(self.inputs / "players8.txt"), str(self.inputs / "teammates4.txt"), 1000, ["--live"])

    def pair_probe(self, name, players, teammates, count, extra=()):
        def parse(out, err, v, r):
            found = re.search(r"(?:probe|live) elapsed_s=([0-9.]+)", err)
            assert found, err
            progress = re.search(r"progress_last=(\d+) progress_total=(\d+)", err)
            if progress:
                assert progress[1] == progress[2] and int(progress[1]) > 0, err
            elif "--live" not in extra:
                raise AssertionError(err)
            return [metric(name, float(found[1]), signature(sorted(out.splitlines()) if "--live" in extra else out), stdout_sha256=hashlib.sha256(out.encode()).hexdigest())]
        self.paired(name, lambda v, r: [self.executable(v, "openbox_pair_probe"), "--players", players, "--teammates", teammates, "--targets", TARGET2, "--count", str(count), "--threads", "auto", "--head", "5", *extra], parse)

    def micro(self):
        name = "core-micro"
        def parse(out, err, v, r):
            rows = [json.loads(line) for line in out.splitlines() if line]
            assert rows, out
            return [metric(name + "/" + row["case"] + "/" + row["workload"] + "/" + str(row.get("include_icons", row.get("profile", {}).get("name", "default"))), row["elapsed_seconds"], signature({k: value for k, value in row.items() if k not in ("elapsed_seconds", "ns_per_op", "ops_per_second")}), row.get("ops"), payload=row) for row in rows]
        self.paired(name, lambda v, r: [self.executable(v, "perf_maintenance"), "--encoder-iters", "2000", "--session-iters", "2000", "--max-rounds", "20000"], parse)

    def bindings(self):
        for icons in (False, True):
            self.binding_pair(icons)

    def binding_pair(self, icons):
        name = "python-wasm-session-" + ("icons" if icons else "plain")
        def command(v, r):
            result = self.raw / f"{name}--{v}--r{r}.samples.json"
            return [sys.executable, __file__, "--bench-dir", self.base, "--binding-worker", v, "--worker-output", result, *(["--icons"] if icons else [])]
        def parse(out, err, v, r):
            data = load(self.raw / f"{name}--{v}--r{r}.samples.json")
            metrics = []
            for record in data["records"]:
                for backend in ("python", "wasm"):
                    runs = record[f"{backend}_runs"]
                    sig = signature([(x["frames"], x["winner_ids"], x["rounds_advanced"]) for x in runs])
                    for item in ("session_create_ms", "next_frame_wall_ms", "total_session_ms"):
                        for quantile in ("p50", "p95"):
                            metrics.append(metric(f"{name}/{backend}/{record['fixture']}/{item}/{quantile}", record[backend][item][quantile] / 1000, sig, len(runs)))
            return metrics
        self.paired(name, command, parse)

    def web(self):
        name = "web-streaming"
        def output(v, r):
            return self.raw / f"{name}--{v}--r{r}"
        def parse(out, err, v, r):
            data = load(output(v, r) / "baseline.json")
            result = []
            for fixture in FIXTURES:
                runs = [x for x in data["runs"] if x["fixture"] == fixture]
                assert len(runs) == 5 and all(not x["error"] for x in runs), fixture
                counts = [(x["frames_received"], x["chunks_rendered"]) for x in runs]
                assert len(set(counts)) == 1, counts
                for key in ("ttis_ms", "ttfe_ms", "frame_pull_p95_ms"):
                    result.append(metric(f"{name}/{fixture}/{key}", statistics.median(x[key] for x in runs) / 1000, signature(counts), len(runs)))
            return result
        self.paired(name, lambda v, r: ["node", ROOT / "scripts/benchmark_web_streaming.mjs"], parse,
                    lambda v, r: {"TSWN_BENCH_WEB_RUNS": "5", "TSWN_BENCH_WEB_PACKAGE": str(self.base / v / "wasm-web"), "TSWN_BENCH_WEB_OUTPUT": str(output(v, r))})

    def pwp(self):
        name = "pwp-generate-validate"
        def dataset(v, r):
            return self.fresh_output("pwp-data", v, r, name)
        def parse(out, err, v, r):
            data = load(self.raw / f"{name}--{v}--r{r}.report.json")
            parquet = {str(p.relative_to(dataset(v, r))): digest(p) for p in sorted(dataset(v, r).glob("shard-*/*.parquet"))}
            save(self.raw / f"{name}--{v}--r{r}.parquet-hashes.json", parquet)
            sig = signature(parquet)
            return [metric(f"pwp/{phase}", data[phase]["wall_seconds"], sig, data["dataset"]["battles"], peak_rss_bytes=data[phase]["peak_rss_bytes"], samples=data["dataset"]["samples"], file_bytes=data["dataset"]["file_bytes"]) for phase in ("generate", "validate")]
        self.paired(name, lambda v, r: [self.executable(v, "tswn-pwp"), "bench", "--input", "docs/perf/fixed_cases_30", "--games-per-matchup", "100", "--seed", "maintenance-bench-20261024", "--out", dataset(v, r), "--battles-per-shard", "250", "--threads", "0", "--interval", "0.02", "--json-out", self.raw / f"{name}--{v}--r{r}.report.json"], parse)

    def pwp_encode(self):
        # 单独的小数据集避免把3000局的稠密padding物化为接近100GiB文件。
        source = self.fresh_output("encoder-source", "current", 0, "prepare-encoder-source")
        fixture_dir = self.inputs / "encoder-cases"
        fixture_dir.mkdir(exist_ok=True)
        for fixture in FIXTURES:
            shutil.copyfile(ROOT / "crates/tswn_test/cases/runtime_stress" / f"{fixture}.txt", fixture_dir / f"{fixture}.txt")
        calibration = self.base / "encoder-calibration.json"
        manifest = self.base / "encoder-manifest.json"
        self.invoke("prepare-encoder-source", "current", 0,
                    [self.executable("current", "tswn-pwp"), "generate", "--input", fixture_dir, "--games-per-matchup", "2", "--samples-per-game", "8", "--battles-per-shard", "8", "--threads", "1", "--seed", "maintenance-encoder", "--out", source], lambda out, err: [])
        generation = load(self.raw / "pwp-generate-validate--current--r0.json")["command"]
        calibration_dataset = generation[generation.index("--out") + 1]
        self.invoke("prepare-encoder-calibration", "current", 0,
                    [self.executable("current", "tswn-pwp"), "calibrate", "--out", calibration_dataset, "--json-out", calibration], lambda out, err: [])
        self.invoke("prepare-encoder-manifest", "current", 0,
                    [self.executable("current", "tswn-pwp"), "build-manifest", "--calibration", calibration, "--out", manifest], lambda out, err: [])
        name = "pwp-encode-b8"
        def output(v, r):
            return self.fresh_output("encoded", v, r, name)
        def parse(out, err, v, r):
            files = {str(p.relative_to(output(v, r))): digest(p) for p in sorted(output(v, r).rglob("*")) if p.is_file()}
            assert files, "编码输出为空"
            save(self.raw / f"{name}--{v}--r{r}.parquet-hashes.json", files)
            return [metric(name, 0, signature(files), use_process_wall=True, file_bytes=sum(p.stat().st_size for p in output(v, r).rglob("*") if p.is_file()), files=len(files))]
        self.paired(name, lambda v, r: [self.executable(v, "tswn-pwp"), "encode", "--dataset", source, "--manifest", manifest, "--out", output(v, r), "--split", "train", "--batch-size", "8"], parse)

    def reference(self):
        # 官方JS不是Rust维护前后A/B，只运行一遍各模式，留作同机横向参考。
        for runtime in ("node", "bun"):
            for mode in ("fixed", "win-rate", "score", "matrix"):
                name = f"reference-{runtime}-{mode}"
                report = self.raw / f"{name}.report.json"
                arguments = {
                    "fixed": ["--case-dir", "docs/perf/fixed_cases_30", "--count", "13000", "--workers", "1"],
                    "win-rate": ["--input", self.inputs / "simple.txt", "--count", "100000", "--workers", "1"],
                    "score": ["--input", "docs/perf/score/mario.txt", "--count", "13000", "--workers", "1"],
                    "matrix": ["--input", self.inputs / "players8.txt", "--targets", TARGET1, "--count", "1000", "--workers", "0"],
                }[mode]
                self.invoke(name, "current", 0, [runtime, "scripts/benchmark_md5_runtime.cjs", "--mode", mode, "--md5", "md5.js", *arguments, "--out", report], lambda out, err: [])

    def summarize(self):
        all_records = [load(p) for p in sorted(self.raw.glob("*.json")) if not any(x in p.name for x in (".samples.", ".report.", ".parquet-hashes."))]
        all_records = [r for r in all_records if isinstance(r, dict) and "metrics" in r and not r["warmup"]]
        groups = {}
        for record in all_records:
            for row in record["metrics"]:
                row = dict(row)
                if row.pop("use_process_wall", False):
                    row["seconds"] = record["process_seconds"]
                groups.setdefault(row["key"], {}).setdefault(record["variant"], []).append({**row, "round": record["round"]})
        rows = []
        for key, variants in sorted(groups.items()):
            assert set(variants) == {"baseline", "current"}, key
            a = sorted(variants["baseline"], key=lambda x: x["round"])
            b = sorted(variants["current"], key=lambda x: x["round"])
            assert len(a) == len(b) == self.args.rounds, (key, len(a), len(b))
            old, new = [r["seconds"] for r in a], [r["seconds"] for r in b]
            before, after = statistics.median(old), statistics.median(new)
            paired = [(y / x - 1) * 100 for x, y in zip(old, new)]
            rows.append({"key": key, "baseline_median_seconds": before, "current_median_seconds": after,
                         "change_percent": (after / before - 1) * 100, "paired_median_change_percent": statistics.median(paired),
                         "baseline_seconds": old, "current_seconds": new, "paired_changes_percent": paired,
                         "baseline_mad_percent": statistics.median(abs(x - before) for x in old) / before * 100,
                         "current_mad_percent": statistics.median(abs(x - after) for x in new) / after * 100,
                         "signatures_equal": len({r["signature"] for r in a + b}) == 1,
                         "baseline_details": a[0], "current_details": b[0]})
        result = {"environment": self.environment, "rounds": self.args.rounds, "warmups_per_configuration": 1,
                  "method": "逐配置交替A/B，r1旧→新、r2新→旧；变化=(新中位数/旧中位数-1)*100，负值更快；小于1.5%不视为收益",
                  "input_hashes": load(self.base / "input-hashes.json"), "metrics": rows}
        save(self.base / "summary.json", result)
        print(f"汇总 {len(all_records)} 次正式调用，{len(rows)} 项指标：{self.base / 'summary.json'}")
        for row in rows:
            if "/case/" not in row["key"]:
                print(f"{row['key']}: {row['baseline_median_seconds']:.6f} → {row['current_median_seconds']:.6f}s {row['change_percent']:+.2f}%，一致={row['signatures_equal']}")


def binding_worker(args):
    # 每版每轮是新进程，复用原计时实现，只替换计时外的构建/导入准备与代码身份。
    sys.path.insert(0, str(ROOT / "scripts"))
    import benchmark_py_battle_session as benchmark
    base = args.bench_dir.resolve()
    environment = load(base / "environment.json")
    variant = args.binding_worker
    original_output = benchmark.command_output
    def command_output(*command):
        if command == ("git", "rev-parse", "HEAD"):
            return environment[variant]
        return original_output(*command)
    benchmark.OPTIONS = {**benchmark.OPTIONS, "include_icons": args.icons}
    benchmark.command_output = command_output
    benchmark.prepare_import_tree = lambda release: sys.path.insert(0, str(base / variant / "python"))
    sys.argv = [str(ROOT / "scripts/benchmark_py_battle_session.py"), "--sessions", "100", "--warmup", "10", "--output", str(args.worker_output), "--wasm-package", str(base / variant / "wasm-node/tswn_wasm.js")]
    benchmark.main()
    data = load(args.worker_output)
    data["environment"].update({"driver_revision": environment["current"], "build": environment["build_environment"], "prebuilt_artifact": str(base / variant / "python/tswn_py/tswn_py.pyd")})
    save(args.worker_output, data)


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bench-dir", type=Path, default=ROOT / "target/maintenance-bench-20261024")
    parser.add_argument("--rounds", type=int, default=5)
    parser.add_argument("--phase", choices=("runtime", "cli", "openbox", "micro", "bindings", "web", "pwp", "pwp_encode", "reference", "all", "summary"), default="all")
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--icons", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--binding-worker", choices=("baseline", "current"), help=argparse.SUPPRESS)
    parser.add_argument("--worker-output", type=Path, help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.binding_worker:
        binding_worker(args)
        return
    if args.rounds < 5:
        parser.error("正式对照至少5轮")
    runner = Runner(args)
    runner.prepare_inputs()
    phases = ("micro", "runtime", "cli", "bindings", "openbox", "pwp", "pwp_encode", "web", "reference") if args.phase == "all" else (args.phase,)
    for phase in phases:
        if phase == "summary":
            runner.summarize()
        else:
            getattr(runner, phase)()
    if args.phase == "all":
        runner.summarize()


if __name__ == "__main__":
    main()
