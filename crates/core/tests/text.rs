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

#[test]
fn undo_deletion_restores_the_original_cursor_without_a_selection() {
    let mut editor = Editor::new("a界b");
    editor.left(false);
    let cursor = editor.cursor();
    editor.backspace();
    assert_eq!(editor.text(), "ab");
    editor.undo();
    assert_eq!(editor.text(), "a界b");
    assert_eq!(editor.cursor(), cursor);
    assert!(editor.selection().is_empty());
    editor.redo();
    assert_eq!(editor.text(), "ab");

    editor.home(false);
    editor.delete();
    editor.undo();
    assert_eq!(editor.text(), "ab");
    assert_eq!(editor.cursor(), 0);
    assert!(editor.selection().is_empty());
}

#[test]
fn word_wrap_rechecks_a_wide_grapheme_after_moving_the_word() {
    use wove::{
        elements::RichText,
        testing::Screen,
        text::{Span, TextLayout, Wrap},
    };
    let spans = vec![Span::new(" aab界", Style::default())];
    let layout = TextLayout::new(&spans, Some(4), Wrap::Word);
    assert_eq!(layout.size(), (3, 3));
    let mut screen = Screen::new(4, 3);
    let id = screen
        .tree
        .add(
            screen.tree.root(),
            RichText {
                spans,
                wrap: Wrap::Word,
            },
        )
        .unwrap();
    let mut style = screen.tree.layout(id).unwrap().clone();
    style.size.width = wove::layout::length(4.0);
    screen.tree.set_layout(id, style).unwrap();
    let frame = screen.frame().unwrap();
    assert_eq!(frame.cell(0, 1).unwrap().symbol(), "a");
    assert_eq!(frame.cell(0, 2).unwrap().symbol(), "界");
}

#[test]
fn clicking_an_input_reports_a_focus_repaint() {
    use wove::{elements::Input, testing::Screen, Event, Mouse, MouseKind};
    let mut screen = Screen::new(10, 2);
    let id = screen
        .tree
        .add(screen.tree.root(), Input::default())
        .unwrap();
    screen.frame().unwrap();
    let event = Event::Mouse(Mouse {
        x: 0,
        y: 0,
        kind: MouseKind::Down,
    });
    let result = screen.send(event.clone()).unwrap();
    assert_eq!(screen.tree.focused(), Some(id));
    assert!(result.changed);
    screen.frame().unwrap();
    assert!(!screen.send(event).unwrap().changed);
}
