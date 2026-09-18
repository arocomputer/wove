use std::{cell::Cell, rc::Rc};
use wove::{
    elements::{List, Table},
    testing::Screen,
    Key,
};
#[test]
fn million_row_list_materializes_only_the_visible_viewport() {
    let calls = Rc::new(Cell::new(0));
    let count = calls.clone();
    let mut screen = Screen::new(12, 4);
    let id = screen
        .tree
        .add(
            screen.tree.root(),
            List::new(1_000_000, 12, move |i| {
                count.set(count.get() + 1);
                format!("Row {i}")
            }),
        )
        .unwrap();
    screen.tree.focus(Some(id)).unwrap();
    screen.frame().unwrap();
    assert_eq!(calls.get(), 4);
    screen.send(Key::End).unwrap();
    let frame = screen.frame().unwrap();
    assert_eq!(calls.get(), 8);
    assert!(frame.lines()[3].contains("999999"));
}
#[test]
fn table_preserves_header_and_clips_wide_cells_before_next_column() {
    let mut screen = Screen::new(8, 3);
    let id = screen
        .tree
        .add(
            screen.tree.root(),
            Table::new(
                vec![("A".into(), 3), ("B".into(), 3)],
                vec![
                    vec!["界界".into(), "one".into()],
                    vec!["x".into(), "two".into()],
                    vec!["y".into(), "end".into()],
                ],
            ),
        )
        .unwrap();
    screen.tree.focus(Some(id)).unwrap();
    assert_eq!(screen.frame().unwrap().cell(4, 1).unwrap().symbol(), "o");
    screen.send(Key::End).unwrap();
    let frame = screen.frame().unwrap();
    assert_eq!(frame.cell(0, 0).unwrap().symbol(), "A");
    assert!(frame.lines()[2].contains("end"));
}
