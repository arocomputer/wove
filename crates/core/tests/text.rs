use weft_core::{text::Editor, Buffer, Style};
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
    b.write(weft_core::Rect::new(1, 0, 3, 1), "a", Style::default());
    assert_eq!(b.cell(0, 0).unwrap().symbol(), " ");
    assert_eq!(b.cell(1, 0).unwrap().symbol(), "a");
    assert_eq!(b.cell(2, 0).unwrap().symbol(), "x");
}
