# Contributing to Wove

Wove is a general-purpose terminal UI library. Propose features with a concrete
use case and a small example. Keep application policy outside the library.

Install Rust through rustup and Python 3.12 or newer. The checked-in toolchain
file selects the compiler. Each contributing worktree installs hooks once:

```sh
./x hooks
./x check
./x ui
```

`./x check` checks formatting, lints, contracts, documentation, package contents,
and repository rules. CI uses the same entry point. `./x ui` checks real PTYs
on Unix and retains frames under `artifacts/ui/`. The optional
`cargo run -p wove --release --example timing` command prints local frame timings.
It has no thresholds and does not run in CI.

Add focused tests for behavior changes. A regression test should fail for the
original defect. Document public contracts beside the API and update guides
when behavior changes. Add user-visible changes under Unreleased in CHANGELOG.md.

Use conventional commit titles, such as `fix: preserve wide glyphs on resize`.
Keep each change about one concern. Name branches `feat/input`,
`fix/resize`, or another conventional type. Never commit secrets.

Before 1.0, incompatible public API changes increment the minor version.
Releases require an explicit maintainer decision; commits do not publish crates.

## Documentation

Published guides, architecture, and the roadmap live in `crates/web/src/content/docs/`.
Crate READMEs introduce their package and link to these guides. Contribution and
release instructions live here.

## Naming

Core building blocks are elements. Components compose elements through an
optional framework adapter. Nodes identify elements within a tree.

Use short, concrete names. The project is Wove; crate imports are `wove`
and `wove_dioxus`. Prefer
`Tree`, `Id`, `Text`, and `Scroll` to compound names with generic suffixes
such as Manager, Handler, Provider, or Renderable. Use module paths to supply
context instead of repeating it in every type name.

Use conventional abbreviations only when they are familiar, such as `Rect`,
`fg`, and `bg`. Keep meaningful Rust snake_case names when two words are needed.
Prefer a single word for files and directories; use a directory when it groups
related files. Preserve ecosystem names such as `rust-toolchain.toml` and
`pre-commit`, which tools recognize. Do not rename dependencies or command flags.

## Releases

Publishable packages are `wove`, `wove-dioxus`, `wove-keymap`, and `wove-ssh`.
`crates/examples` is private. `crates/web` uses Bun and is excluded from Cargo. The core has no dependency on the other packages.

The workspace version is 0.0.1 and has not been published. The earlier `wove`
0.2.0 package is yanked on crates.io. The other packages have not been published.

1. Run `./x check` and `./x ui` on the release commit.
2. Choose the workspace version, update the adapter's exact core dependency,
   move Unreleased changes into that version, update Cargo.lock, and commit.
3. `./x package` builds the publishable archives and runs an external consumer against their
   extracted contents, with and without the terminal backend.
4. After a maintainer authorizes registry publication, publish core first, then
   the dependent packages. Verify package ownership and credentials before the first release.
5. Tag the release commit `v<version>` and push the tag. The workflow checks the
   version and publishes the archives and their checksums to a GitHub release.
   It does not publish to crates.io.

Actions are pinned by commit. Only the release job has contents write permission.
Do not publish from a pull request.

## Website

`crates/web/` is a minimal Astro and MDX documentation setup, using Bun 1.4.2 or newer.
Run `./x web` for dependency, type, and build checks. Do not add a full website
design or deployment configuration until requested.
