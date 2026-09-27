//! Public contracts used by application-defined layouts and interactive views.
use std::{cell::Cell, rc::Rc};

#[test]
fn a_deferred_reveal_is_discarded_when_its_descendant_is_removed() {
    let mut tree = Tree::new();
    let lazy = tree
        .add(tree.root(), wove::elements::Lazy::default())
        .unwrap();
    tree.set_layout(lazy, fixed(10, 0)).unwrap();
    let row = tree.add(lazy, Container).unwrap();
    let input = tree.add(row, Input::new("gone")).unwrap();
    tree.focus(Some(input)).unwrap();
    tree.frame(10, 3).unwrap();
    tree.remove(input).unwrap();
    tree.set_layout(lazy, fixed(10, 2)).unwrap();
    tree.frame(10, 3).unwrap();
    assert_eq!(tree.focused(), None);
}

#[test]
fn lazy_overlays_keep_root_relative_layout_across_frames() {
    let mut tree = Tree::new();
    let lazy = tree
        .add(tree.root(), wove::elements::Lazy::default())
        .unwrap();
    tree.set_layout(lazy, fixed(4, 2)).unwrap();
    let overlay = tree.add(lazy, Text::new("popup")).unwrap();
    tree.set_overlay(overlay, true).unwrap();
    let mut layout = fixed(5, 1);
    layout.inset.left = layout::percent(0.5);
    tree.set_layout(overlay, layout).unwrap();
    for width in [20, 24] {
        assert_eq!(
            tree.frame(width, 4)
                .unwrap()
                .cell(width / 2, 0)
                .unwrap()
                .symbol(),
            "p"
        );
        assert_eq!(tree.bounds(overlay).unwrap().x, width / 2);
    }
}

#[test]
fn clearing_a_logical_selection_reports_a_repaint() {
    let mut tree = Tree::new();
    let document = tree.add(tree.root(), Document::new("hello")).unwrap();
    tree.set_layout(document, fixed(10, 1)).unwrap();
    tree.frame(10, 1).unwrap();
    tree.dispatch(mouse(MouseKind::Down(Button::Left), 0, 0))
        .unwrap();
    tree.dispatch(mouse(MouseKind::Drag(Button::Left), 4, 0))
        .unwrap();
    tree.dispatch(mouse(MouseKind::Up(Button::Left), 4, 0))
        .unwrap();
    tree.frame(10, 1).unwrap();
    assert!(
        tree.dispatch(mouse(MouseKind::Down(Button::Left), 2, 0))
            .unwrap()
            .changed
    );
}
use wove::{
    elements::{Container, Document, Input, Text},
    layout::{length, Size},
    text::Wrap,
    *,
};

fn fixed(width: u16, height: u16) -> Layout {
    Layout {
        size: Size {
            width: length(f32::from(width)),
            height: length(f32::from(height)),
        },
        flex_shrink: 0.0,
        ..Layout::default()
    }
}
fn mouse(kind: MouseKind, x: u16, y: u16) -> Event {
    Event::Mouse(Mouse::new(x, y, kind))
}

/// A horizontal window, deliberately unlike the built-in vertical Lazy.
#[derive(Default)]
struct Strip {
    first: usize,
}
impl Element for Strip {
    fn manages_children(&self) -> bool {
        true
    }
    fn arrange(&mut self, children: &mut dyn Children) -> Result<(), Error> {
        if let Some(index) = children.focused() {
            self.first = index;
        }
        for slot in 0..usize::from(children.size().0 / 4) {
            let index = self.first + slot;
            if index >= children.ids().len() {
                break;
            }
            children.place(index, ((slot * 4) as i32, 0), 4)?;
        }
        Ok(())
    }
}

