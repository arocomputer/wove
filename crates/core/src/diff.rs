use crate::{
    elements::RichText,
    text::{Span, Wrap},
    Style,
};
use similar::{ChangeTag, TextDiff};

/// Produce a line diff with explicit addition and deletion styles, one row per
/// line. Unchanged lines retain the base style; missing final newlines remain
/// separate rows, and no empty row trails the last line.
pub fn render(before: &str, after: &str, base: Style, added: Style, removed: Style) -> RichText {
    let changes = TextDiff::from_lines(before, after);
    let mut spans: Vec<Span> = changes
        .iter_all_changes()
        .map(|change| {
            let (prefix, style) = match change.tag() {
                ChangeTag::Insert => ("+", added),
                ChangeTag::Delete => ("-", removed),
                ChangeTag::Equal => (" ", base),
            };
            let value = change.value();
            Span::new(
                format!(
                    "{prefix}{value}{}",
                    if value.ends_with('\n') { "" } else { "\n" }
                ),
                style,
            )
        })
        .collect();
    // Every line ends in a newline, which after the last would lay out as an empty row.
    if let Some(last) = spans.last_mut() {
        last.text.pop();
    }
    RichText::new(spans, Wrap::None)
}
