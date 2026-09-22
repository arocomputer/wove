use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
    rc::Rc,
};
use wove::{
    elements::{Feed, Lazy, List, Panel, Table, Text},
    testing::Screen,
    text::{Span, Wrap},
    Canvas, Element, Id, Key, Style,
};

fn block(text: &str) -> Vec<Span> {
    vec![Span::new(text, Style::default())]
}

#[test]
fn a_feed_follows_its_tail_until_scrolled_and_holds_its_place_while_blocks_arrive() {
    let mut screen = Screen::new(6, 3);
    let mut feed = Feed::new(0);
    for i in 0..100_000 {
        feed.push(block(&format!("b{i}")), Wrap::Word);
    }
    let id = screen.tree.add(screen.tree.root(), feed).unwrap();
    screen.tree.focus(Some(id)).unwrap();
    assert_eq!(
        screen.frame().unwrap().lines(),
        ["b99997", "b99998", "b99999"]
    );
    screen.send(Key::Up).unwrap();
    screen
        .tree
        .update::<Feed>(id, |feed| {
            feed.push(block("new"), Wrap::Word);
        })
        .unwrap();
    assert_eq!(
        screen.frame().unwrap().lines(),
        ["b99996", "b99997", "b99998"]
    );
    assert!(!screen.tree.get::<Feed>(id).unwrap().following());
    screen.send(Key::PageDown).unwrap();
    assert!(screen.tree.get::<Feed>(id).unwrap().following());
    assert_eq!(
        screen.frame().unwrap().lines(),
        ["b99998", "b99999", "new   "]
    );
}

#[test]
fn a_feed_scrolls_by_rows_through_wrapped_blocks_and_their_gaps() {
    let mut screen = Screen::new(6, 3);
    let mut feed = Feed::new(1);
    feed.push(block("top"), Wrap::Word);
    let last = feed.push(block("aaaaaa bbbbbb"), Wrap::Word);
    let id = screen.tree.add(screen.tree.root(), feed).unwrap();
    screen.tree.focus(Some(id)).unwrap();
    assert_eq!(
        screen.frame().unwrap().lines(),
        ["      ", "aaaaaa", "bbbbbb"]
    );
    screen.send(Key::Up).unwrap();
    assert_eq!(
        screen.frame().unwrap().lines(),
        ["top   ", "      ", "aaaaaa"]
    );
    // Growing the last block while scrolled away does not move the view.
    screen
        .tree
        .update::<Feed>(id, |feed| feed.set(last, block("aaaaaa bbbbbb cccccc")))
        .unwrap();
    assert_eq!(
        screen.frame().unwrap().lines(),
        ["top   ", "      ", "aaaaaa"]
    );
    screen.send(Key::End).unwrap();
    assert_eq!(
        screen.frame().unwrap().lines(),
        ["aaaaaa", "bbbbbb", "cccccc"]
    );
}

#[test]
fn a_feed_anchored_in_a_tall_block_keeps_showing_text_when_it_gets_wider() {
    let mut screen = Screen::new(4, 2);
    let mut feed = Feed::new(0);
    feed.push(block("aaaa bbbb cccc dddd eeee"), Wrap::Word);
    feed.push(block("tail"), Wrap::Word);
    feed.push(block("end"), Wrap::Word);
    let id = screen.tree.add(screen.tree.root(), feed).unwrap();
    screen.tree.focus(Some(id)).unwrap();
    screen.frame().unwrap();
    screen.send(Key::Up).unwrap();
    screen.send(Key::Up).unwrap();
    assert_eq!(screen.frame().unwrap().lines(), ["dddd", "eeee"]);
    // At this width the first block is one row; the anchor must not point past it.
    screen.resize(30, 2);
    let lines = screen.frame().unwrap().lines();
    assert!(
        lines[0].starts_with("aaaa bbbb cccc dddd eeee"),
        "{lines:?}"
    );
    assert!(lines[1].starts_with("tail"), "{lines:?}");
}

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
                vec![Span::new(format!("Row {i}"), Style::default())]
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

#[test]
fn list_and_table_measure_tabs_as_the_cells_they_paint() {
    let mut screen = Screen::new(9, 3);
    let list = List::new(1, 9, |_| {
        vec![
            Span::new("a\t", Style::default()),
            Span::new("b", Style::default()),
        ]
    });
    screen.tree.add(screen.tree.root(), list).unwrap();
    let table = Table::new(
        vec![("A".into(), 4), ("B".into(), 3)],
        vec![vec!["ab\tcd".into(), "xyz".into()]],
    );
    screen.tree.add(screen.tree.root(), table).unwrap();
    let lines = screen.frame().unwrap().lines();
    assert_eq!(lines[0], "a   b    ");
    assert_eq!(lines[2], "ab   xyz ");
}

/// A row of a lazy column that records when it is measured.
struct Row {
    index: usize,
    measured: Rc<RefCell<BTreeSet<usize>>>,
}
impl Element for Row {
    fn measure(&self, width: Option<u16>) -> (u16, u16) {
        self.measured.borrow_mut().insert(self.index);
        (width.unwrap_or(0), 1)
    }
    fn paint(&self, canvas: &mut Canvas<'_>) {
        canvas.text(0, 0, &format!("r{}", self.index), Style::default());
    }
    fn focusable(&self) -> bool {
        true
    }
}