#[test]
fn external_containers_place_only_visible_children_and_reveal_focus() {
    let mut tree = Tree::new();
    let strip = tree.add(tree.root(), Strip::default()).unwrap();
    tree.set_layout(strip, fixed(8, 1)).unwrap();
    let ids: Vec<_> = (0..1000)
        .map(|i| tree.add(strip, Input::new(&format!("{i:03}"))).unwrap())
        .collect();
    tree.frame(8, 1).unwrap();
    assert_eq!(
        tree.target(&mouse(MouseKind::Down(Button::Left), 5, 0)),
        Some(ids[1])
    );
    assert_eq!(tree.bounds(ids[999]).unwrap(), Rect::default());
    tree.focus(Some(ids[999])).unwrap();
    tree.frame(8, 1).unwrap();
    assert_eq!(
        tree.target(&mouse(MouseKind::Down(Button::Left), 1, 0)),
        Some(ids[999])
    );
    assert_eq!(tree.bounds(ids[0]).unwrap(), Rect::default());
    tree.dispatch(Key::Char('!').into()).unwrap();
    assert_eq!(tree.get::<Input>(ids[999]).unwrap().editor.text(), "999!");
}

#[test]
fn paint_only_application_updates_reuse_measurement() {
    struct Meter(Rc<Cell<usize>>, bool);
    impl Element for Meter {
        fn measure(&self, _: Option<u16>) -> (u16, u16) {
            self.0.set(self.0.get() + 1);
            (1, 1)
        }
        fn paint(&self, canvas: &mut Canvas<'_>) {
            canvas.text(0, 0, if self.1 { "B" } else { "A" }, Style::default());
        }
    }
    let count = Rc::new(Cell::new(0));
    let mut tree = Tree::new();
    let id = tree.add(tree.root(), Meter(count.clone(), false)).unwrap();
    tree.frame(8, 2).unwrap();
    let before = count.get();
    tree.repaint::<Meter>(id, |meter| meter.1 = true).unwrap();
    assert_eq!(tree.frame(8, 2).unwrap().cell(0, 0).unwrap().symbol(), "B");
    assert_eq!(count.get(), before);
}

#[test]
fn nested_scopes_bound_tab_bubbling_and_pointer_then_restore_focus() {
    let mut tree = Tree::new();
    let outside = tree.add(tree.root(), Input::new("outside")).unwrap();
    let dialog = tree.add(tree.root(), Container).unwrap();
    let first = tree.add(dialog, Input::new("first")).unwrap();
    let nested = tree.add(dialog, Container).unwrap();
    let last = tree.add(nested, Input::new("last")).unwrap();
    tree.focus(Some(outside)).unwrap();
    tree.push_scope(dialog).unwrap();
    assert_eq!(tree.focused(), Some(first));
    tree.dispatch(Key::Tab.into()).unwrap();
    assert_eq!(tree.focused(), Some(last));
    tree.dispatch(Key::Tab.into()).unwrap();
    assert_eq!(tree.focused(), Some(first));
    tree.focus(Some(outside)).unwrap();
    assert_eq!(tree.focused(), Some(first));
    assert_eq!(
        tree.dispatch(Key::Enter.into()).unwrap().path,
        [first, dialog]
    );
    tree.frame(12, 5).unwrap();
    assert_eq!(
        tree.target(&mouse(MouseKind::Down(Button::Left), 0, 0)),
        Some(dialog)
    );
    tree.push_scope(nested).unwrap();
    assert_eq!(tree.focused(), Some(last));
    tree.remove(nested).unwrap();
    assert_eq!(tree.focused(), Some(first));
    tree.remove(dialog).unwrap();
    assert_eq!(tree.focused(), Some(outside));
}

