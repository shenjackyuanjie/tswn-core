"""只读校验 FeatureEncoder 的小端张量编码包。"""

from __future__ import annotations

import argparse
import json
from pathlib import Path


DTYPE_SIZE = {"f32": 4, "i32": 4, "u32": 4, "u8": 1}


def load_export(root: Path) -> dict:
    manifest = json.loads((root / "manifest.json").read_text(encoding="utf-8"))
    batches = []
    for index in range(manifest["batches"]):
        batch_dir = root / f"batch-{index:06d}"
        batch = json.loads((batch_dir / "batch-manifest.json").read_text(encoding="utf-8"))
        if batch["encoder_manifest_sha256"] != manifest["encoder_manifest_sha256"]:
            raise ValueError(f"{batch_dir}: encoder manifest 摘要不一致")
        for tensor in batch["tensors"]:
            shape = tensor["shape"]
            expected = 1
            for axis in shape:
                expected *= axis
            expected *= DTYPE_SIZE[tensor["dtype"]]
            if expected != tensor["byte_length"]:
                raise ValueError(f"{batch_dir}/{tensor['name']}: shape 与 byte_length 不一致")
            payload = (batch_dir / tensor["file"]).read_bytes()
            if len(payload) != tensor["byte_length"]:
                raise ValueError(f"{batch_dir}/{tensor['name']}: 文件长度不一致")
        batches.append(batch)
    if sum(batch["batch_size"] for batch in batches) != manifest["samples"]:
        raise ValueError("批次样本数与总样本数不一致")
    return {"manifest": manifest, "batches": batches}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("root", type=Path)
    args = parser.parse_args()
    result = load_export(args.root)
    print(json.dumps({"samples": result["manifest"]["samples"], "batches": len(result["batches"])}, ensure_ascii=False))


if __name__ == "__main__":
    main()
