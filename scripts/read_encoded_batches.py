"""只读校验训练编码包，并按批映射 NumPy 数组；不在 Python 重做编码。"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
import struct


DTYPE_SIZE = {"f32": 4, "i32": 4, "u32": 4, "u8": 1}
NUMPY_DTYPE = {"f32": "<f4", "i32": "<i4", "u32": "<u4", "u8": "u1"}


def _read_root(root: Path) -> dict:
    manifest = json.loads((root / "manifest.json").read_text(encoding="utf-8"))
    if manifest.get("format") != "tswn-core/encoded-dataset" or manifest.get("format_version") != 2:
        raise ValueError("不支持的编码包版本，请用当前 encode 重新导出")
    encoder_path = _check_file(root, manifest["encoder_manifest"])
    encoder = json.loads(encoder_path.read_text(encoding="utf-8"))
    if encoder.get("contract_digest") != manifest["encoder_manifest_sha256"]:
        raise ValueError("encoder manifest 摘要不一致")
    if manifest["batches"] <= 0 or manifest["samples"] <= 0 or manifest["batch_size"] <= 0:
        raise ValueError("编码包样本数和批大小必须为正")
    if len(manifest["batch_manifest_sha256"]) != manifest["batches"]:
        raise ValueError("批清单摘要数量不匹配")
    return manifest


def _payload_path(directory: Path, entry: dict) -> Path:
    name = entry["file"]
    if not name or name in (".", "..") or any(char in name for char in ("/", "\\", ":")):
        raise ValueError(f"无效文件名：{name}")
    path = directory / name
    if path.is_symlink():
        raise ValueError(f"不读取符号链接：{path}")
    return path


def _check_file(directory: Path, entry: dict) -> Path:
    path = _payload_path(directory, entry)
    if path.stat().st_size != entry["byte_length"]:
        raise ValueError(f"{path}: 文件长度不一致")
    if _file_digest(path) != entry["sha256"]:
        raise ValueError(f"{path}: 文件 SHA-256 不一致")
    return path


def _file_digest(path: Path) -> str:
    with path.open("rb") as payload:
        digest = hashlib.sha256()
        for chunk in iter(lambda: payload.read(64 * 1024), b""):
            digest.update(chunk)
        return digest.hexdigest()


def _iter_validated_batches(root: Path, manifest: dict):
    samples = 0
    tensor_bytes = 0
    layout = None
    for index in range(manifest["batches"]):
        directory = root / f"batch-{index:06d}"
        if directory.is_symlink():
            raise ValueError(f"不读取符号链接：{directory}")
        batch_path = _payload_path(directory, {"file": "batch-manifest.json"})
        if _file_digest(batch_path) != manifest["batch_manifest_sha256"][index]:
            raise ValueError(f"{directory}: 批清单 SHA-256 不一致")
        batch = json.loads(batch_path.read_text(encoding="utf-8"))
        if batch.get("format") != "tswn-core/encoded-batch" or batch.get("format_version") != 2:
            raise ValueError(f"{directory}: 不支持的批格式")
        if batch["batch_index"] != index or batch["encoder_manifest_sha256"] != manifest["encoder_manifest_sha256"]:
            raise ValueError(f"{directory}: 批编号或 encoder 摘要不一致")
        size = batch["batch_size"]
        if not 0 < size <= manifest["batch_size"] or (index + 1 < manifest["batches"] and size != manifest["batch_size"]):
            raise ValueError(f"{directory}: 非尾批未填满或批大小无效")
        names, files, current_layout = set(), set(), []
        for channel in ("features", "exact", "labels"):
            for tensor in batch[channel]:
                shape = tensor["shape"]
                if not shape or shape[0] != size or any(not isinstance(axis, int) or axis <= 0 for axis in shape):
                    raise ValueError(f"{directory}/{tensor['name']}: shape 无效")
                if tensor["name"] in names or tensor["file"] in files:
                    raise ValueError(f"{directory}: 张量名或文件重复")
                names.add(tensor["name"])
                files.add(tensor["file"])
                if tensor["dtype"] not in DTYPE_SIZE or math.prod(shape) * DTYPE_SIZE[tensor["dtype"]] != tensor["byte_length"]:
                    raise ValueError(f"{directory}/{tensor['name']}: dtype/shape 与字节数不一致")
                _check_file(directory, tensor)
                tensor_bytes += tensor["byte_length"]
                current_layout.append((channel, tensor["name"], tensor["dtype"], shape[1:]))
        if layout is not None and current_layout != layout:
            raise ValueError(f"{directory}: 批间张量布局不一致")
        layout = current_layout
        labels = {tensor["name"]: tensor for tensor in batch["labels"]}
        if set(labels) != {"winner_team_index", "label_mask"}:
            raise ValueError(f"{directory}: 缺少监督标签")
        if labels["winner_team_index"]["dtype"] != "i32" or labels["label_mask"]["dtype"] != "u8":
            raise ValueError(f"{directory}: 标签 dtype 无效")
        if any(tensor["shape"] != [size] for tensor in labels.values()):
            raise ValueError(f"{directory}: 标签 shape 无效")
        winners = struct.unpack(f"<{size}i", _payload_path(directory, labels["winner_team_index"]).read_bytes())
        masks = _payload_path(directory, labels["label_mask"]).read_bytes()
        team_tensor = next((tensor for tensor in batch["features"] if tensor["name"] == "team_mask"), None)
        if team_tensor is None or team_tensor["dtype"] != "u8" or len(team_tensor["shape"]) != 2:
            raise ValueError(f"{directory}: 缺少 team_mask")
        teams = _payload_path(directory, team_tensor).read_bytes()
        team_count = team_tensor["shape"][1]
        for slot, (winner, mask) in enumerate(zip(winners, masks)):
            if not ((mask == 0 and winner == -1) or (mask == 1 and 0 <= winner < team_count and teams[slot * team_count + winner] == 1)):
                raise ValueError(f"{directory}: 标签与队伍 mask 不一致")
            if not manifest["keep_unlabeled"] and mask == 0:
                raise ValueError(f"{directory}: 训练包包含空标签")
        audit_path = _check_file(directory, batch["audit"])
        if batch["audit"]["file"] in files:
            raise ValueError(f"{directory}: 审计与张量文件重叠")
        audit = json.loads(audit_path.read_text(encoding="utf-8"))
        if not isinstance(audit, list) or len(audit) != size:
            raise ValueError(f"{directory}: 源行关联数不匹配")
        samples += size
        yield directory, batch
    if samples != manifest["samples"]:
        raise ValueError("批次样本数与总样本数不一致")
    if tensor_bytes != manifest["tensor_bytes"]:
        raise ValueError("实际张量字节数与预算不一致")


def load_export(root: Path) -> dict:
    root = Path(root)
    manifest = _read_root(root)
    batches = [batch for _, batch in _iter_validated_batches(root, manifest)]
    return {"manifest": manifest, "batches": batches}


def iter_encoded_batches(root: Path, *, include_exact: bool = False):
    """逐批返回只读 features/labels；精确旁路需显式选择，审计列不进入数组字典。"""
    import numpy as np

    root = Path(root)
    manifest = _read_root(root)
    for directory, batch in _iter_validated_batches(root, manifest):
        channels = ("features", "labels", "exact") if include_exact else ("features", "labels")
        yield {
            channel: {
                tensor["name"]: np.memmap(
                    _payload_path(directory, tensor), dtype=NUMPY_DTYPE[tensor["dtype"]],
                    mode="r", shape=tuple(tensor["shape"]), order="C",
                )
                for tensor in batch[channel]
            }
            for channel in channels
        }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    args = parser.parse_args()
    result = load_export(args.root)
    print(json.dumps({"samples": result["manifest"]["samples"], "batches": len(result["batches"])}, ensure_ascii=False))


if __name__ == "__main__":
    main()
