# Working on Wove

Instructions for agents and contributors editing this repository. Read
[CONTRIBUTING.md](CONTRIBUTING.md) for contribution accountability rules, and
[architecture](crates/web/src/content/docs/architecture.mdx) before changing contracts.

Wove is a Rust library for building terminal user interfaces. It must serve applications
with different designs and runtimes. Keep application policy out of the library.
Capitalize Wove in prose; keep Cargo packages and Rust imports lowercase.

## Working style

- Preserve unrelated changes. Never revert or reformat files outside the task.
- Never expose secrets. Use synthetic fixtures and review logs before sharing them.
- Choose the smallest concrete design that solves the problem. A new abstraction,
  dependency, feature flag, or crate needs a use case beyond symmetry.
- Document public contracts and nontrivial functions by purpose. Update comments
  and guides when behavior changes; do not append corrections to stale guidance.
- Claim performance or compatibility only with reproducible evidence.

## Build and check

The Rust toolchain is pinned. Python 3.12 or newer runs repository tooling;
Bun 1.4.2 or newer builds the website. Run `./x hooks` once per worktree.

Use the smallest relevant checks during development:

- `./x core`, `./x dioxus`, `./x keymap`, `./x ssh`, or `./x gpu` tests one package.
- `./x quality` checks shared formatting, lint, rustdoc, packaged consumers, and guards.
- `./x ui` checks Unix PTY input, resize, terminal restoration, and golden frames.
- `./x web` builds the website and checks generated links and anchors.
- `./x workflows` checks workflow syntax and security with checksum-pinned tools.
- `./x fuzz-check` compiles the bounded-input fuzz targets.
- `./x guides` compiles the actual quickstart against this checkout.
- `./x affected plan --base origin/main` explains the work selected for local changes.

