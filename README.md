# wove

A Rust library to build terminal user interfaces.

wove owns a persistent tree of widgets. Layout, focus, input, and rendering work
without a component framework. An optional Dioxus adapter adds RSX, signals, and
component lifecycles over the same tree.

```text
crates/
  core/       widgets, layout, text editing, input, rendering, testing
  dioxus/     Dioxus adapter and terminal RSX elements
```

The design follows OpenTUI's separation between its core and framework adapters.
wove uses Rust widgets and Taffy layout, with no JavaScript runtime or native FFI
boundary in its own code. It is independent of any consuming application.

## Try it

```sh
cargo run -p wove --example gallery
cargo run -p wove-dioxus --example counter
```

The [captured gallery](docs/assets/gallery.txt) demonstrates filtering, selection,
borders, wrapping, and scrolling.
The counter demonstrates Dioxus signals and event cancellation. Both exit with
Escape or Ctrl-C.

## Use the core

```rust
use wove::{Tree, widgets::Text};

let mut tree = Tree::new();
let greeting = tree.add(tree.root(), Text::new("Hello, terminal"))?;
tree.update::<Text>(greeting, |text| text.content.push('!'))?;
let frame = tree.frame(80, 24)?;
# Ok::<(), wove::Error>(())
```

Widgets retain state when moved. Removing a subtree drops its widgets and
callbacks. Applications can implement `Widget` and use `Canvas` to paint inside
their allocated, clipped area. `Tree` is usable without terminal access through
`default-features = false`.

This is an early library with an unstable API. Current widgets are `Container`,
`Panel`, `Text`, `Input`, `Select`, and `Scroll`. Text editing supports grapheme
movement, selection, and undo. It does not yet include a multiline editor, rich
text spans, virtualized lists, or accessibility integration. Dioxus support is
optional and experimental.

[Start](docs/start.md) · [Architecture](docs/architecture.md) ·
[Dioxus](docs/dioxus.md) · [Contributing](CONTRIBUTING.md)
