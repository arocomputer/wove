# Start

Use Rust 1.98 or newer. The core is published as [`wove`](https://crates.io/crates/wove).
The optional `wove-dioxus` adapter currently requires a local checkout.
Their Rust imports are `wove` and `wove_dioxus`.

```toml
[dependencies]
wove = { git = "https://github.com/intuitums/wove", branch = "main", version = "0.0.1" }
```

## Build a tree

```rust
use wove::{Tree, elements::{Input, Text}, terminal};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut tree = Tree::new();
    tree.add(tree.root(), Text::new("Your name"))?;
    let input = tree.add(tree.root(), Input::default())?;
    tree.focus(Some(input))?;
    terminal::run(&mut tree, |_, _, _| true)?;
    Ok(())
}
```

`add` attaches an element; `create` leaves it detached. `append` and `insert` move
existing elements without losing state. `update` changes typed element state and
invalidates layout and paint. `remove` drops a subtree. IDs cannot be reused or
passed to another tree.

The root fills the frame and arranges children in a column. Set a node's `Layout`
using the types in `wove::layout` for flex, grid, spacing, and alignment.
Lengths are terminal cells. `Panel` reserves its border in its default layout;
when replacing a layout, start from `tree.layout(id)?.clone()` to retain defaults.
Text defaults to clipping. Set `Text::wrap` for hard wrapping at grapheme boundaries.

## Input and rendering

`Tree::on` runs before an element's default behavior. `Response::HANDLED` consumes
the event; otherwise it bubbles through parents. Tab and Shift-Tab move focus
when no handler consumes the key. Mouse presses focus the nearest focusable
ancestor of the hit element. Hit testing uses the last painted frame.

`Input` supports arrows, Shift selection, Home, End, Backspace, Delete, Ctrl-A,
Ctrl-Z, Ctrl-Y, and bracketed paste. Programmatic changes through its public
editor should contain single-line text. `Editor` itself accepts arbitrary text;
it is also available to custom elements.

`Scroll` clips children and handles arrows, paging, Home, End, and mouse wheels.
Children that should retain their full content height need `flex_shrink: 0.0`.
End enables following; moving away from the end stops it.

`frame` recomputes after updates or resize and otherwise returns the cached frame.
`terminal::Renderer` writes changed cells and cursor position. `terminal::run`
is a blocking convenience loop with Escape and Ctrl-C exit behavior. Applications
with timers or background work can use `Terminal`, `read`, and `poll` in their
own loop. Do not print over a live session without invalidating its renderer.

## Test without a terminal

```rust
use wove::{testing::Screen, elements::Input, Key};

let mut screen = Screen::new(20, 3);
let input = screen.tree.add(screen.tree.root(), Input::default())?;
screen.tree.focus(Some(input))?;
screen.send(Key::Char('界'))?;
assert_eq!(screen.frame()?.cell(0, 0).unwrap().symbol(), "界");
# Ok::<(), wove::Error>(())
```

Headless tests use the same layout, elements, clipping, and input routing as a
terminal session. Real PTY checks run through `./x ui` on Unix.
