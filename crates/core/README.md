# wove

A Rust library to build terminal user interfaces.

Start with a few widgets. Arrange them with flex or grid, handle input, and let
wove draw the terminal. Build directly in Rust, or use the optional Dioxus adapter
for components and signals.

```toml
[dependencies]
wove = "0.2"
```

```rust
use wove::{Tree, widgets::{Input, Text}, terminal};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut tree = Tree::new();
    tree.add(tree.root(), Text::new("Hello from wove. What's your name?"))?;
    let input = tree.add(tree.root(), Input::default())?;
    tree.focus(Some(input))?;
    terminal::run(&mut tree, |_, _, _| true)?;
    Ok(())
}
```

Run your app and start typing. Shift + arrows selects text, Ctrl + Z undoes an
edit, and Escape exits.

Disable default features for headless use. `Tree::frame` returns a cell buffer
without acquiring a terminal. Custom widgets implement `Widget` and paint through
a clipped `Canvas`. Moving a widget preserves its state; removing it drops its
subtree and callbacks.

This is an early release with an unstable API. Current widgets are `Container`,
`Panel`, `Text`, `Input`, `Select`, and `Scroll`. Multiline editing, rich text spans,
and virtualized lists are not implemented yet.

[API documentation](https://docs.rs/wove) ·
[Source and examples](https://github.com/intuitums/wove/tree/main) ·
[Guide](https://github.com/intuitums/wove/blob/main/docs/start.md)
