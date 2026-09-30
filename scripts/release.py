"""Verify a version tag on protected main, then stage tested crate archives and checksums."""
import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import tomllib

from package import archive, published, root


def verify(tag=None):
    """Reject off-main tags, dirty tracked files, and inconsistent internal versions."""
    version, names = published()
    if tag is None and os.environ.get("GITHUB_REF_TYPE") != "tag":
        raise ValueError("release must run on a version tag")
    tag = tag or os.environ.get("GITHUB_REF_NAME")
    if tag != f"v{version}":
        raise ValueError("release must run on the tag matching the workspace version")
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?", version):
        raise ValueError("invalid release version")
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root)
    tagged = subprocess.check_output(["git", "rev-parse", f"refs/tags/{tag}^{{commit}}"], cwd=root)
    if head != tagged:
        raise ValueError("checkout does not match the release tag")
    subprocess.run(["git", "merge-base", "--is-ancestor", "HEAD", "origin/main"], cwd=root, check=True)
    for staged in ([], ["--cached"]):
        subprocess.run(["git", "diff", "--exit-code", *staged], cwd=root, check=True)
    for manifest in (root / "crates").glob("*/Cargo.toml"):
        data = tomllib.loads(manifest.read_text())
        if data["package"].get("publish") is False:
            continue
        for section in ("dependencies", "dev-dependencies", "build-dependencies"):
            for name, dependency in data.get(section, {}).items():
                if isinstance(dependency, dict) and dependency.get("package", name) in names:
                    if dependency.get("version") != f"={version}":
                        raise ValueError(f"{manifest}: {name} must require ={version}")
    return version, names


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--verify", action="store_true", help="Verify release provenance before executing builds")
    parser.add_argument("--tag", help="Existing authorized tag for manual registry publication")
    args = parser.parse_args()
    version, names = verify(args.tag)
    if args.verify:
        print(f"verified v{version} on main")
        return
    out = root / "artifacts/release"
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)
    checksums = []
    for name in names:
        crate = archive(name, version)
        if not crate.is_file():
            raise ValueError(f"{crate} is missing; run ./x package first")
        shutil.copy2(crate, out / crate.name)
        checksums.append(f"{hashlib.sha256(crate.read_bytes()).hexdigest()}  {crate.name}")
    (out / "SHA256SUMS").write_text("\n".join(checksums) + "\n")
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root).decode().strip()
    (out / "source.json").write_text(json.dumps({"tag": f"v{version}", "commit": commit, "packages": names}, indent=2) + "\n")


if __name__ == "__main__":
    main()
