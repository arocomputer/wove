"""Validate a release tag and stage the verified crate and its checksum."""
import hashlib
import os
from pathlib import Path
import shutil
import tomllib

root = Path(__file__).resolve().parents[1]
version = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]["version"]
if os.environ["GITHUB_REF_NAME"] != f"v{version}":
    raise SystemExit("release tag does not match workspace version")
changelog = (root / "CHANGELOG.md").read_text(encoding="utf-8")
heading = f"## {version}"
if heading not in changelog.splitlines():
    raise SystemExit("missing versioned changelog entry")
notes = changelog.split(heading + "\n", 1)[1].split("\n## ", 1)[0].strip()
out = root / "artifacts/release"
out.mkdir(parents=True, exist_ok=True)
checksums = []
for manifest in sorted((root / "crates").glob("*/Cargo.toml")):
    name = tomllib.loads(manifest.read_text(encoding="utf-8"))["package"]["name"]
    archive = root / f"target/package/{name}-{version}.crate"
    shutil.copy2(archive, out / archive.name)
    checksums.append(f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}")
(out / "SHA256SUMS").write_text("\n".join(checksums) + "\n")
(root / "artifacts/notes.md").write_text(notes + "\n", encoding="utf-8")
