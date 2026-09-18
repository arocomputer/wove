use similar::{ChangeTag, TextDiff};
use wove::{
    elements::RichText,
    text::{Span, Wrap},
    Style,
};

/// Produce a line diff with explicit addition and deletion styles. Unchanged
/// lines retain the base style; missing final newlines remain separate rows.
pub fn diff(before: &str, after: &str, base: Style, added: Style, removed: Style) -> RichText {
    let changes = TextDiff::from_lines(before, after);
    let spans = changes
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
    RichText {
        spans,
        wrap: Wrap::None,
    }
}
