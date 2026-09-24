#![cfg(feature = "syntax")]
use wove::syntax::{Highlighter, Syntaxes, ThemeSet};
use wove::Element;

#[test]
fn a_source_ending_in_a_newline_lays_out_no_empty_row() {
    let themes = ThemeSet::load_defaults();
    let highlighter = Highlighter::new(
        Syntaxes::load_defaults_newlines(),
        themes.themes["base16-ocean.dark"].clone(),
    );
    let rich = highlighter.highlight("fn main() {}\n", "rs").unwrap();
    assert_eq!(rich.measure(None), (12, 1));
    assert!(rich.spans.iter().all(|span| !span.text.ends_with('\n')));
}