/// A lazy column of `count` rows, with the ids of the column and its rows.
fn rows(
    screen: &mut Screen,
    lazy: Lazy,
    count: usize,
) -> (Id, Vec<Id>, Rc<RefCell<BTreeSet<usize>>>) {
    let measured = Rc::new(RefCell::new(BTreeSet::new()));
    let id = screen.tree.add(screen.tree.root(), lazy).unwrap();
    let rows = (0..count)
        .map(|index| {
            let measured = measured.clone();
            screen.tree.add(id, Row { index, measured }).unwrap()
        })
        .collect();
    (id, rows, measured)
}

#[test]
fn a_lazy_column_lays_out_only_the_children_in_view() {
    let mut screen = Screen::new(6, 3);
    let (id, _, measured) = rows(&mut screen, Lazy::default(), 10_000);
    screen.tree.focus(Some(id)).unwrap();
    assert_eq!(
        screen.frame().unwrap().lines(),
        ["r0    ", "r1    ", "r2    "]
    );
    assert!(
        measured.borrow().len() < 10,
        "{:?}",
        measured.borrow().len()
    );
    screen.send(Key::PageDown).unwrap();
    assert_eq!(
        screen.frame().unwrap().lines(),
        ["r3    ", "r4    ", "r5    "]
    );
    screen.send(Key::End).unwrap();
    screen.send(Key::Up).unwrap();
    assert_eq!(
        screen.frame().unwrap().lines(),
        ["r9996 ", "r9997 ", "r9998 "]
    );
    // The first rows, the rows around the tail, and nothing in between.
    let measured = measured.borrow();
    assert!(measured.len() < 20, "{measured:?}");
    assert!(!measured.contains(&5000));
}

#[test]
fn a_lazy_column_follows_its_tail_until_scrolled_and_holds_its_place() {
    let mut screen = Screen::new(6, 3);
    let mut lazy = Lazy::default();
    lazy.follow = true;
    let (id, _, measured) = rows(&mut screen, lazy, 100);
    screen.tree.focus(Some(id)).unwrap();
    assert_eq!(
        screen.frame().unwrap().lines(),
        ["r97   ", "r98   ", "r99   "]
    );
    let add = |screen: &mut Screen, index| {
        let measured = measured.clone();
        screen.tree.add(id, Row { index, measured }).unwrap();
    };
    add(&mut screen, 100);
    assert_eq!(
        screen.frame().unwrap().lines(),
        ["r98   ", "r99   ", "r100  "]
    );
    screen.send(Key::Up).unwrap();
    add(&mut screen, 101);
    assert_eq!(
        screen.frame().unwrap().lines(),
        ["r97   ", "r98   ", "r99   "]
    );
    assert!(!screen.tree.get::<Lazy>(id).unwrap().follow);
    screen.send(Key::End).unwrap();
    assert_eq!(
        screen.frame().unwrap().lines(),
        ["r99   ", "r100  ", "r101  "]
    );
    assert!(screen.tree.get::<Lazy>(id).unwrap().follow);
}

#[test]
fn a_lazy_column_reveals_focus_and_takes_clicks_only_where_it_painted() {
    let mut screen = Screen::new(6, 3);
    let (_, ids, _) = rows(&mut screen, Lazy::default(), 100);
    screen.tree.focus(Some(ids[50])).unwrap();
    assert_eq!(
        screen.frame().unwrap().lines(),
        ["r48   ", "r49   ", "r50   "]
    );
    assert_eq!(screen.tree.bounds(ids[50]).unwrap().y, 2);
    assert_eq!(screen.tree.bounds(ids[0]).unwrap().height, 0);
    screen.send(Key::Tab).unwrap();
    assert_eq!(screen.tree.focused(), Some(ids[51]));
    assert_eq!(
        screen.frame().unwrap().lines(),
        ["r49   ", "r50   ", "r51   "]
    );
    assert_eq!(screen.click(0, 0).unwrap().target, Some(ids[49]));
}

#[test]
fn a_lazy_column_measures_a_child_again_when_its_subtree_changes() {
    let mut screen = Screen::new(8, 4);
    let mut lazy = Lazy::default();
    lazy.follow = true;
    let id = screen.tree.add(screen.tree.root(), lazy).unwrap();
    let mut texts = vec![];
    for label in ["one", "two"] {
        let panel = screen.tree.add(id, Panel::default()).unwrap();
        texts.push(screen.tree.add(panel, Text::new(label)).unwrap());
    }
    assert_eq!(
        screen.frame().unwrap().lines(),
        ["└──────┘", "┌──────┐", "│two   │", "└──────┘"],
    );
    screen
        .tree
        .update::<Text>(texts[1], |text| text.content = "two\nlines".into())
        .unwrap();
    assert_eq!(
        screen.frame().unwrap().lines(),
        ["┌──────┐", "│two   │", "│lines │", "└──────┘"],
    );
}

#[test]
fn a_child_moves_between_a_lazy_column_and_an_ordinary_parent() {
    let mut screen = Screen::new(6, 2);
    let (id, ids, _) = rows(&mut screen, Lazy::default(), 3);
    screen.frame().unwrap();
    let root = screen.tree.root();
    screen.tree.insert(root, ids[0], 0).unwrap();
    assert_eq!(screen.frame().unwrap().lines(), ["r0    ", "r1    "]);
    screen.tree.append(id, ids[0]).unwrap();
    screen.tree.focus(Some(id)).unwrap();
    screen.send(Key::End).unwrap();
    assert_eq!(screen.frame().unwrap().lines(), ["r2    ", "r0    "]);
}
