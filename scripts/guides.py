"""Compile the actual quickstart source against this checkout, without opening a terminal."""
from pathlib import Path
import os
import re
import shutil
import subprocess
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def main():
    """Extract the complete app rather than maintaining a second example copy."""
    guide = (ROOT / "crates/web/src/content/docs/quickstart.mdx").read_text()
    sources = re.findall(r"^```rust\n(.*?)^```", guide, re.MULTILINE | re.DOTALL)
    if len(sources) != 1 or "fn main(" not in sources[0]:
        raise ValueError("quickstart must contain one complete Rust app")
    toolchain = tomllib.loads((ROOT / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
    with tempfile.TemporaryDirectory(prefix="wove-guide-") as directory:
        project = Path(directory)
        (project / "src").mkdir()
        (project / "src/main.rs").write_text(sources[0])
        shutil.copy2(ROOT / "Cargo.lock", project / "Cargo.lock")
        core = str(ROOT / "crates/core")
        (project / "Cargo.toml").write_text(
            '[package]\nname = "wove-guide"\nversion = "0.0.0"\nedition = "2021"\n'
            f'[dependencies]\nwove = {{ path = {core!r} }}\n'
        )
        environment = {**os.environ, "RUSTUP_TOOLCHAIN": toolchain,
                       "CARGO_TARGET_DIR": str(ROOT / "target/guides")}
        subprocess.run(["cargo", "check"], cwd=project, env=environment, check=True)


if __name__ == "__main__":
    main()