#[test]
fn overlays_escape_clipping_but_keep_ownership_and_can_pass_pointer_input() {
    let mut tree = Tree::new();
    let parent = tree.add(tree.root(), Container).unwrap();
    tree.set_layout(parent, fixed(2, 1)).unwrap();
    let overlay = tree.add(parent, Text::new("popover")).unwrap();
    tree.set_overlay(overlay, true).unwrap();
    let mut style = fixed(7, 1);
    style.position = layout::Position::Absolute;
    style.inset.left = length(4.0);
    tree.set_layout(overlay, style).unwrap();
    assert_eq!(tree.frame(12, 3).unwrap().cell(4, 0).unwrap().symbol(), "p");
    let event = mouse(MouseKind::Down(Button::Left), 5, 0);
    assert_eq!(tree.target(&event), Some(overlay));
    assert_eq!(
        tree.dispatch(Key::Enter.into()).unwrap().path,
        [tree.root()]
    );
    tree.set_pointer_events(overlay, false).unwrap();
    assert_eq!(tree.target(&event), Some(tree.root()));
    tree.remove(parent).unwrap();
    assert!(!tree.contains(overlay));
    assert_eq!(tree.frame(12, 3).unwrap().cell(4, 0).unwrap().symbol(), " ");
}

#[test]
fn logical_selection_survives_scroll_and_reflow_without_copying_wrapped_rows() {
    let mut tree = Tree::new();
    let mut document = Document::new("alpha界 beta\ngamma\ndelta");
    document.wrap = Wrap::Character;
    let id = tree.add(tree.root(), document).unwrap();
    tree.set_layout(id, fixed(8, 2)).unwrap();
    tree.frame(8, 2).unwrap();
    tree.dispatch(mouse(MouseKind::Down(Button::Left), 0, 0))
        .unwrap();
    tree.dispatch(mouse(MouseKind::Drag(Button::Left), 4, 1))
        .unwrap();
    tree.dispatch(mouse(MouseKind::Up(Button::Left), 4, 1))
        .unwrap();
    assert_eq!(tree.selected_text().as_deref(), Some("alpha界 beta"));
    tree.dispatch(Key::End.into()).unwrap();
    tree.set_layout(id, fixed(16, 2)).unwrap();
    tree.frame(16, 2).unwrap();
    assert_eq!(tree.selected_text().as_deref(), Some("alpha界 beta"));
    tree.repaint::<Document>(id, |document| document.select(Some(6..9)))
        .unwrap();
    assert_eq!(tree.selected_text().as_deref(), Some("界 "));
    tree.remove(id).unwrap();
    assert_eq!(tree.selected_text(), None);
}

#[test]
fn hidden_outer_scope_closes_nested_scopes_and_restores_prior_focus() {
    let mut tree = Tree::new();
    let input = tree.add(tree.root(), Input::default()).unwrap();
    let outer = tree.add(tree.root(), Container).unwrap();
    let inner = tree.add(outer, Input::default()).unwrap();
    tree.focus(Some(input)).unwrap();
    tree.push_scope(outer).unwrap();
    tree.push_scope(inner).unwrap();
    tree.set_layout(
        outer,
        Layout {
            display: layout::Display::None,
            ..Layout::default()
        },
    )
    .unwrap();
    tree.frame(8, 4).unwrap();
    assert_eq!(tree.scope(), None);
    assert_eq!(tree.focused(), Some(input));
}

#[test]
fn failed_custom_arrangement_preserves_owned_children_and_can_recover() {
    struct Fallible(bool);
    impl Element for Fallible {
        fn manages_children(&self) -> bool {
            true
        }
        fn arrange(&mut self, children: &mut dyn Children) -> Result<(), Error> {
            children.place(if self.0 { 9 } else { 0 }, (0, 0), 8)
        }
    }
    let mut tree = Tree::new();
    let parent = tree.add(tree.root(), Fallible(true)).unwrap();
    tree.set_layout(parent, fixed(8, 1)).unwrap();
    let child = tree.add(parent, Text::new("retained")).unwrap();
    assert!(matches!(tree.frame(8, 1), Err(Error::MissingNode)));
    assert_eq!(tree.children(parent).unwrap(), [child]);
    tree.repaint::<Fallible>(parent, |element| element.0 = false)
        .unwrap();
    assert_eq!(tree.frame(8, 1).unwrap().cell(0, 0).unwrap().symbol(), "r");
}

