"""Check repository trust boundaries without reading user configuration."""
from pathlib import Path
import re

root = Path(__file__).resolve().parents[1]
for path in (root / ".github/workflows").glob("*.yml"):
    for action in re.findall(r"uses:\s*(\S+)", path.read_text(encoding="utf-8")):
        if not re.fullmatch(r"[\w./-]+@[0-9a-f]{40}", action):
            raise SystemExit(f"{path}: action must be pinned by commit: {action}")
for path in (root / "crates").glob("*/src/**/*.rs"):
    if re.search(r"\bunsafe\s*(?:\{|impl|fn)", path.read_text(encoding="utf-8")):
        raise SystemExit(f"{path}: library must not use unsafe code")
manifest = (root / "crates/core/Cargo.toml").read_text(encoding="utf-8")
if "dioxus" in manifest:
    raise SystemExit("core must not depend on a component adapter")
print("repository guards passed")
