"""Exercise archive manifests and non-mutating dry runs using fake build inputs."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parent.parent


class PackagingTests(unittest.TestCase):
    def test_platform_archives_and_dry_run(self):
        for os in ("linux", "macos", "windows"):
            for arch in ("x86_64", "aarch64"):
                with self.subTest(os=os, arch=arch), tempfile.TemporaryDirectory() as temp:
                    root = Path(temp)
                    binary = root / "build"
                    binary.mkdir()
                    suffix = ".exe" if os == "windows" else ""
                    library = {"linux": "libgdguard_gdextension.so", "macos": "libgdguard_gdextension.dylib", "windows": "gdguard_gdextension.dll"}[os]
                    for name in ("gdguard" + suffix, "gdscript-formatter" + suffix, library):
                        (binary / name).write_bytes(b"test build input")
                    output = root / "archives"
                    args = [sys.executable, str(ROOT / ".github/package_release.py"), "--binary-dir", str(binary), "--platform", f"{os}-{arch}", "--output", str(output)]
                    dry = subprocess.run(args + ["--dry-run"], check=True, capture_output=True)
                    self.assertFalse(output.exists())
                    manifest = json.loads(dry.stdout)["archives"]
                    self.assertEqual(len(manifest), 4)
                    subprocess.run(args, check=True, capture_output=True)
                    for name, entries in manifest.items():
                        with zipfile.ZipFile(output / name) as archive:
                            self.assertIsNone(archive.testzip())
                            self.assertEqual(archive.namelist(), entries)
                            self.assertFalse(any("GDQuest_GDScript_formatter" in entry for entry in entries))
                            if name.startswith("gdguard-rules-"):
                                self.assertIn("gdguard.d.ts", entries)
                                self.assertTrue(any(entry.endswith(".js") for entry in entries))
                                self.assertTrue(any(entry.endswith(".yml") for entry in entries))
                            if name.startswith("gdguard-native-"):
                                self.assertIn(f"addons/gdguard_standalone/bin/{library}", entries)
                            if name.startswith("gdguard-") and not any(name.startswith(f"gdguard-{kind}-") for kind in ("rules", "addon", "native")):
                                self.assertIn("gdguard" + suffix, entries)
                                self.assertIn("gdscript-formatter" + suffix, entries)

    def test_missing_binary_fails_before_creating_output(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            result = subprocess.run([sys.executable, str(ROOT / ".github/package_release.py"), "--binary-dir", str(root), "--output", str(root / "output"), "--only", "cli"], capture_output=True)
            self.assertEqual(result.returncode, 2)
            self.assertIn(b"missing release input", result.stderr)
            self.assertFalse((root / "output").exists())


if __name__ == "__main__":
    unittest.main()
