# Contributing to weft

weft is a general-purpose terminal UI library. Propose features with a concrete
use case and a small example. Keep application policy outside the library.

Install Rust through rustup and Python 3.11 or newer. The checked-in toolchain
file selects the compiler. Each contributing worktree installs hooks once:

```sh
./x hooks
./x check
./x ui
./x bench
```

`./x check` checks formatting, lints, contracts, documentation, package contents,
and repository rules. CI uses the same entry point. `./x ui` checks real PTYs
on Unix and retains frames under `artifacts/ui/`. `./x bench` records local
performance and applies broad regression ceilings.

Add focused tests for behavior changes. A regression test should fail for the
original defect. Document public contracts beside the API and update guides
when behavior changes. Add user-visible changes under Unreleased in CHANGELOG.md.

Use conventional commit titles, such as `fix: preserve wide glyphs on resize`.
Keep each change about one concern. Name branches `feat/short-description`,
`fix/short-description`, or another conventional type. Never commit secrets.

Before 1.0, incompatible public API changes increment the minor version.
Releases require an explicit maintainer decision; commits do not publish crates.
See [releasing](contributing/releases.md).
