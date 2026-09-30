"""验证聚合包包含 Openbox GUI 与 CLI，且缺少 CLI 时不会静默漏包。"""

from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import build_all


class OpenboxPackagingTests(unittest.TestCase):
    def test_gui_and_cli_are_both_in_manifest_readme_and_zip(self):
        for system, suffix in [("Windows", ".exe"), ("Linux", "")]:
            with self.subTest(system=system), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                artifacts = root / "artifacts"
                artifacts.mkdir()
                (artifacts / f"tswn_openbox{suffix}").write_bytes(b"gui")
                (artifacts / f"openbox-cli{suffix}").write_bytes(b"cli")
                bundle = root / "bundle"
                destination = bundle / "openbox"
                with (
                    patch.object(build_all.platform, "system", return_value=system),
                    patch.object(build_all, "cargo_profile_dir", return_value=artifacts),
                    patch.object(build_all, "run"),
                    patch.object(build_all, "collect_existing_linux_openbox_artifacts"),
                ):
                    gui, cli, support = build_all.build_openbox(destination, True, None, "", [])
                build_all.write_openbox_readme(destination, gui, cli, support)
                build_all.write_openbox_manifest(destination, True, None, gui, cli, support)
                for name in ["README.txt", "MANIFEST.txt"]:
                    text = (destination / name).read_text(encoding="utf-8")
                    self.assertIn(gui.name, text)
                    self.assertIn(cli.name, text)
                archive = build_all.make_zip(bundle, root / "all.zip")
                with zipfile.ZipFile(archive) as zipped:
                    for binary, expected in [(gui, b"gui"), (cli, b"cli")]:
                        member = binary.relative_to(root).as_posix()
                        self.assertEqual(zipped.read(member), expected)

    def test_missing_cli_rejects_incomplete_package(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "tswn_openbox.exe").write_bytes(b"gui")
            with (
                patch.object(build_all.platform, "system", return_value="Windows"),
                patch.object(build_all, "cargo_profile_dir", return_value=root),
                patch.object(build_all, "run"),
                self.assertRaisesRegex(FileNotFoundError, "openbox-cli"),
            ):
                build_all.build_openbox(root / "bundle", True, None, "", [])


if __name__ == "__main__":
    unittest.main()
