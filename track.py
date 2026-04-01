#!/usr/bin/env python3
"""
统一入口:
  python track.py test  ...
  python track.py miner ...
"""

import subprocess
import sys
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parent
TOOLS = {
    "test": PROJECT_ROOT / "track_test.py",
    "miner": PROJECT_ROOT / "track_case_miner.py",
}


def main():
    if len(sys.argv) < 2 or sys.argv[1] not in TOOLS:
        print("用法: python track.py <test|miner> [args...]")
        sys.exit(1)

    target = sys.argv[1]
    script = TOOLS[target]
    cmd = [sys.executable, str(script), *sys.argv[2:]]
    result = subprocess.run(cmd, cwd=str(PROJECT_ROOT))
    sys.exit(result.returncode)


if __name__ == "__main__":
    main()
