"""Build both archives and compile a consumer against their extracted contents."""
from pathlib import Path
import os
import shutil
import subprocess
import tarfile
import tempfile
import tomllib

root = Path(__file__).resolve().parents[1]
version = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]["version"]
subprocess.run(["cargo", "package", "--workspace", "--locked", "--allow-dirty", "--no-verify"], cwd=root, check=True)
# A fresh consumer avoids Cargo's temporary-registry cache retaining an older
# archive when contributors package the same unpublished version repeatedly.
with tempfile.TemporaryDirectory(prefix="wove-package-") as directory:
    consumer = Path(directory)
    for name in ("wove", "wove-dioxus"):
        with tarfile.open(root / f"target/package/{name}-{version}.crate") as archive:
            archive.extractall(consumer, filter="data")
    (consumer / "Cargo.toml").write_text(f'''[package]
name = "consumer"
version = "0.0.0"
edition = "2021"
[features]
default = ["terminal"]
terminal = ["wove/terminal", "wove-dioxus/terminal"]
[dependencies]
wove = {{ path = "wove-{version}", default-features = false }}
wove-dioxus = {{ path = "wove-dioxus-{version}", default-features = false }}
[patch.crates-io]
wove = {{ path = "wove-{version}" }}
''', encoding="utf-8")
    (consumer / "src").mkdir()
    (consumer / "src/main.rs").write_text('''use wove::{Tree, elements::Text};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut tree = Tree::new();
    tree.add(tree.root(), Text::new("packed consumer"))?;
    let frame = tree.frame(20, 2)?;
    assert_eq!(frame.cell(0, 0).unwrap().symbol(), "p");
    let _registry = wove_dioxus::Registry::default();
    #[cfg(feature = "terminal")]
    wove::terminal::Renderer::default().draw(&mut Vec::new(), frame)?;
    Ok(())
}
''', encoding="utf-8")
    shutil.copy2(root / "Cargo.lock", consumer / "Cargo.lock")
    environment = {**os.environ, "CARGO_TARGET_DIR": str(root / "target/consumer")}
    for features in ([], ["--no-default-features"]):
        subprocess.run(["cargo", "run", "--offline", *features], cwd=consumer, env=environment, check=True)
