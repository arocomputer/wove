"""Audit selected dependency ecosystems; advisory service failures fail the check."""
from pathlib import Path
import argparse
import os
import sys
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("ecosystem", nargs="?", choices=("rust", "web", "python"))
    args = parser.parse_args()
    for ecosystem in (args.ecosystem,) if args.ecosystem else ("rust", "web", "python"):
        if ecosystem == "rust":
            subprocess.run(["cargo", "install", "cargo-audit", "--version", "0.22.2", "--locked"], cwd=ROOT, check=True)
            for lock in ('Cargo.lock', 'fuzz/Cargo.lock'):
                if (ROOT / lock).is_file():
                    subprocess.run(['cargo', 'audit', '--file', lock], cwd=ROOT, check=True)
        elif ecosystem == "web":
            subprocess.run(["bun", "install", "--frozen-lockfile"], cwd=ROOT / "crates/web", check=True)
            subprocess.run(["bun", "audit"], cwd=ROOT / "crates/web", check=True)
        else:
            environment = ROOT / "target/audit"
            subprocess.run([sys.executable, "-m", "venv", str(environment)], check=True)
            python = environment / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
            subprocess.run([str(python), "-m", "pip", "install", "pip-audit==2.10.1"], check=True)
            subprocess.run([str(python), "-m", "pip_audit", "--disable-pip", "--no-deps", "--progress-spinner", "off",
                            "-r", str(ROOT / "scripts/ui/requirements.txt")], check=True)


if __name__ == "__main__":
    main()