Run `./x check` for Rust contract or dependency changes, `./x ui` for rendering,
input, or terminal changes, and `./x web` for published docs or site changes.
Repository prose needs `git diff --check`; a prose-only change does not need Rust
builds. CI selection, release gates, and settings migration are documented once
in [CONTRIBUTING.md](CONTRIBUTING.md#continuous-integration).

CI has one required `CI` result. Its selected package matrices include GPU;
unaffected work skips explicitly. Selection failures, cancellations, and unexpected
skips must fail the gate. Do not add redundant jobs or weaken selection to pass CI.

The PTY suite retains frames and terminal bytes in `artifacts/ui/`. Each frame
must match its golden in `scripts/ui/frames/`. Review the actual frame before
updating a golden; never relax an assertion to hide a rendering regression.
The `timing` example prints local measurements, not a benchmark gate.

## Where things live

```text
crates/core/       wove: elements, layout, events, text, and terminal rendering
  src/element.rs  the contract for built-in and custom elements
  src/input/      portable events, keys, modifiers, responses; decoder.rs turns
                  terminal input bytes into events for any byte transport
  src/tree/       node ownership, focus, input routing; paint.rs measures and paints
  src/elements/   text, inputs, lists, tables, scrolling, panels, containers,
                  pixels.rs, an image drawn with block characters,
                  feed.rs, the virtualized column for long text documents, and
                  lazy.rs, the virtualized column of element subtrees
  src/text/       grapheme editing, undo, shared input behavior, text layout
  src/render/     flat cell buffers, styles, geometry, clipped drawing, and output
                  as plain bytes with no backend: renderer.rs (full screen),
                  inline.rs (main screen with native scrollback), session.rs
                  (the modes a session enables), pen.rs (cells to escapes),
                  surface.rs (clipboard, title, bell, progress)
  src/terminal/   optional crossterm ownership of the local terminal, restored on
                  panic and fatal signals; query.rs is the one startup probe
  src/{markdown,syntax,diff}.rs  independent optional formatting features
  src/testing.rs headless screens, clocks, and frame recording
crates/dioxus/    component adapter; core does not depend on it
  src/view.rs    component state and event routing
  src/host/      tree mutations and typed RSX attributes
  src/runtime.rs optional local terminal loop
crates/keymap/   scoped command bindings, sequences, and timeouts
crates/gpu/      wgpu rendering into Pixels elements; its test needs a GPU or a
                 software adapter, which CI installs on Linux
crates/ssh/      authenticated remote applications
  src/server.rs  listener, authentication, PTY requests, connection lifecycle
  src/runtime.rs application thread, frame output, and remote cleanup
crates/examples/ unpublished application examples
crates/web/      Bun/Astro website; excluded from the Cargo workspace
  src/content/docs/  published MDX guides
scripts/         checks, hooks, package verification, release preparation, PTY tests
```

## Fast test loops

```sh
cargo test -p wove --test text
cargo test -p wove --test tree
cargo test -p wove --test render
cargo test -p wove --test inline
cargo test -p wove --no-default-features --features markdown --test markdown
cargo test -p wove-dioxus --test view
cargo test -p wove-keymap
cargo test -p wove-ssh --test connection
cargo test -p wove-gpu --test render
python3 -m unittest discover -s scripts/hooks -p 'test_*.py'
python3 -m unittest discover -s scripts/ci -p 'test_*.py'
```

Tests should pin observable behavior. A regression test must fail against the
unfixed code for the intended reason. Reuse `testing::Screen` and existing test
helpers. Use temporary directories and loopback listeners on ephemeral ports;
never depend on personal configuration, credentials, or a public service.

## Library boundaries

- Core has no dependency on adapters, keymaps, or transports. Its default feature
  is `terminal`; `markdown`, `syntax`, and `diff` stay independently optional.
  A feature must compile without the terminal backend and without sibling features.
- Elements measure and paint in terminal cells. Preserve grapheme boundaries,
  clipping, wide-cell ownership, deterministic layout, and cursor state.
- Nodes own element state. Moves preserve identity; removal drops the subtree and
  its resources. Keep focus, hit testing, and event delivery consistent with visibility.
- Core events carry no backend types. Event responses must describe consumption
  and repaint changes accurately. Command matching belongs to keymap.
- Adapters use public tree operations. Dioxus component state must not become a
  requirement for direct Rust use, and a failed mutation must not silently continue.
- SSH owns its authentication and network lifecycle. Require a supplied host key
  and authorization policy. Preserve bounded input, output waits, and dimensions.
  Do not force Send onto core elements just to satisfy transport scheduling.
- GPU work stays in `wove-gpu`. It writes frames into `Pixels` and nothing
  else; core never depends on a graphics API, and a missing adapter is an error
  the application sees, not a panic.
- Text content must not emit terminal control sequences. Terminal and SSH backends
  own protocol output and cleanup. No unsafe code in library modules.
- When changing a shared contract, check direct core use, Dioxus, keymap, SSH, GPU,
  and examples. Update every affected consumer and guide in the same change.

## Naming and documentation

Core building blocks are elements; components compose them through an adapter;
nodes identify them in a tree. Prefer short names such as `Tree`, `Id`, and `Text`.
Use modules to provide context instead of suffixes such as Manager or Provider.
Keep established tool filenames and conventional Rust naming.

Crate READMEs introduce their APIs. Published guides live under
`crates/web/src/content/docs/`; repository policy stays in root markdown files.
Do not create a contributing folder or duplicate guides. Website design and
hosting changes require a task that asks for them.

## Branches and review

- `main` is the default branch. Before pushing, use `<type>/<slug>`, such as
  `fix/input-selection` or `chore/repository-guides`. Do not push main unless
  explicitly authorized. Never force-push another contributor's work.
- Open a PR only when asked. Keep its scope coherent and its description about
  the final change. Do not merge, publish crates, or create releases without an
  explicit maintainer instruction.
- Use conventional commit titles. Scopes, when useful, are `core`, `dioxus`,
  `keymap`, `ssh`, `web`, `infra`, and `docs`.
- Use `.github/pull_request_template.md`. Explain the problem, what changed, why
  it works, and the checks run with their results. Link an issue when applicable.
  Include captured frames for visual changes and migration notes for API or
  feature changes. Use enough detail for review; there is no fixed sentence limit.
- Do not apply PR labels or add automatic PR labeling. Describe the change type
  in the title and template. Issue labels are separate.
- Describe user-visible changes and migration steps in the PR. GitHub Releases
  are the changelog; use the release-note format in CONTRIBUTING.md. Do not add
  a changelog file or a roadmap.
- Follow the contribution accountability rules in CONTRIBUTING.md. Preserve
  accurate authorship, service identities, signatures, and attribution metadata.
  Do not rewrite history merely to add or remove agent attribution.

## Repository automation

`./x` defines the contributor commands. Keep checks there and have CI call them.
Actions are pinned to full commit SHAs and workflows use minimal permissions.
Never execute contributor code with a privileged PR token.

`scripts/guard.py` checks action pins, library safety rules, and core dependencies.
A legitimate boundary change updates the guard with an explanation; do not bypass
it. Package checks must compile consumers from archives, not only workspace paths.
Release automation creates GitHub artifacts only after an explicit version tag;
it does not publish to crates.io. See CONTRIBUTING.md for release preparation.
