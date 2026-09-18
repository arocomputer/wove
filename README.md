<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/logo/dark.svg">
  <img src="docs/assets/logo/light.svg" alt="wove" width="170" height="40">
</picture>

A Rust library to build terminal user interfaces.

Start with a few widgets. Arrange them with flex or grid, handle input, and let
wove draw the terminal. Build directly in Rust, or use the optional Dioxus adapter
for components and signals.

[Get started](docs/start.md) · [API](https://docs.rs/wove) · [Examples](crates/core/examples) · [crates.io](https://crates.io/crates/wove)

## Your first terminal app

With Rust 1.98 or newer, create a project and add wove:

```sh
cargo new hello
cd hello
cargo add wove
```

Put this in `src/main.rs`:

```rust
use wove::{terminal, widgets::{Input, Text}, Tree};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ui = Tree::new();
    ui.add(ui.root(), Text::new("Hello from wove. What's your name?"))?;
    let name = ui.add(ui.root(), Input::default())?;
    ui.focus(Some(name))?;

    terminal::run(&mut ui, |_, _, _| true)?;
    Ok(())
}
```

Run `cargo run` and start typing. Try selecting text with Shift + arrows or
undoing an edit with Ctrl + Z. Press Escape to exit.

## Build from here

- Compose text, inputs, lists, panels, and scrolling views.
- Move widgets around without resetting their input or selection.
- Choose your colors and layout, or implement `Widget` to draw something new.
- Test frames and input without opening a terminal.

The [guide](docs/start.md) covers layout, events, and headless testing.
If you prefer RSX and signals, see [Dioxus](docs/dioxus.md). The adapter is optional
and currently available from a source checkout.

To try the demos, run these from this repository:

```sh
cargo run -p wove --example gallery
cargo run -p wove-dioxus --example counter
```

The gallery has editable filtering, keyboard selection, and scrolling. The
counter shows how Dioxus signals and input events work together.

## Growing wove

wove is young, and its API is still changing. Multiline editing, rich text, and
virtualized lists are [ahead of us](docs/roadmap.md).

Building something with it? We'd like to hear what works and what gets in your
way. [Open an issue](https://github.com/intuitums/wove/issues) with an example,
or read [Contributing](CONTRIBUTING.md) to work on the library.

For a look inside, [core](crates/core) owns the widgets and terminal behavior;
[dioxus](crates/dioxus) adds the component adapter. The
[architecture guide](docs/architecture.md) explains how they fit together.
