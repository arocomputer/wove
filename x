#!/bin/sh
# Shared commands for contributors and CI.
set -eu
cd "$(dirname "$0")"
command=${1:-check}
if [ "$#" -gt 0 ]; then shift; fi
case "$command" in
  check)
    ./x fmt --check
    ./x lint
    ./x test
    ./x docs
    ./x package
    ./x guard
    ;;
  fmt) cargo fmt --all "$@" ;;
  lint)
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo clippy --workspace --all-targets --locked --no-default-features -- -D warnings
    ;;
  test)
    cargo test --workspace --locked --all-targets "$@"
    cargo test --workspace --locked --doc
    cargo test --workspace --locked --no-default-features
    ;;
  docs) RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked ;;
  package) cargo package --locked --allow-dirty -p intuitums-weft ;;
  guard) python3 scripts/guard.py ;;
  hooks)
    git config extensions.worktreeConfig true
    git config --worktree core.hooksPath "$PWD/scripts/hooks"
    ;;
  ui)
    cargo build --locked --examples
    python3 -m venv target/ui-env
    target/ui-env/bin/python -m pip install --quiet -r scripts/ui-requirements.txt
    target/ui-env/bin/python scripts/ui.py
    ;;
  bench)
    cargo build --locked --release --examples
    python3 benchmarks/run.py
    ;;
  *) echo 'usage: ./x [hooks|check|fmt|lint|test|docs|package|guard|ui|bench]' >&2; exit 2 ;;
esac
