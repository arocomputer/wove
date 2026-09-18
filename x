#!/bin/sh
# Shared commands for contributors and CI.
set -eu
cd "$(dirname "$0")"
command=${1:-check}
if [ "$#" -gt 0 ]; then shift; fi
case "$command" in
  check)
    ./x quality
    ./x test
    ;;
  quality)
    ./x fmt --check
    ./x lint
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
    for feature in markdown syntax diff; do
      cargo test -p wove --locked --no-default-features --features "$feature"
    done
    ;;
  core|dioxus|keymap|ssh)
    # Package workflows exercise their crate without enabling sibling features.
    package="wove-$command"
    if [ "$command" = core ]; then package=wove; fi
    cargo test -p "$package" --locked --all-features --all-targets "$@"
    cargo test -p "$package" --locked --all-features --doc
    cargo test -p "$package" --locked --no-default-features
    if [ "$command" = core ]; then
      for feature in markdown syntax diff; do
        cargo test -p wove --locked --no-default-features --features "$feature"
      done
    fi
    ;;
  docs) RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked ;;
  package) python3 scripts/package.py ;;
  web)
    cd crates/web
    bun install --frozen-lockfile
    bun run check
    bun run build
    ;;
  guard)
    python3 scripts/guard.py
    python3 -m unittest discover -s scripts/hooks -p 'test_*.py'
    ;;
  hooks)
    python3 scripts/hooks/install.py
    ;;
  ui)
    cargo build --workspace --locked --examples --bins
    python3 -m venv target/ui
    target/ui/bin/python -m pip install --quiet -r scripts/ui/requirements.txt
    target/ui/bin/python scripts/ui.py
    ;;
  *) echo 'usage: ./x [hooks|check|quality|core|dioxus|keymap|ssh|fmt|lint|test|docs|package|guard|ui|web]' >&2; exit 2 ;;
esac
