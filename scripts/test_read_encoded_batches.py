"""编码包消费端的真实文件校验与数组读取回归，不约束目录或源码写法。"""

import hashlib
import importlib.util
import json
from pathlib import Path
import struct
import tempfile
import unittest

from read_encoded_batches import iter_encoded_batches, load_export


def write_json(path, value):
    path.write_text(json.dumps(value), encoding="utf-8")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def payload(directory, name, data):
    (directory / name).write_bytes(data)
    return {"file": name, "byte_length": len(data), "sha256": digest(directory / name)}


def tensor(directory, name, dtype, shape, data):
    return {"name": name, "dtype": dtype, "shape": shape, **payload(directory, name + ".bin", data)}


class EncodedBatchTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.addCleanup(self.temp.cleanup)
        encoder = payload(self.root, "encoder-manifest.json", b'{"contract_digest":"test-contract"}')
        hashes, tensor_bytes = [], 0
        for index, size in enumerate((2, 1)):
            directory = self.root / f"batch-{index:06d}"
            directory.mkdir()
            features = [
                tensor(directory, "global_num", "f32", [size, 1], struct.pack(f"<{size}f", *([1.25] * size))),
                tensor(directory, "team_mask", "u8", [size, 2], bytes([1, 1] * size)),
            ]
            exact = [tensor(directory, "extra_bits", "u32", [size, 1, 2], struct.pack(f"<{size * 2}I", *([0xFFFFFFFF, 0xFFFFFFFF] * size)))]
            labels = [
                tensor(directory, "winner_team_index", "i32", [size], struct.pack(f"<{size}i", *([1] * size))),
                tensor(directory, "label_mask", "u8", [size], bytes([1] * size)),
            ]
            audit = payload(directory, "rows.json", json.dumps([
                {"source_shard": index, "source_row": slot, "battle_id": index * 2 + slot, "frame": None}
                for slot in range(size)
            ]).encode())
            write_json(directory / "batch-manifest.json", {
                "format": "tswn-core/encoded-batch", "format_version": 2, "batch_index": index,
                "batch_size": size, "encoder_manifest_sha256": "test-contract",
                "features": features, "exact": exact, "labels": labels, "audit": audit,
            })
            hashes.append(digest(directory / "batch-manifest.json"))
            tensor_bytes += sum(t["byte_length"] for t in features + exact + labels)
        write_json(self.root / "manifest.json", {
            "format": "tswn-core/encoded-dataset", "format_version": 2, "batches": 2, "samples": 3,
            "batch_size": 2, "keep_unlabeled": False, "encoder_manifest_sha256": "test-contract",
            "encoder_manifest": encoder, "batch_manifest_sha256": hashes, "tensor_bytes": tensor_bytes,
        })

    def test_tail_and_same_length_payload_corruption(self):
        package = load_export(self.root)
        self.assertEqual([batch["batch_size"] for batch in package["batches"]], [2, 1])
        tensor_file = self.root / "batch-000001/global_num.bin"
        original = tensor_file.read_bytes()
        tensor_file.write_bytes(bytes([original[0] ^ 1]) + original[1:])
        with self.assertRaisesRegex(ValueError, "SHA-256"):
            load_export(self.root)

    def test_metadata_and_encoder_copy_corruption(self):
        path = self.root / "batch-000000/batch-manifest.json"
        original = path.read_bytes()
        batch = json.loads(original)
        batch["batch_size"] = 1
        write_json(path, batch)
        with self.assertRaisesRegex(ValueError, "批清单 SHA-256"):
            load_export(self.root)
        path.write_bytes(original)
        encoder = self.root / "encoder-manifest.json"
        encoder.write_bytes(encoder.read_bytes().replace(b"test-contract", b"bad--contract"))
        with self.assertRaisesRegex(ValueError, "SHA-256"):
            load_export(self.root)

    def test_label_mask_mismatch_even_when_file_digest_is_updated(self):
        directory = self.root / "batch-000000"
        path = directory / "batch-manifest.json"
        batch = json.loads(path.read_text(encoding="utf-8"))
        label = next(tensor for tensor in batch["labels"] if tensor["name"] == "label_mask")
        label.update(payload(directory, label["file"], bytes([0, 1])))
        write_json(path, batch)
        manifest_path = self.root / "manifest.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        manifest["batch_manifest_sha256"][0] = digest(path)
        write_json(manifest_path, manifest)
        with self.assertRaisesRegex(ValueError, "标签与队伍 mask"):
            load_export(self.root)

    @unittest.skipUnless(importlib.util.find_spec("numpy"), "数组读取需要 NumPy")
    def test_numpy_maps_channels_without_audit_or_numeric_bit_conversion(self):
        iterator = iter_encoded_batches(self.root, include_exact=True)
        batch = next(iterator)
        try:
            self.assertEqual(set(batch), {"features", "exact", "labels"})
            self.assertEqual(batch["features"]["global_num"].shape, (2, 1))
            self.assertAlmostEqual(float(batch["features"]["global_num"][0, 0]), 1.25)
            self.assertFalse(batch["features"]["global_num"].flags.writeable)
            self.assertEqual(batch["labels"]["winner_team_index"].tolist(), [1, 1])
            self.assertEqual(batch["exact"]["extra_bits"].tolist(), [[[0xFFFFFFFF, 0xFFFFFFFF]], [[0xFFFFFFFF, 0xFFFFFFFF]]])
        finally:
            iterator.close()
            del batch


if __name__ == "__main__":
    unittest.main()
