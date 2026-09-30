//! Parse once and reformat styled Markdown at the available terminal width.
use wove::{
    elements::RichText,
    markdown::{parse, render_blocks, Options, Palette},
    terminal::{self, Terminal},
    Color, Event, Key, Style, Tree,
};

const SOURCE: &str = r#"# Markdown

**Bold**, *italic*, ~~struck~~, and `inline code`.

## Lists and quotes

- [ ] An unfinished task
- [x] A finished task
  - A nested item

> A [linked quotation](https://example.com) wraps with its prefix and destination intact.

| Language | Status | Count |
|:---|:---:|---:|
| **Rust** | Ready | 12 |
| 日本語 | Ready | 3 |

```rust linenos
let document = parse(source);
```

Resize to reflow; Esc quits."#;

/// Keep terminal ownership in the application; the formatter returns an element.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let document = parse(SOURCE);
    let palette = Palette {
        heading: Style {
            bold: true,
            ..Style::default()
        },
        link: Style {
            fg: Color::Indexed(4),
            underline: true,
            ..Style::default()
        },
        table_header: Style {
            bold: true,
            ..Style::default()
        },
        ..Palette::default()
    };
    let mut tree = Tree::new();
    let content = tree.add(tree.root(), RichText::default())?;
    let mut terminal = Terminal::new()?;
    let mut width = None;
    let mut typed = terminal.typed_ahead().into_iter();
    loop {
        let (cols, rows) = terminal.size()?;
        if width != Some(cols) {
            tree.update::<RichText>(content, |text| {
                *text = render_blocks(
                    &document,
                    palette,
                    Options {
                        width: Some(cols),
                        link_destinations: false,
                        ..Options::default()
                    },
                );
            })?;
            width = Some(cols);
        }
        terminal.draw(tree.frame(cols, rows)?)?;
        let event = match typed.next() {
            Some(event) => Some(event),
            None => terminal::read()?,
        };
        match event {
            Some(Event::Key(Key::Escape, _)) => break,
            Some(Event::Resize(..)) => terminal.invalidate(),
            _ => {}
        }
    }
    Ok(())
}
