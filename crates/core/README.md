# Wove

A Rust library for building terminal user interfaces.

```rust
use wove::{elements::{Input, Text}, terminal, Key, Tree};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut tree = Tree::new();
    tree.add(tree.root(), Text::new("What's your name?"))?;
    let input = tree.add(tree.root(), Input::default())?;
    tree.focus(Some(input))?;
    terminal::run(&mut tree, |_, event, _| *event != Key::Escape.into())?;
    Ok(())
}
```

Install from Git, since Wove is not on crates.io yet:

```sh
cargo add wove --git https://github.com/arocomputer/wove
```

Then build your first app with the [quickstart](https://wovetui.com/docs/quickstart/).

## Features

Only `terminal` is on by default. Each of these works on its own:

| Feature | Adds |
| --- | --- |
| `terminal` | Running on the local terminal |
| `markdown` | `wove::markdown`, Markdown as styled text |
| `syntax` | `wove::syntax`, syntax highlighting |
| `diff` | `wove::diff`, styled line diffs |

Turn off default features to render without a terminal, for tests or custom
transports.
