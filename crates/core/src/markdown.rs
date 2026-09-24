use crate::{
    elements::RichText,
    text::{Span, Wrap},
    Style,
};
use pulldown_cmark::{Event, Parser, Tag, TagEnd};

/// Complete styles for Markdown roles, chosen by the application.
#[derive(Clone, Copy, Default)]
pub struct Palette {
    pub text: Style,
    pub heading: Style,
    pub code: Style,
    pub link: Style,
}

/// Render CommonMark as styled text. HTML remains literal text. Link text opens
/// its destination in terminals with hyperlinks and is followed by the
/// destination in parentheses, unless the text already is the destination.
/// Block layout is linear: one blank row separates blocks, list items take a
/// row each with nested items indented, and no newline trails the last block.
pub fn render(source: &str, palette: Palette) -> RichText {
    let mut document = Document {
        palette,
        spans: Vec::new(),
        styles: vec![palette.text],
        links: Vec::new(),
        lists: Vec::new(),
    };
    for event in Parser::new(source) {
        document.event(event);
    }
    // A trailing newline would lay out as an empty row.
    let mut spans = document.spans;
    while let Some(last) = spans.last_mut() {
        last.text.truncate(last.text.trim_end_matches('\n').len());
        if !last.text.is_empty() {
            break;
        }
        spans.pop();
    }
    RichText::new(spans, Wrap::Word)
}

/// The text so far and the blocks open around the parser's position.
struct Document {
    palette: Palette,
    spans: Vec<Span>,
    /// The style of each open inline or block, innermost last.
    styles: Vec<Style>,
    /// Each open link's destination and the text shown for it so far.
    links: Vec<(String, String)>,
    /// Each open list's next number, or `None` for bullets.
    lists: Vec<Option<u64>>,
}

impl Document {
    fn current(&self) -> Style {
        *self.styles.last().unwrap_or(&self.palette.text)
    }

    fn event(&mut self, event: Event<'_>) {
        let current = self.current();
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => {
                self.spans.push(match self.links.last_mut() {
                    Some((url, shown)) => {
                        shown.push_str(&text);
                        Span::link(text.into_string(), current, url.as_str())
                    }
                    None => Span::new(text.into_string(), current),
                });
            }
            Event::Code(text) => {
                let code = self.palette.code;
                self.spans.push(match self.links.last_mut() {
                    Some((url, shown)) => {
                        shown.push_str(&text);
                        Span::link(text.into_string(), code, url.as_str())
                    }
                    None => Span::new(text.into_string(), code),
                });
            }
            Event::SoftBreak => self.spans.push(Span::new(" ", current)),
            Event::HardBreak => self.spans.push(Span::new("\n", current)),
            Event::Rule => {
                self.spans.push(Span::new("────", current));
                end_rows(&mut self.spans, 2, self.palette.text);
            }
            _ => {}
        }
    }

    /// Open a block or inline: push its style, and write what leads it.
    fn start(&mut self, tag: Tag<'_>) {
        let palette = self.palette;
        let current = self.current();
        let mut next = current;
        match tag {
            Tag::Heading { .. } => next = palette.heading,
            Tag::Strong => next.bold = true,
            Tag::Emphasis => next.italic = true,
            Tag::CodeBlock(_) => next = palette.code,
            Tag::Link { dest_url, .. } => {
                self.links.push((dest_url.to_string(), String::new()));
                next = palette.link;
            }
            Tag::List(start) => self.lists.push(start),
            Tag::Item => {
                if !self.spans.is_empty() {
                    end_rows(&mut self.spans, 1, palette.text);
                }
                let marker = self.item_marker();
                self.spans.push(Span::new(marker, current));
            }
            Tag::BlockQuote(_) => self.spans.push(Span::new("│ ", current)),
            _ => {}
        }
        self.styles.push(next);
    }

    /// The indent and number or bullet of the next item in the innermost list.
    fn item_marker(&mut self) -> String {
        let indent = "  ".repeat(self.lists.len().saturating_sub(1));
        match self.lists.last_mut() {
            Some(Some(number)) => {
                let marker = format!("{indent}{number}. ");
                *number += 1;
                marker
            }
            _ => format!("{indent}• "),
        }
    }

    /// Close a block or inline: pop its style, and write what follows it.
    fn end(&mut self, tag: TagEnd) {
        let palette = self.palette;
        self.styles.pop();
        match tag {
            TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::CodeBlock | TagEnd::BlockQuote(_) => {
                end_rows(&mut self.spans, 2, palette.text);
            }
            TagEnd::Item => end_rows(&mut self.spans, 1, palette.text),
            TagEnd::List(_) => {
                self.lists.pop();
                if self.lists.is_empty() {
                    end_rows(&mut self.spans, 2, palette.text);
                }
            }
            TagEnd::Link => {
                if let Some((url, text)) = self.links.pop() {
                    if text != url {
                        self.spans
                            .push(Span::new(format!(" ({url})"), palette.link));
                    }
                }
            }
            _ => {}
        }
    }
}

/// End the text with at least `count` newlines, adding only those missing, so
/// the ends of nested blocks never stack blank rows.
fn end_rows(spans: &mut Vec<Span>, count: usize, style: Style) {
    let mut have = 0;
    for span in spans.iter().rev() {
        let trimmed = span.text.trim_end_matches('\n');
        have += span.text.len() - trimmed.len();
        if !trimmed.is_empty() {
            break;
        }
    }
    if have < count {
        spans.push(Span::new("\n".repeat(count - have), style));
    }
}
