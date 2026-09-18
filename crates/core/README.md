# Wove

A Rust library to build terminal user interfaces.

Start with a few elements. Arrange them with flex or grid, handle input, and let
Wove draw the terminal. Build directly in Rust, or use the optional Dioxus adapter
for components and signals.

```toml
[dependencies]
wove = { git = "https://github.com/intuitums/wove", branch = "main", version = "0.0.1" }
```

```rust
use wove::{Tree, elements::{Input, Text}, terminal};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut tree = Tree::new();
    tree.add(tree.root(), Text::new("Hello from Wove. What's your name?"))?;
    let input = tree.add(tree.root(), Input::default())?;
    tree.focus(Some(input))?;
    terminal::run(&mut tree, |_, _, _| true)?;
    Ok(())
}
```

Run your app and start typing. Shift + arrows selects text, Ctrl + Z undoes an
edit, and Escape exits.

Disable default features for headless use. `Tree::frame` returns a cell buffer
without acquiring a terminal. Custom elements implement `Element` and paint through
a clipped `Canvas`. Moving an element preserves its state; removing it drops its
subtree and callbacks.

This is an early release with an unstable API. Current elements are `Container`,
`Panel`, `Text`, `RichText`, `Input`, `Textarea`, `Select`, `List`, `Table`, and
`Scroll`. `List` requests only visible rows from its provider. `Textarea` supports
logical lines and horizontal scrolling; soft-wrapped editing is not implemented.

[Source and examples](https://github.com/intuitums/wove/tree/main) ·
[Guide](https://github.com/intuitums/wove/blob/main/crates/web/src/content/docs/start.mdx)
