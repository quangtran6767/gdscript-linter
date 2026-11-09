#!/usr/bin/env python3
"""Generate and measure a 1000-file project, without modifying real game files."""
import argparse
import json
from pathlib import Path
import shutil
import statistics
import subprocess
import tempfile
import time

parser = argparse.ArgumentParser()
parser.add_argument("--binary", default="target/release/gdguard")
parser.add_argument("--files", type=int, default=1000)
parser.add_argument("--runs", type=int, default=5)
parser.add_argument("--keep", type=Path)
args = parser.parse_args()
binary = Path(args.binary).resolve()
root = args.keep or Path(tempfile.mkdtemp(prefix="gdguard-benchmark-"))
root.mkdir(parents=True, exist_ok=True)
(root / "project.godot").write_text('config_version=5\n[application]\nconfig/name="gdguard benchmark"\n')
(root / "gdguard.toml").write_text('[lint]\nexclude=["addons/**", ".godot/**"]\n')
for index in range(args.files):
    (root / f"script_{index:04}.gd").write_text(f'''class_name Bench{index}
extends Node

signal changed(value: int)
var count: int = 0

func update(step: int) -> void:
\tcount += step
\tchanged.emit(count)
''')

def run():
    start = time.perf_counter()
    output = subprocess.run([str(binary), "lint", "--format", "json", "."], cwd=root, capture_output=True)
    elapsed = time.perf_counter() - start
    if output.returncode not in (0, 1):
        raise RuntimeError(output.stderr.decode())
    return elapsed, output.stdout

try:
    cold, warm = [], []
    reference = None
    for _ in range(args.runs):
        shutil.rmtree(root / ".gdguard/cache", ignore_errors=True)
        elapsed, output = run()
        cold.append(elapsed)
        if reference is None:
            reference = output
        assert output == reference, "cold output is nondeterministic"
        elapsed, output = run()
        warm.append(elapsed)
        assert output == reference, "warm output differs from cold"
    report = {"files": args.files, "runs": args.runs, "cold_median_ms": round(statistics.median(cold)*1000, 2),
              "warm_median_ms": round(statistics.median(warm)*1000, 2), "output_identical": True,
              "cold_target_met": statistics.median(cold) < 3, "warm_target_met": statistics.median(warm) < 1}
    print(json.dumps(report, indent=2))
    if not report["cold_target_met"] or not report["warm_target_met"]:
        raise SystemExit(1)
finally:
    if not args.keep:
        shutil.rmtree(root)
