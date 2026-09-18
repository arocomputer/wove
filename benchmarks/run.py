"""Measure release artifacts and enforce broad regression ceilings."""
import json
from pathlib import Path
import platform
import statistics
import subprocess
import time

root = Path(__file__).resolve().parents[1]
examples = root / "target/release/examples"
extension = ".exe" if platform.system() == "Windows" else ""
probe = examples / ("measure" + extension)
samples = []
for _ in range(25):
    start = time.perf_counter()
    subprocess.run([str(probe), "--startup"], check=True)
    samples.append((time.perf_counter() - start) * 1000)
frame_us = float(subprocess.check_output([str(probe)], text=True).split()[0])
result = {
    "platform": platform.platform(),
    "explorer_bytes": (examples / ("explorer" + extension)).stat().st_size,
    "warm_launch_ms": round(statistics.median(samples[5:]), 3),
    "frame_us": frame_us,
}
(root / "artifacts").mkdir(exist_ok=True)
(root / "artifacts/bench.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result, indent=2))
for key, ceiling in json.loads((root / "benchmarks/budgets.json").read_text(encoding="utf-8")).items():
    if result[key] > ceiling:
        raise SystemExit(f"{key}: {result[key]} exceeds {ceiling}")
