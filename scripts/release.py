"""Validate a release tag and stage verified crate archives and checksums."""
import hashlib
import os
from pathlib import Path
import shutil
import tomllib

root = Path(__file__).resolve().parents[1]
version = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]["version"]
if os.environ["GITHUB_REF_NAME"] != f"v{version}":
    raise SystemExit("release tag does not match workspace version")
out = root / "artifacts/release"
out.mkdir(parents=True, exist_ok=True)
checksums = []
for manifest in sorted((root / "crates").glob("*/Cargo.toml")):
    package = tomllib.loads(manifest.read_text(encoding="utf-8"))["package"]
    if package.get("publish") is False:
        continue
    name = package["name"]
    archive = root / f"target/package/{name}-{version}.crate"
    shutil.copy2(archive, out / archive.name)
    checksums.append(f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}")
(out / "SHA256SUMS").write_text("\n".join(checksums) + "\n")
