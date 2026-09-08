"""只读取状态与标签；默认排除截断对局，不向训练代码暴露审计列。"""

import argparse
import json
import sys
from pathlib import Path


def iter_samples(root: Path, include_unresolved: bool = False):
    import pyarrow.compute as pc
    import pyarrow.parquet as pq

    manifest = json.loads((root / "manifest.json").read_text(encoding="utf-8"))
    if manifest["format_version"] != 1 or manifest["state_schema_version"] != 1:
        raise ValueError("不支持的数据版本")
    battles = len(manifest["cases"]) * manifest["games_per_matchup"]
    size = manifest["battles_per_shard"]
    for index in range((battles + size - 1) // size):
        shard = root / f"shard-{index:06}"
        if not (shard / "complete.json").is_file():
            raise ValueError(f"分片未完成：{shard}")
        table = pq.ParquetFile(shard / "samples.parquet")
        for batch in table.iter_batches(columns=["state", "winner_team_index"], batch_size=128):
            if not include_unresolved:
                batch = batch.filter(pc.is_valid(batch.column("winner_team_index")))
            yield from batch.to_pylist()


def main():
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dataset", type=Path)
    parser.add_argument("--include-unresolved", action="store_true")
    args = parser.parse_args()
    count = sum(1 for _ in iter_samples(args.dataset, args.include_unresolved))
    print(f"可读取样本：{count}")


if __name__ == "__main__":
    main()
