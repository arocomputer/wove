"""Validate a release tag and stage the crate archives from `./x package` with checksums."""
import hashlib
import os
import shutil

from package import archive, published, root

version, names = published()
if os.environ["GITHUB_REF_NAME"] != f"v{version}":
    raise SystemExit("release tag does not match workspace version")
out = root / "artifacts/release"
out.mkdir(parents=True, exist_ok=True)
checksums = []
for name in names:
    crate = archive(name, version)
    if not crate.is_file():
        raise SystemExit(f"{crate} is missing; run ./x package first")
    shutil.copy2(crate, out / crate.name)
    checksums.append(f"{hashlib.sha256(crate.read_bytes()).hexdigest()}  {crate.name}")
(out / "SHA256SUMS").write_text("\n".join(checksums) + "\n")
