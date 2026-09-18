use std::{cell::Cell, rc::Rc};
use wove::{
    elements::*,
    layout::{length, FlexDirection, Size},
    *,
};
fn fixed(w: f32, h: f32) -> Layout {
    Layout {
        size: Size {
            width: length(w),
            height: length(h),
        },
        flex_shrink: 0.0,
        ..Layout::default()
    }
}
fn lines(frame: &Buffer) -> Vec<String> {
    (0..frame.area().height)
        .map(|y| {
            (0..frame.area().width)
                .map(|x| frame.cell(x, y).unwrap().symbol())
                .collect()
        })
        .collect()
}

#[test]
fn moves_keep_input_state_and_focus_but_removal_invalidates_ids() {
    let mut t = Tree::new();
    let a = t.add(t.root(), Container).unwrap();
    let b = t.add(t.root(), Container).unwrap();
    let input = t.add(a, Input::new("hello")).unwrap();
    t.focus(Some(input)).unwrap();
    t.dispatch(Key::Char('!').into()).unwrap();
    t.append(b, input).unwrap();
    assert_eq!(t.focused(), Some(input));
    assert_eq!(t.get::<Input>(input).unwrap().editor.text(), "hello!");
    assert!(matches!(t.append(input, b), Err(Error::Cycle)));
    t.remove(b).unwrap();
    assert_eq!(t.focused(), None);
    assert!(!t.contains(input));
    let replacement = t.add(a, Input::default()).unwrap();
    assert_ne!(replacement, input);
    let other = Tree::new();
    assert!(!other.contains(a));
}
#[test]
fn removal_drops_elements_and_handlers() {
    struct Owned(Rc<()>);
    impl Element for Owned {
        fn measure(&self, _: Option<u16>) -> (u16, u16) {
            let _ = &self.0;
            (0, 0)
        }
    }
    let token = Rc::new(());
    let mut t = Tree::new();
    let id = t.add(t.root(), Owned(token.clone())).unwrap();
    let captured = token.clone();
    t.on(id, move |_| {
        let _ = &captured;
        Response::IGNORE
    })
    .unwrap();
    assert_eq!(Rc::strong_count(&token), 3);
    t.remove(id).unwrap();
    assert_eq!(Rc::strong_count(&token), 1);
}
#[test]
fn handlers_can_cancel_editing_and_tab_then_bubble_unhandled_keys() {
    let mut t = Tree::new();
    let input = t.add(t.root(), Input::default()).unwrap();
    t.focus(Some(input)).unwrap();
    t.on(input, |_| Response::HANDLED).unwrap();
    t.dispatch(Key::Char('a').into()).unwrap();
    t.dispatch(Key::Tab.into()).unwrap();
    assert_eq!(t.get::<Input>(input).unwrap().editor.text(), "");
    assert_eq!(t.focused(), Some(input));
    t.on(input, |_| Response::IGNORE).unwrap();
    let result = t.dispatch(Key::Enter.into()).unwrap();
    assert_eq!(result.path, vec![input, t.root()]);
}
#[test]
fn hidden_and_removed_elements_leave_the_focus_order() {
    let mut t = Tree::new();
    let a = t.add(t.root(), Input::default()).unwrap();
    let b = t.add(t.root(), Input::default()).unwrap();
    t.focus_next(false).unwrap();
    assert_eq!(t.focused(), Some(a));
    let mut style = t.layout(a).unwrap().clone();
    style.display = layout::Display::None;
    t.set_layout(a, style).unwrap();
    t.frame(10, 2).unwrap();
    assert_eq!(t.focused(), None);
    t.focus_next(true).unwrap();
    assert_eq!(t.focused(), Some(b));
}
#[test]
fn idle_frames_do_not_measure_or_paint() {
    struct Count(Rc<Cell<usize>>);
    impl Element for Count {
        fn paint(&self, _: &mut Canvas<'_>) {
            self.0.set(self.0.get() + 1);
        }
    }
    let count = Rc::new(Cell::new(0));
    let mut t = Tree::new();
    let id = t.add(t.root(), Count(count.clone())).unwrap();
    t.frame(8, 2).unwrap();
    t.frame(8, 2).unwrap();
    assert_eq!(count.get(), 1);
    t.update::<Count>(id, |_| {}).unwrap();
    t.frame(8, 2).unwrap();
    assert_eq!(count.get(), 2);
}
#[test]
fn wrapped_text_respects_panel_edges_and_resize() {
    let mut t = Tree::new();
    let panel = t.add(t.root(), Panel::default()).unwrap();
    let mut style = fixed(6.0, 5.0);
    style.border = layout::Rect::length(1.0);
    t.set_layout(panel, style).unwrap();
    t.add(
        panel,
        Text {
            content: "ab界cd".into(),
            wrap: true,
            ..Text::default()
        },
    )
    .unwrap();
    assert_eq!(
        lines(t.frame(6, 5).unwrap()),
        ["┌────┐", "│ab界│", "│cd  │", "│    │", "└────┘"]
    );
    assert_eq!(t.frame(0, 0).unwrap().area(), Rect::default());
}
#[test]
fn scroll_clips_content_and_follows_only_when_at_end() {
    let mut t = Tree::new();
    let scroll = t.add(t.root(), Scroll::default()).unwrap();
    let mut style = t.layout(scroll).unwrap().clone();
    style.size = Size {
        width: length(8.0),
        height: length(2.0),
    };
    t.set_layout(scroll, style).unwrap();
    let text = t.add(scroll, Text::new("one\ntwo\nthree\nfour")).unwrap();
    let mut style = t.layout(text).unwrap().clone();
    style.flex_shrink = 0.0;
    t.set_layout(text, style).unwrap();
    t.focus(Some(scroll)).unwrap();
    assert_eq!(lines(t.frame(8, 2).unwrap()), ["one     ", "two     "]);
    t.dispatch(Key::End.into()).unwrap();
    assert_eq!(lines(t.frame(8, 2).unwrap()), ["three   ", "four    "]);
    t.update::<Text>(text, |w| w.content.push_str("\nfive"))
        .unwrap();
    assert_eq!(lines(t.frame(8, 2).unwrap()), ["four    ", "five    "]);
    t.dispatch(Key::Up.into()).unwrap();
    t.update::<Text>(text, |w| w.content.push_str("\nsix"))
        .unwrap();
    assert_eq!(lines(t.frame(8, 2).unwrap()), ["three   ", "four    "]);
}
#[test]
fn mouse_focus_matches_clipped_frame() {
    let mut t = Tree::new();
    let mut style = t.layout(t.root()).unwrap().clone();
    style.flex_direction = FlexDirection::Row;
    t.set_layout(t.root(), style).unwrap();
    let a = t.add(t.root(), Input::default()).unwrap();
    t.set_layout(a, fixed(4.0, 1.0)).unwrap();
    let b = t.add(t.root(), Input::default()).unwrap();
    t.set_layout(b, fixed(4.0, 1.0)).unwrap();
    t.frame(8, 2).unwrap();
    t.dispatch(Event::Mouse(Mouse {
        x: 5,
        y: 0,
        kind: MouseKind::Down,
    }))
    .unwrap();
    assert_eq!(t.focused(), Some(b));
    t.dispatch(Key::Char('界').into()).unwrap();
    assert_eq!(t.frame(8, 2).unwrap().cursor(), Some((6, 0)));
}
