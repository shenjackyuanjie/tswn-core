"""Build and verify the CLI process streaming/output contract."""
from pathlib import Path
import json
import os
import subprocess

ROOT = Path(__file__).resolve().parent.parent


def main() -> None:
    subprocess.run(["cargo", "build", "-p", "tswn_core", "--bin", "tswn-cli"], cwd=ROOT, check=True)
    executable = ROOT / "target" / "debug" / ("tswn-cli.exe" if os.name == "nt" else "tswn-cli")
    def run(*args: str, raw: str | None = None) -> subprocess.CompletedProcess[str]:
        return subprocess.run([str(executable), *args], input=raw, capture_output=True,
                              encoding="utf-8", cwd=ROOT)

    fixture_dir = ROOT / "crates" / "tswn_test" / "cases" / "runtime_stress"
    for fixture in ["1v1-0f92cb76cc37fdc5.txt", "2v2-554f4128af707167.txt",
                    "ffa_8-16d11de1ebe1df41.txt", "3v3v3-0ace5df17b84e26a.txt"]:
        for limit in [1, 20_000]:
            process = run("fight", "--jsonl", "--max-rounds", str(limit), "-f", str(fixture_dir / fixture))
            assert process.returncode == 0, process.stderr
            assert not process.stderr, process.stderr
            events = [json.loads(line) for line in process.stdout.splitlines()]
            assert events[0]["type"] == "initial" and events[-1]["type"] == "result"
            assert all(event["type"] == "frame" for event in events[1:-1])
            result = events[-1]["data"]
            assert result["frames_emitted"] == len(events) - 2
            assert result["rounds_advanced"] <= limit
            assert result["status"] in ["finished", "truncated"]
            assert [event["data"]["frame_index"] for event in events[1:-1]] == list(range(len(events) - 2))

    for args in [("raw", "-r", "a\\n\\nb"), ("diff", "-r", "a\\n\\nb"),
                 ("fight", "--out-raw", "-r", "a\\n\\nb"),
                 ("fight", "--jsonl", "--max-rounds", "0", "-r", "a\\n\\nb"),
                 ("fight", "--jsonl", "-r", "")]:
        process = run(*args)
        assert process.returncode != 0 and process.stderr and not process.stdout, args
    piped = run("fight", "--jsonl", "--max-rounds", "1", raw="a\n\nb")
    assert piped.returncode == 0 and json.loads(piped.stdout.splitlines()[0])["type"] == "initial"
    human = run("fight", "--max-rounds", "1", "-r", "a\\n\\nb")
    assert human.returncode == 0 and "对局截断" in human.stdout
    diagnostic = run("runtime", "diff", "-r", "a\\n\\nb")
    assert diagnostic.returncode == 0 and diagnostic.stdout and "欢迎" not in diagnostic.stdout
    print("OK: CLI JSONL, stdin, human output, diagnostics and rejected legacy commands")


if __name__ == "__main__":
    main()
