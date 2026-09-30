//! Markdown parsing and a plain formatter, independent of terminal ownership.
//!
//! Use `parse` when your application controls block presentation. `render` and
//! `render_with` provide styled text without requiring a tree or terminal.
mod format;
mod parse;
pub use parse::{parse, Alignment, Block, Inline, ListItem};

use crate::{elements::RichText, text::Wrap, Style};

/// Complete styles for Markdown roles. A heading override applies only at its
/// level; otherwise `heading` is used. Inline emphasis augments the block style.
#[derive(Clone, Copy, Default)]
pub struct Palette {
    pub text: Style,
    pub heading: Style,
    pub headings: [Option<Style>; 6],
    pub code: Style,
    pub link: Style,
    pub marker: Style,
    pub quote: Style,
    pub rule: Style,
    pub table_header: Style,
    pub table_border: Style,
}

/// Choices for the supplied text formatter. Custom block layouts use `parse`
/// directly. With a width, wrapping and table sizing happen before returning
/// the element; call again when the available width changes.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub width: Option<u16>,
    pub link_destinations: bool,
    pub rule_width: u16,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            width: None,
            link_destinations: true,
            rule_width: 4,
        }
    }
}

/// Format Markdown with default presentation. HTML remains literal text.
/// Links retain their destinations as metadata and also show them as text.
pub fn render(source: &str, palette: Palette) -> RichText {
    render_with(source, palette, Options::default())
}

/// Parse and format with application-selected styles and presentation options.
pub fn render_with(source: &str, palette: Palette, options: Options) -> RichText {
    render_blocks(&parse(source), palette, options)
}

/// Format parsed blocks, which callers may inspect or change without reparsing.
/// Tables retain inline styles and hyperlinks, and size columns to the width.
pub fn render_blocks(blocks: &[Block], palette: Palette, options: Options) -> RichText {
    RichText::new(
        format::blocks(blocks, palette, options, false),
        if options.width.is_some() {
            Wrap::None
        } else {
            Wrap::Word
        },
    )
}

/// Format inline content independently, for custom headings, tables, and code
/// panels. `style` is the surrounding block's style; role colors come from
/// `palette`, while emphasis and enclosing link destinations remain intact.
pub fn render_inline(
    content: &[Inline],
    style: Style,
    palette: Palette,
    options: Options,
) -> Vec<crate::text::Span> {
    format::inlines(content, style, palette, options)
}
