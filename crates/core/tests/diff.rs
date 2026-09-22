#![cfg(feature = "diff")]
use wove::{diff::render, Color, Style};
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
    let result = render("old", "new", Style::default(), green, red);
    assert_eq!(
        result
            .spans
            .iter()
            .map(|s| s.text.as_str())
            .collect::<String>(),
        "-old\n+new"
    );
    assert_eq!(result.spans[0].style, red);
    assert_eq!(result.spans[1].style, green);
}
