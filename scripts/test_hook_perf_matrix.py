"""验证恢复留出集确定性、范围和保守去重；不运行战斗。"""
import json
from pathlib import Path
import tempfile
import unittest

from generate_hook_perf_matrix import BOSSES, build_cases, generate
from generate_perf_matrix import roster_signature


class HookMatrixTests(unittest.TestCase):
    def fixture(self, root):
        repo = root / "repo"
        prior = root / "prior"
        for folder, count in ((repo / "docs/perf/fixed_cases_30", 30), (prior / "expanded", 85), (prior / "training", 48)):
            folder.mkdir(parents=True)
            for index in range(count):
                (folder / f"{index}.txt").write_text(f"old-{folder.name}-{index}\nother\n")
        return repo, prior

    def test_deterministic(self):
        self.assertEqual(build_cases(), build_cases())

    def test_scope_and_layout(self):
        cases = build_cases()
        self.assertEqual(len(cases), 32)
        self.assertEqual({case["boss"] for case in cases if case["boss"]}, set(BOSSES))
        self.assertEqual(sum(all(size == 1 for size in case["group_sizes"]) for case in cases), 20)
        self.assertTrue(all(b"+" not in case["data"] for case in cases))

    def test_unique_rosters(self):
        self.assertEqual(len({roster_signature(case["data"]) for case in build_cases()}), 32)

    def test_written_manifest_and_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repo, prior = self.fixture(root)
            out = root / "out"
            generate(repo, prior, out)
            manifest = json.loads((out / "manifest.json").read_text())
            self.assertEqual(manifest["cases"], 32)
            self.assertEqual(manifest["prior_cases_checked"], 163)
            for case in build_cases():
                self.assertEqual((out / "inputs" / case["file"]).read_bytes(), case["data"])
            with self.assertRaises(ValueError):
                generate(repo, prior, out)

    def test_overlap_rejected_before_write(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repo, prior = self.fixture(root)
            (prior / "expanded/0.txt").write_bytes(build_cases()[0]["data"])
            with self.assertRaises(ValueError):
                generate(repo, prior, root / "out")
            self.assertFalse((root / "out").exists())

    def test_missing_prior_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaises(ValueError):
                generate(root, root / "missing", root / "out")
            self.assertFalse((root / "out").exists())


if __name__ == "__main__":
    unittest.main()