#[test]
fn removing_a_managed_child_clears_hit_testing_before_the_next_frame() {
    let mut tree = Tree::new();
    let strip = tree.add(tree.root(), Strip::default()).unwrap();
    tree.set_layout(strip, fixed(8, 1)).unwrap();
    let input = tree.add(strip, Input::default()).unwrap();
    tree.frame(8, 1).unwrap();
    tree.remove(input).unwrap();
    assert_eq!(
        tree.target(&mouse(MouseKind::Down(Button::Left), 0, 0)),
        Some(strip)
    );
}

#[test]
fn custom_logical_selection_excludes_display_only_gutters() {
    struct Numbered(Document);
    impl Element for Numbered {
        fn paint(&self, canvas: &mut Canvas<'_>) {
            canvas.text(0, 0, "12 | hello", Style::default());
        }
        fn text_position(&self, at: (u16, u16), size: (u16, u16)) -> Option<usize> {
            self.0.text_position((at.0.saturating_sub(5), at.1), size)
        }
        fn select(&mut self, range: Option<std::ops::Range<usize>>) {
            self.0.select(range);
        }
        fn selected_text(&self) -> Option<String> {
            self.0.selected_text()
        }
    }
    let mut tree = Tree::new();
    let id = tree
        .add(tree.root(), Numbered(Document::new("hello")))
        .unwrap();
    tree.set_layout(id, fixed(12, 1)).unwrap();
    tree.frame(12, 1).unwrap();
    tree.dispatch(mouse(MouseKind::Down(Button::Left), 0, 0))
        .unwrap();
    tree.dispatch(mouse(MouseKind::Drag(Button::Left), 10, 0))
        .unwrap();
    assert_eq!(tree.selected_text().as_deref(), Some("hello"));
}

#[test]
fn a_logical_drag_can_continue_after_scrolling_to_another_part_of_the_document() {
    let mut tree = Tree::new();
    let id = tree
        .add(tree.root(), Document::new("first\nsecond\nthird\nfourth"))
        .unwrap();
    tree.set_layout(id, fixed(10, 2)).unwrap();
    tree.frame(10, 2).unwrap();
    tree.dispatch(mouse(MouseKind::Down(Button::Left), 0, 0))
        .unwrap();
    tree.dispatch(mouse(MouseKind::ScrollDown, 1, 1)).unwrap();
    tree.frame(10, 2).unwrap();
    tree.dispatch(mouse(MouseKind::Drag(Button::Left), 5, 1))
        .unwrap();
    assert_eq!(
        tree.selected_text().as_deref(),
        Some("first\nsecond\nthird")
    );
}

#[test]
fn custom_containers_measure_focus_at_their_chosen_child_width() {
    struct Narrow(Rc<Cell<u32>>);
    impl Element for Narrow {
        fn manages_children(&self) -> bool {
            true
        }
        fn arrange(&mut self, children: &mut dyn Children) -> Result<(), Error> {
            if let Some(focus) = children.reveal(4)? {
                self.0.set(focus.at.1);
            }
            children.place(0, (0, 0), 4)
        }
    }
    let row = Rc::new(Cell::new(u32::MAX));
    let mut tree = Tree::new();
    let parent = tree.add(tree.root(), Narrow(row.clone())).unwrap();
    tree.set_layout(parent, fixed(20, 4)).unwrap();
    let child = tree.add(parent, Container).unwrap();
    tree.set_layout(
        child,
        Layout {
            flex_direction: layout::FlexDirection::Column,
            ..Layout::default()
        },
    )
    .unwrap();
    tree.add(
        child,
        Text {
            wrap: true,
            ..Text::new("abcdefgh")
        },
    )
    .unwrap();
    let input = tree.add(child, Input::default()).unwrap();
    tree.focus(Some(input)).unwrap();
    tree.frame(20, 4).unwrap();
    assert_eq!(row.get(), 2);
    assert_eq!(tree.bounds(input).unwrap().y, 2);
}
