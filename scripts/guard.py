"""Check repository trust boundaries without reading user configuration."""
from pathlib import Path
import re
import tomllib

root = Path(__file__).resolve().parents[1]
for path in (root / ".github/workflows").glob("*.yml"):
    for action in re.findall(r"uses:\s*(\S+)", path.read_text(encoding="utf-8")):
        if not re.fullmatch(r"[\w./-]+@[0-9a-f]{40}", action):
            raise SystemExit(f"{path}: action must be pinned by commit: {action}")

# The compiler enforces the unsafe-code ban once each published crate forbids it.
for manifest in sorted((root / "crates").glob("*/Cargo.toml")):
    if tomllib.loads(manifest.read_text(encoding="utf-8"))["package"].get("publish") is False:
        continue
    library = manifest.parent / "src/lib.rs"
    if not re.search(r"^#!\[forbid\(unsafe_code\)\]", library.read_text(encoding="utf-8"), re.MULTILINE):
        raise SystemExit(f"{library}: published crates must declare #![forbid(unsafe_code)]")

# Core must not depend on any sibling package, including renamed or target-specific entries.
core = tomllib.loads((root / "crates/core/Cargo.toml").read_text(encoding="utf-8"))
tables = [core, *core.get("target", {}).values()]
for table in tables:
    for kind in ("dependencies", "dev-dependencies", "build-dependencies",
                 "dev_dependencies", "build_dependencies"):
        for key, spec in table.get(kind, {}).items():
            name = spec.get("package", key) if isinstance(spec, dict) else key
            if name.startswith("wove"):
                raise SystemExit(f"core must not depend on another Wove package: {kind} {name}")
print("repository guards passed")
