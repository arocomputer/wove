"""Prepare Cargo's registry upload protocol using the exact verified crate archives."""
import hashlib
import json
from pathlib import Path
import struct
import sys
import tarfile
import tomllib


def payload(archive):
    """Serialize normalized package metadata without rebuilding or executing the crate."""
    with tarfile.open(archive) as crate:
        prefix = archive.name.removesuffix(".crate") + "/"
        manifest = tomllib.loads(crate.extractfile(prefix + "Cargo.toml").read().decode())
        package = manifest["package"]
        metadata = {"name": package["name"], "vers": package["version"],
                    "features": manifest.get("features", {}), "deps": [], "badges": manifest.get("badges", {})}
        for key in ("authors", "keywords", "categories"):
            metadata[key] = package.get(key, [])
        for key in ("description", "documentation", "homepage", "license", "repository", "links"):
            metadata[key] = package.get(key)
        metadata["license_file"] = package.get("license-file")
        metadata["rust_version"] = package.get("rust-version")
        readme = package.get("readme")
        metadata["readme_file"] = readme if isinstance(readme, str) else None
        metadata["readme"] = crate.extractfile(prefix + readme).read().decode() if isinstance(readme, str) else None
        for target, table in [(None, manifest), *manifest.get("target", {}).items()]:
            for section, kind in (("dependencies", "normal"), ("dev-dependencies", "dev"), ("build-dependencies", "build")):
                for name, spec in table.get(section, {}).items():
                    spec = {"version": spec} if isinstance(spec, str) else spec
                    if "version" not in spec or "git" in spec or "path" in spec:
                        raise ValueError(f"{archive}: dependency {name} is not a normalized registry dependency")
                    metadata["deps"].append({
                        "name": spec.get("package", name), "version_req": spec["version"],
                        "features": spec.get("features", []), "optional": spec.get("optional", False),
                        "default_features": spec.get("default-features", True), "target": target, "kind": kind,
                        "registry": spec.get("registry-index"), "explicit_name_in_toml": name if "package" in spec else None,
                    })
    # https://doc.rust-lang.org/cargo/reference/registry-web-api.html#publish
    description = json.dumps(metadata, separators=(",", ":")).encode()
    source = archive.read_bytes()
    return struct.pack("<I", len(description)) + description + struct.pack("<I", len(source)) + source


def main(directory):
    """Stage upload bodies and extend integrity checks before any credentials are issued."""
    archives = sorted(directory.glob("*.crate"))
    if len(archives) != 5:
        raise ValueError("expected five verified Wove crate archives")
    with (directory / "SHA256SUMS").open("a") as checksums:
        for archive in archives:
            upload = archive.with_suffix(".publish")
            upload.write_bytes(payload(archive))
            checksums.write(f"{hashlib.sha256(upload.read_bytes()).hexdigest()}  {upload.name}\n")


if __name__ == "__main__":
    main(Path(sys.argv[1]))
