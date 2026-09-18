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
    cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
    cargo clippy --workspace --all-targets --locked --no-default-features -- -D warnings
    ;;
  test)
    cargo test --workspace --locked --all-features --all-targets "$@"
    cargo test --workspace --locked --all-features --doc
    cargo test --workspace --locked --no-default-features
    cargo test -p wove --locked --no-default-features
    for feature in markdown syntax diff keymap; do
      cargo test -p wove --locked --no-default-features --features "$feature"
    done
    ;;
  docs) RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked ;;
  package) python3 scripts/package.py ;;
  web)
    cd crates/web
    bun install --frozen-lockfile
    bun run check
    bun run build
    ;;
  guard) python3 scripts/guard.py ;;
  hooks)
    git config extensions.worktreeConfig true
    git config --worktree core.hooksPath "$PWD/scripts/hooks"
    ;;
  ui)
    cargo build --workspace --locked --examples --bins
    python3 -m venv target/ui
    target/ui/bin/python -m pip install --quiet -r scripts/ui/requirements.txt
    target/ui/bin/python scripts/ui.py
    ;;
  *) echo 'usage: ./x [hooks|check|fmt|lint|test|docs|package|guard|ui|web]' >&2; exit 2 ;;
esac
