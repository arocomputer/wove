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
fn a_quote_before_a_loose_paragraph_does_not_hide_item_spacing() {
    use wove::markdown::{parse, Block};
    let source = "- > quoted\n\n  after\n\n- > next\n\n  tail";
    assert!(matches!(
        &parse(source)[0],
        Block::List { tight: false, .. }
    ));
    assert_eq!(
        plain(source),
        "• │ quoted\n  \n  after\n\n• │ next\n  \n  tail"
    );
    assert!(matches!(
        &parse("- > quoted\n- > next")[0],
        Block::List { tight: true, .. }
    ));
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

#[test]
fn a_rule_is_a_block_of_its_own_and_code_in_a_link_is_linked() {
    assert_eq!(plain("para\n\n---\n\nnext"), "para\n\n────\n\nnext");
    let rich = render("[`code`](https://example.com)", Palette::default());
    let code = rich.spans.iter().find(|s| s.text == "code").unwrap();
    assert_eq!(code.link.as_deref(), Some("https://example.com"));
}

#[test]
fn task_markers_replace_bullets_in_tight_and_loose_items() {
    assert_eq!(
        plain("- [ ] pending\n- [x] complete"),
        "[ ] pending\n[x] complete"
    );
    assert_eq!(
        plain("- [ ] pending\n\n- [x] complete"),
        "[ ] pending\n\n[x] complete"
    );
}

#[test]
fn table_cells_keep_emphasis_and_hyperlink_metadata() {
    let result = render(
        "| Name | Docs |\n|---|---|\n| **Wove** | [read](https://example.com) |",
        Palette::default(),
    );
    assert!(result
        .spans
        .iter()
        .any(|s| s.text == "Wove" && s.style.bold));
    assert!(result
        .spans
        .iter()
        .any(|s| s.text == "read" && s.link.as_deref() == Some("https://example.com")));
    assert!(result.spans.iter().any(|s| s.text.contains("-+-")));
}

#[test]
fn semantic_blocks_preserve_code_info_nesting_and_source_numbers() {
    use wove::markdown::{parse, Block};
    let blocks = parse("> 3. first\n> 3. second\n>\n> ```rust linenos\n> let x = 1;\n> ```");
    let Block::Quote(children) = &blocks[0] else {
        panic!("missing quote")
    };
    let Block::List { items, .. } = &children[0] else {
        panic!("missing list")
    };
    assert_eq!(
        items.iter().map(|item| item.number).collect::<Vec<_>>(),
        [Some(3), Some(3)]
    );
    assert!(
        matches!(&children[1], Block::Code { info, source } if info == "rust linenos" && source == "let x = 1;\n")
    );
}

#[test]
fn a_heading_override_and_hidden_destinations_preserve_nested_emphasis() {
    use wove::{
        markdown::{render_with, Options},
        Color, Style,
    };
    let heading = Style {
        fg: Color::Indexed(5),
        underline: true,
        ..Style::default()
    };
    let mut palette = Palette::default();
    palette.headings[1] = Some(heading);
    let rich = render_with(
        "## **bold** [*linked*](https://example.com)",
        palette,
        Options {
            link_destinations: false,
            ..Options::default()
        },
    );
    let bold = rich.spans.iter().find(|s| s.text == "bold").unwrap();
    assert!(bold.style.bold && bold.style.underline);
    assert_eq!(bold.style.fg, Color::Indexed(5));
    let link = rich.spans.iter().find(|s| s.text == "linked").unwrap();
    assert!(link.style.italic && link.style.underline);
    assert_eq!(link.link.as_deref(), Some("https://example.com"));
    assert!(!rich.spans.iter().any(|s| s.text.contains("https://")));
}

#[test]
fn quoted_wrapped_links_keep_their_prefix_and_destination_on_every_row() {
    use wove::{
        markdown::{render_with, Options},
        testing::Screen,
    };
    let mut screen = Screen::new(10, 6);
    let rich = render_with(
        "> [alpha beta gamma](https://example.com)",
        Palette::default(),
        Options {
            width: Some(10),
            link_destinations: false,
            ..Options::default()
        },
    );
    screen.tree.add(screen.tree.root(), rich).unwrap();
    let frame = screen.frame().unwrap();
    assert_eq!(
        &frame.lines()[..3],
        &["│ alpha   ", "│ beta    ", "│ gamma   "]
    );
    for row in 0..3 {
        assert_eq!(
            frame.cell(2, row).unwrap().link(),
            Some("https://example.com")
        );
        assert_eq!(frame.cell(0, row).unwrap().link(), None);
    }
}

#[test]
fn tables_align_and_clip_wide_cells_within_the_given_width() {
    use wove::{
        markdown::{render_with, Options},
        testing::Screen,
    };
    let mut screen = Screen::new(12, 4);
    let rich = render_with(
        "| L | R |\n|:---|---:|\n| 界界界 | 7 |",
        Palette::default(),
        Options {
            width: Some(12),
            ..Options::default()
        },
    );
    screen.tree.add(screen.tree.root(), rich).unwrap();
    let frame = screen.frame().unwrap();
    assert_eq!(frame.lines()[2], "界界界 | 7  ");
    assert_eq!(frame.cell(1, 2).unwrap().symbol(), "");
}

#[test]
fn strikethrough_and_inline_code_keep_enclosing_emphasis() {
    let rich = render("**~~gone~~ `code`**", Palette::default());
    let gone = rich.spans.iter().find(|s| s.text == "gone").unwrap();
    assert!(gone.style.bold && gone.style.strikethrough);
    let code = rich.spans.iter().find(|s| s.text == "code").unwrap();
    assert!(code.style.bold);
}

#[test]
fn excessive_source_nesting_stays_literal_instead_of_recursing_without_bound() {
    use wove::markdown::{parse, Block, Inline};
    let source = format!("{}text", "> ".repeat(1024));
    assert_eq!(
        parse(&source),
        [Block::Paragraph(vec![Inline::Text(source)])]
    );
}

#[test]
fn ordered_lists_display_consecutive_numbers_while_parsing_retains_source() {
    assert_eq!(plain("3. a\n3. b"), "3. a\n4. b");
}

#[test]
fn raw_html_keeps_its_source_kind_for_custom_formatters() {
    use wove::markdown::{parse, Block, Inline};
    assert_eq!(
        parse("before <b>after</b>"),
        [Block::Paragraph(vec![
            Inline::Text("before ".into()),
            Inline::Html("<b>".into()),
            Inline::Text("after".into()),
            Inline::Html("</b>".into()),
        ])]
    );
    assert_eq!(plain("before <b>after</b>"), "before <b>after</b>");
    assert_eq!(
        parse("<div>block</div>"),
        [Block::Html("<div>block</div>".into())]
    );
    assert_eq!(plain("<div>block</div>"), "<div>block</div>");
}

#[test]
fn multiline_html_remains_one_source_block_without_added_spacing() {
    use wove::markdown::{parse, Block};
    let source = "<div>\nhello\n</div>\n";
    assert_eq!(parse(source), [Block::Html(source.into())]);
    assert_eq!(plain(source), source.trim_end_matches('\n'));
}
