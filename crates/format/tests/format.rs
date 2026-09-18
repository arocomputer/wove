use wove::{Color, Style};
use wove_format::{diff, markdown, Palette};
#[test]
fn markdown_keeps_nested_style_and_link_destination() {
    let rich = markdown(
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
#[test]
fn diff_separates_unterminated_replacement_lines_and_preserves_styles() {
    let red = Style {
        fg: Color::Indexed(1),
        ..Default::default()
    };
    let green = Style {
        fg: Color::Indexed(2),
        ..Default::default()
    };
    let result = diff("old", "new", Style::default(), green, red);
    assert_eq!(
        result
            .spans
            .iter()
            .map(|s| s.text.as_str())
            .collect::<String>(),
        "-old\n+new\n"
    );
    assert_eq!(result.spans[0].style, red);
    assert_eq!(result.spans[1].style, green);
}
