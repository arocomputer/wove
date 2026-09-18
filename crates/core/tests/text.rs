use wove::{text::Editor, Buffer, Style};
#[test]
fn selection_replaces_whole_graphemes_and_undo_restores_it() {
    let mut e = Editor::new("a👩‍💻e\u{301}");
    e.left(true);
    e.left(true);
    assert_eq!(&e.text()[e.selection()], "👩‍💻e\u{301}");
    e.insert("界");
    assert_eq!(e.text(), "a界");
    e.undo();
    assert_eq!(&e.text()[e.selection()], "👩‍💻e\u{301}");
    e.redo();
    assert_eq!(e.text(), "a界");
}
#[test]
fn insertion_that_joins_a_cluster_keeps_cursor_on_a_boundary() {
    let mut e = Editor::new("👩💻");
    e.left(false);
    e.insert("\u{200d}");
    assert_eq!(e.cursor(), e.text().len());
    e.backspace();
    assert_eq!(e.text(), "");
}
#[test]
fn overwriting_a_wide_continuation_erases_the_entire_glyph() {
    let mut b = Buffer::new(4, 1);
    let area = b.area();
    b.write(area, "界x", Style::default());
    b.write(wove::Rect::new(1, 0, 3, 1), "a", Style::default());
    assert_eq!(b.cell(0, 0).unwrap().symbol(), " ");
    assert_eq!(b.cell(1, 0).unwrap().symbol(), "a");
    assert_eq!(b.cell(2, 0).unwrap().symbol(), "x");
}

#[test]
fn vertical_movement_preserves_display_column_through_short_lines() {
    let mut editor = Editor::new("界ab\nx\n界cd");
    editor.vertical(-1, false);
    assert_eq!(editor.cursor(), "界ab\nx".len());
    editor.vertical(-1, true);
    assert_eq!(editor.cursor(), "界ab".len());
    assert_eq!(&editor.text()[editor.selection()], "\nx");
}

#[test]
fn undo_tracks_joined_grapheme_edits_and_discards_redo_after_new_input() {
    let mut editor = Editor::new("👩💻");
    editor.left(false);
    editor.insert("\u{200d}");
    editor.undo();
    assert_eq!(editor.text(), "👩💻");
    editor.redo();
    assert_eq!(editor.text(), "👩‍💻");
    editor.undo();
    editor.insert(" ");
    editor.redo();
    assert_eq!(editor.text(), "👩 💻");
}

#[test]
fn textarea_paste_navigation_and_selection_paint_use_logical_lines() {
    use wove::{elements::Textarea, testing::Screen, Event, Key, Modifiers};
    let mut screen = Screen::new(8, 3);
    let id = screen
        .tree
        .add(screen.tree.root(), Textarea::default())
        .unwrap();
    screen.tree.focus(Some(id)).unwrap();
    screen.send(Event::Paste("a\r\n界b\nc".into())).unwrap();
    screen
        .send(Event::Key(
            Key::Up,
            Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        ))
        .unwrap();
    let frame = screen.frame().unwrap();
    assert!(frame.cell(0, 1).unwrap().style().reverse);
    assert_eq!(frame.cursor(), Some((0, 1)));
    screen
        .send(Event::Key(
            Key::Char('z'),
            Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
        ))
        .unwrap();
    assert_eq!(screen.tree.get::<Textarea>(id).unwrap().editor.text(), "");
}

#[test]
fn rich_text_wraps_words_without_splitting_clusters_at_span_boundaries() {
    use wove::{
        elements::RichText,
        testing::Screen,
        text::{Span, Wrap},
    };
    let bold = Style {
        bold: true,
        ..Style::default()
    };
    let mut screen = Screen::new(6, 3);
    let id = screen
        .tree
        .add(
            screen.tree.root(),
            RichText {
                spans: vec![
                    Span::new("hi e", bold),
                    Span::new("\u{301} world", Style::default()),
                ],
                wrap: Wrap::Word,
            },
        )
        .unwrap();
    let mut layout = screen.tree.layout(id).unwrap().clone();
    layout.size.width = wove::layout::length(6.0);
    screen.tree.set_layout(id, layout).unwrap();
    let frame = screen.frame().unwrap();
    assert_eq!(frame.cell(3, 0).unwrap().symbol(), "e\u{301}");
    assert!(frame.cell(3, 0).unwrap().style().bold);
    assert_eq!(frame.cell(0, 1).unwrap().symbol(), "w");
}

#[test]
fn logical_line_navigation_never_stops_inside_a_crlf_grapheme() {
    let mut editor = Editor::new("a\r\nb");
    editor.home(false);
    editor.line_end(false);
    assert_eq!(editor.cursor(), 1);
    editor.vertical(1, false);
    assert_eq!(editor.cursor(), 4);
    editor.vertical(-1, false);
    assert_eq!(editor.cursor(), 1);
}
