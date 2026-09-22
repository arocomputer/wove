#![cfg(feature = "markdown")]
use wove::markdown::{render, Palette};
#[test]
fn markdown_keeps_nested_style_and_link_destination() {
    let rich = render(
        "**bold *nested* end** [label](https://example.com)",
        Palette::default(),
    );
    let nested = rich.spans.iter().find(|s| s.text == "nested").unwrap();
    assert!(nested.style.bold && nested.style.italic);
    let end = rich.spans.iter().find(|s| s.text == " end").unwrap();
    assert!(end.style.bold && !end.style.italic);
    let label = rich.spans.iter().find(|s| s.text == "label").unwrap();
    assert_eq!(label.link.as_deref(), Some("https://example.com"));
    assert!(rich
        .spans
        .iter()
        .any(|s| s.text == " (https://example.com)"));
}

fn plain(source: &str) -> String {
    render(source, Palette::default())
        .spans
        .iter()
        .map(|s| s.text.as_str())
        .collect()
}

#[test]
fn list_items_take_one_row_each_with_nested_items_indented() {
    assert_eq!(plain("- a\n  - b\n- c"), "• a\n  • b\n• c");
    assert_eq!(plain("1. a\n2. b"), "1. a\n2. b");
}

#[test]
fn loose_list_items_are_separated_by_one_blank_row() {
    assert_eq!(plain("- a\n\n- b"), "• a\n\n• b");
}

#[test]
fn blocks_are_separated_by_one_blank_row_and_nothing_trails_the_last() {
    assert_eq!(
        plain("# Title\n\n```\ncode\n```\n\ntext\n"),
        "Title\n\ncode\n\ntext"
    );
    assert_eq!(plain("- a\n\nafter"), "• a\n\nafter");
}

#[test]
fn a_link_that_shows_its_destination_does_not_repeat_it() {
    assert_eq!(plain("<https://example.com>"), "https://example.com");
}
