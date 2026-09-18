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
    assert!(rich
        .spans
        .iter()
        .any(|s| s.text == " (https://example.com)"));
}
