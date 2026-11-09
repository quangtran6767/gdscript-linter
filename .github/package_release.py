#!/usr/bin/env python3
"""Review/package existing builds. No version edits, downloads, git or tags."""
import argparse
import json
from pathlib import Path
import platform
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parent.parent


def main():
    host_os = {"Darwin": "macos", "Windows": "windows"}.get(platform.system(), "linux")
    host_arch = "aarch64" if platform.machine().lower() in ("aarch64", "arm64") else "x86_64"
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dry-run", action="store_true", help="Validate inputs and print archive manifest; write nothing")
    parser.add_argument("--output", type=Path, help="Write archives here (required unless --dry-run)")
    parser.add_argument("--platform", default=f"{host_os}-{host_arch}", choices=[f"{os}-{arch}" for os in ("linux", "macos", "windows") for arch in ("x86_64", "aarch64")])
    parser.add_argument("--binary-dir", type=Path, default=ROOT / "target/release")
    parser.add_argument("--only", nargs="+", choices=("cli", "native", "common"), default=["cli", "native", "common"])
    args = parser.parse_args()
    if not args.dry_run and args.output is None:
        parser.error("pass --output DIR to package, or --dry-run to review")
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    archives = {}
    suffix = ".exe" if args.platform.startswith("windows-") else ""
    if "cli" in args.only:
        archives[f"gdguard-{version}-{args.platform}{suffix}.zip"] = [
            (args.binary_dir / (name + suffix), name + suffix)
            for name in ("gdguard", "gdscript-formatter")
        ] + [(ROOT / "LICENSE", "LICENSE")]
    if "native" in args.only:
        library = ("gdguard_gdextension.dll" if suffix else
                   "libgdguard_gdextension.dylib" if args.platform.startswith("macos-") else
                   "libgdguard_gdextension.so")
        entries = addon_files("gdguard_standalone") + addon_files("gdguard_lint")
        entries.append((args.binary_dir / library, f"addons/gdguard_standalone/bin/{library}"))
        archives[f"gdguard-native-{version}-{args.platform}.zip"] = entries
    if "common" in args.only:
        archives[f"gdguard-addon-{version}.zip"] = addon_files("gdguard")
        archives[f"gdguard-rules-{version}.zip"] = [
            (ROOT / "integrations/types/gdguard.d.ts", "gdguard.d.ts"),
        ] + [(path, str(path.relative_to(ROOT))) for path in sorted((ROOT / "examples/rules").rglob("*")) if path.is_file()]
    for entries in archives.values():
        for source, _ in entries:
            if not source.is_file():
                parser.error(f"missing release input: {source}; build the workspace in release mode first")
    manifest = {name: [destination for _, destination in entries] for name, entries in archives.items()}
    if not args.dry_run:
        args.output.mkdir(parents=True, exist_ok=True)
        for name, entries in archives.items():
            with zipfile.ZipFile(args.output / name, "w", compression=zipfile.ZIP_DEFLATED) as archive:
                for source, destination in entries:
                    archive.write(source, destination)
    print(json.dumps({"version": version, "dry_run": args.dry_run, "archives": manifest}, indent=2))


def addon_files(name):
    return [(path, str(path.relative_to(ROOT))) for path in sorted((ROOT / "addons" / name).rglob("*"))
            if path.is_file() and path.suffix not in (".import", ".so", ".dll", ".dylib")]


if __name__ == "__main__":
    main()
