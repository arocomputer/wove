# wove

A Rust library to build terminal user interfaces.

wove owns a persistent tree of widgets with flex and grid layout, focus, input
routing, Unicode text editing, and clipped scrolling. The core works directly
from Rust; an optional Dioxus adapter lives in a separate workspace crate.

```toml
[dependencies]
wove = "0.2"
```

```rust
use wove::{Tree, widgets::{Input, Text}, terminal};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut tree = Tree::new();
    tree.add(tree.root(), Text::new("Your name"))?;
    let input = tree.add(tree.root(), Input::default())?;
    tree.focus(Some(input))?;
    terminal::run(&mut tree, |_, _, _| true)?;
    Ok(())
}
```

Disable default features for headless use. `Tree::frame` returns a cell buffer
without acquiring a terminal. Custom widgets implement `Widget` and paint through
a clipped `Canvas`. Moving a widget preserves its state; removing it drops its
subtree and callbacks.

This is an early release with an unstable API. Current widgets are `Container`,
`Panel`, `Text`, `Input`, `Select`, and `Scroll`. Multiline editing, rich text spans,
and virtualized lists are not implemented yet.

[API documentation](https://docs.rs/wove) ·
[Source and examples](https://github.com/intuitums/wove/tree/chore/wove) ·
[Guide](https://github.com/intuitums/wove/blob/chore/wove/docs/start.md)
