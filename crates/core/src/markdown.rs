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
    let mut spans = Vec::new();
    let mut styles = vec![palette.text];
    // Each open link's destination and the text shown for it so far.
    let mut links: Vec<(String, String)> = Vec::new();
    let mut lists: Vec<Option<u64>> = Vec::new();
    for event in Parser::new(source) {
        let current = *styles.last().unwrap_or(&palette.text);
        match event {
            Event::Start(tag) => {
                let mut next = current;
                match tag {
                    Tag::Heading { .. } => next = palette.heading,
                    Tag::Strong => next.bold = true,
                    Tag::Emphasis => next.italic = true,
                    Tag::CodeBlock(_) => next = palette.code,
                    Tag::Link { dest_url, .. } => {
                        links.push((dest_url.to_string(), String::new()));
                        next = palette.link;
                    }
                    Tag::List(start) => lists.push(start),
                    Tag::Item => {
                        if !spans.is_empty() {
                            end_rows(&mut spans, 1, palette.text);
                        }
                        let indent = "  ".repeat(lists.len().saturating_sub(1));
                        let prefix = match lists.last_mut() {
                            Some(Some(n)) => {
                                let s = format!("{indent}{n}. ");
                                *n += 1;
                                s
                            }
                            _ => format!("{indent}• "),
                        };
                        spans.push(Span::new(prefix, current));
                    }
                    Tag::BlockQuote(_) => spans.push(Span::new("│ ", current)),
                    _ => {}
                }
                styles.push(next);
            }
            Event::End(tag) => {
                styles.pop();
                match tag {
                    TagEnd::Paragraph
                    | TagEnd::Heading(_)
                    | TagEnd::CodeBlock
                    | TagEnd::BlockQuote(_) => end_rows(&mut spans, 2, palette.text),
                    TagEnd::Item => end_rows(&mut spans, 1, palette.text),
                    TagEnd::List(_) => {
                        lists.pop();
                        if lists.is_empty() {
                            end_rows(&mut spans, 2, palette.text);
                        }
                    }
                    TagEnd::Link => {
                        if let Some((url, text)) = links.pop() {
                            if text != url {
                                spans.push(Span::new(format!(" ({url})"), palette.link));
                            }
                        }
                    }
                    _ => {}
                }
            }
            Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => {
                spans.push(match links.last_mut() {
                    Some((url, shown)) => {
                        shown.push_str(&text);
                        Span::link(text.into_string(), current, url.as_str())
                    }
                    None => Span::new(text.into_string(), current),
                })
            }
            Event::Code(text) => {
                if let Some((_, shown)) = links.last_mut() {
                    shown.push_str(&text);
                }
                spans.push(Span::new(text.into_string(), palette.code))
            }
            Event::SoftBreak => spans.push(Span::new(" ", current)),
            Event::HardBreak => spans.push(Span::new("\n", current)),
            Event::Rule => spans.push(Span::new("────\n", current)),
            _ => {}
        }
    }
    // A trailing newline would lay out as an empty row.
    while let Some(last) = spans.last_mut() {
        last.text.truncate(last.text.trim_end_matches('\n').len());
        if !last.text.is_empty() {
            break;
        }
        spans.pop();
    }
    RichText::new(spans, Wrap::Word)
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
