#!/usr/bin/env python3
"""Build and run the native integration and addon import in an isolated project."""
import argparse
import contextlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument("--godot", required=True)
parser.add_argument("--skip-build", action="store_true")
parser.add_argument("--keep-temp", action="store_true")
args = parser.parse_args()
repo = Path(__file__).resolve().parents[2]
if not args.skip_build:
    subprocess.run(["cargo", "build", "--workspace"], cwd=repo, check=True)
system = platform.system()
name = {"Linux": "libgdguard_gdextension.so", "Darwin": "libgdguard_gdextension.dylib", "Windows": "gdguard_gdextension.dll"}[system]
condition = {"Linux": "linux.debug.x86_64", "Darwin": "macos.debug", "Windows": "windows.debug.x86_64"}[system]
cli = repo / "target/debug" / ("gdscript-formatter.exe" if system == "Windows" else "gdscript-formatter")
sample = repo / "tests/e2e/sample_project"
with (contextlib.nullcontext(tempfile.mkdtemp(prefix="gdguard-godot-")) if args.keep_temp else tempfile.TemporaryDirectory(prefix="gdguard-godot-")) as temporary:
    print("Godot test project:", temporary, flush=True)
    root = Path(temporary) / "game"
    shutil.copytree(sample, root, ignore=shutil.ignore_patterns("cache", ".godot"))
    native = root / "addons/gdguard_native"
    native.mkdir(parents=True)
    shutil.copy2(repo / "target/debug" / name, native / name)
    (native / "native.gdextension").write_text(f'[configuration]\nentry_symbol="gdext_rust_init"\ncompatibility_minimum=4.7\n[libraries]\n{condition}="res://addons/gdguard_native/{name}"\n')
    # Load the native library at process startup, as on a normal restarted
    # installation; late discovery/hot-load is not supported by this smoke.
    (root / ".godot").mkdir(exist_ok=True)
    (root / ".godot/extension_list.cfg").write_text("res://addons/gdguard_native/native.gdextension\n")
    shutil.copytree(repo / "addons/gdguard_lint", root / "addons/gdguard_lint")
    with (root / "project.godot").open("a") as config:
        config.write('\n[editor_plugins]\nenabled=PackedStringArray("res://addons/gdguard_lint/plugin.cfg")\n')
    fixtures = []
    for path in sorted(sample.rglob("*.gd")):
        relative = path.relative_to(sample)
        output = subprocess.run([str(cli), "lint", "--no-cache", "--format", "json", str(relative)], cwd=sample, text=True, capture_output=True)
        if output.returncode not in (0, 1):
            raise RuntimeError(output.stderr)
        report = json.loads(output.stdout)
        fixtures.append({"path": relative.as_posix(), "source": path.read_text(), "keys": [f"{issue['rule']}:{issue['range']['start']['byte']}:{issue['range']['end']['byte']}" for issue in report["issues"]]})
    (root / "native_parity.json").write_text(json.dumps(fixtures, ensure_ascii=False))
    shutil.copy2(repo / "tests/gdextension/godot/native_test.gd", root / "native_test.gd")
    environment = dict(os.environ)
    for variable, folder in [("XDG_DATA_HOME", "data"), ("XDG_CONFIG_HOME", "config"), ("XDG_CACHE_HOME", "cache")]:
        directory = Path(temporary) / folder
        directory.mkdir()
        environment[variable] = str(directory)
    for command in [["--editor", "--import"], ["--script", "res://native_test.gd"]]:
        output = subprocess.run([args.godot, "--headless", "--path", str(root), "--log-file", str(Path(temporary) / "godot.log"), *command], text=True, capture_output=True, timeout=45, env=environment)
        print("Godot exit:", output.returncode, flush=True)
        print(output.stdout, end="")
        print(output.stderr, end="")
        if output.returncode or any(marker in output.stderr for marker in ["SCRIPT ERROR:", "Parse Error:", "ERROR:"]):
            log = Path(temporary) / "godot.log"
            if log.exists():
                print(log.read_text(errors="replace")[-12000:])
            raise RuntimeError(f"Godot integration failed ({output.returncode}): {command}")
