# weft

A Rust library to build terminal user interfaces.

Compose widgets, divide space, and render application-owned state. Use the
included terminal loop or drive rendering from your own event loop. Drawing
and layout work in memory without a terminal backend or async runtime.

**Early development.** The first version supports full-screen applications,
fixed and weighted layouts, styled Unicode text, borders, and selectable lists.
The API is not stable yet. Text editing, wrapping, focus navigation, mouse
routing, and inline output are planned, not implemented.

## Try it

```sh
git clone https://github.com/intuitums/weft
cd weft
cargo run --example explorer
cargo run --example counter
```

The [captured explorer frame](docs/assets/explorer.txt) shows a two-pane catalog. The counter is a small complete
application. Arrow keys navigate, and `q` or Ctrl-C exits.

## Use it

The crate is not published yet. Depend on the repository and pin a reviewed
commit for reproducible builds:

```toml
[dependencies]
weft = { package = "intuitums-weft", git = "https://github.com/intuitums/weft" }
```

```rust
use weft::{Buffer, Style, Text, Widget};

let mut frame = Buffer::new(30, 3);
Text {
    content: "Hello, terminal.",
    style: Style::default(),
}.render(frame.area(), &mut frame);

assert_eq!(frame.lines()[0].trim_end(), "Hello, terminal.");
```

Disable default features for pure layout, drawing, and widget tests. The
`terminal` feature adds crossterm input and output. weft does not require Tokio,
a global application store, code generation, or a second compiler.

## Design

Applications own their state, palette, and event policy. Widgets draw into
bounded regions of a cell buffer. Layouts nest to compose screens. A renderer
compares completed frames and writes changed cells. A terminal session owns
raw mode and restores it when dropped, including during panic unwinding.

weft is intended for file browsers, dashboards, editors, developer tools, and
other terminal applications. No application-specific concepts live in the
library.

- [Getting started](docs/getting-started.md)
- [Architecture and contracts](docs/architecture.md)
- [Roadmap](docs/roadmap.md)
- [Contributing](CONTRIBUTING.md)
- [Performance methodology](benchmarks/README.md)

## Development

```sh
./x hooks
./x check
./x ui
./x bench
```

MIT licensed. Linux, macOS, and Windows builds are checked in CI. PTY scenarios
run on Linux and macOS. Terminal appearance and Unicode widths can vary by
terminal emulator and font.
