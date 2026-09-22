use dioxus::prelude::*;
use dioxus_core::AttributeValue;
use wove::{
    elements::{Input, Text},
    Event as InputEvent, Id, Key, Tree,
};
use wove_dioxus::{elements as dioxus_elements, View};
fn descendants(tree: &Tree, id: Id) -> Vec<Id> {
    let mut out = vec![id];
    for child in tree.children(id).unwrap() {
        out.extend(descendants(tree, *child));
    }
    out
}
fn text(view: &mut View) -> String {
    let f = view.frame(40, 12).unwrap();
    (0..12)
        .flat_map(|y| (0..40).map(move |x| (x, y)))
        .map(|(x, y)| f.cell(x, y).unwrap().symbol())
        .collect()
}
fn counter() -> Element {
    let mut n = use_signal(|| 0);
    rsx! {view {direction:"column",onkey:move |event|{if let InputEvent::Key(Key::Char('+'),_)=*event.data {n+=1;event.prevent_default();}},
        text {content:"Count {n}"}
        input {value:"start"}
    }}
}
#[test]
fn signals_render_and_prevent_default_preserves_input() {
    let mut view = View::new(VirtualDom::new(counter)).unwrap();
    assert!(text(&mut view).contains("Count 0"));
    view.send(Key::Tab.into()).unwrap();
    view.send(Key::Char('+').into()).unwrap();
    assert!(text(&mut view).contains("Count 1"));
    let id = view.tree().focused().unwrap();
    assert_eq!(view.tree().get::<Input>(id).unwrap().editor.text(), "start");
}
fn list() -> Element {
    let mut reversed = use_signal(|| false);
    let mut show = use_signal(|| true);
    let items = if reversed() {
        vec![3, 2, 1]
    } else {
        vec![1, 2, 3]
    };
    rsx! {view {direction:"column",onkey:move |event| {match *event.data {InputEvent::Key(Key::Char('r'),_)=>reversed.toggle(),InputEvent::Key(Key::Char('x'),_)=>show.toggle(),_=>{}}event.prevent_default();},
        for i in items { input {key:"{i}",value:"{i}"} }
        if show() { text {content:"visible"} }
    }}
}
#[test]
fn keyed_reordering_moves_nodes_and_conditional_removal_drops_them() {
    let mut v = View::new(VirtualDom::new(list)).unwrap();
    v.frame(20, 10).unwrap();
    v.send(Key::Tab.into()).unwrap();
    let first = v.tree().focused().unwrap();
    v.send(Key::Char('r').into()).unwrap();
    v.frame(20, 10).unwrap();
    assert_eq!(v.tree().focused(), Some(first));
    assert_eq!(v.tree().bounds(first).unwrap().y, 2);
    let before = descendants(v.tree(), v.tree().root())
        .into_iter()
        .find(|id| {
            v.tree()
                .get::<Text>(*id)
                .is_ok_and(|w| w.content == "visible")
        })
        .unwrap();
    v.send(Key::Char('x').into()).unwrap();
    assert!(!v.tree().contains(before));
    assert!(!text(&mut v).contains("visible"));
    v.send(Key::Char('x').into()).unwrap();
    assert!(text(&mut v).contains("visible"));
}
#[test]
fn invalid_attributes_fail_explicitly() {
    fn app() -> Element {
        rsx! {input {width:"nope"}}
    }
    assert!(matches!(
        View::new(VirtualDom::new(app)),
        Err(wove_dioxus::Error::Unsupported(_))
    ));
}

#[test]
fn custom_tags_use_custom_elements_without_changing_core() {
    use wove_dioxus::{Error, Registry};
    fn create(t: &mut Tree) -> Result<Id, Error> {
        Ok(t.create(Text::new("custom"))?)
    }
    fn set(t: &mut Tree, id: Id, name: &str, value: &AttributeValue) -> Result<(), Error> {
        match (name, value) {
            ("content", AttributeValue::Text(s)) => {
                t.update::<Text>(id, |w| w.content = s.clone())?;
                Ok(())
            }
            _ => Err(Error::Unsupported(name.into())),
        }
    }
    fn app() -> Element {
        rsx! {custom-label {"content":"registered"}}
    }
    let mut registry = Registry::default();
    registry.register("custom-label", create, set);
    let mut view = View::with_registry(VirtualDom::new(app), registry).unwrap();
    assert!(text(&mut view).contains("registered"));
}

#[test]
fn stop_propagation_keeps_native_editing_and_skips_parent_listener() {
    fn app() -> Element {
        let mut parent = use_signal(|| 0);
        rsx! {view {direction:"column",onkey:move |_|parent+=1,
            text {content:"Parent {parent}"}
            input {onkey:move |e|e.stop_propagation()}
        }}
    }
    let mut view = View::new(VirtualDom::new(app)).unwrap();
    view.send(Key::Tab.into()).unwrap();
    view.send(Key::Char('z').into()).unwrap();
    let id = view.tree().focused().unwrap();
    assert_eq!(view.tree().get::<Input>(id).unwrap().editor.text(), "z");
    assert!(text(&mut view).contains("Parent 0"));
}

#[test]
fn typed_styles_reach_the_native_frame() {
    fn app() -> Element {
        rsx! {text {content:"styled",style:AttributeValue::any_value(wove::Style{bold:true,..Default::default()})}}
    }
    let mut v = View::new(VirtualDom::new(app)).unwrap();
    assert!(v.frame(10, 2).unwrap().cell(0, 0).unwrap().style().bold);
}

#[test]
fn input_notifications_update_signals_without_resetting_undo() {
    fn app() -> Element {
        let mut value = use_signal(String::new);
        rsx! {view {direction:"column",
            input {value:"{value}",oninput:move |event|value.set(event.data.to_string())}
            text {content:"Value: {value}"}
        }}
    }
    let mut v = View::new(VirtualDom::new(app)).unwrap();
    v.focus_next(false).unwrap();
    v.send(InputEvent::Paste("a界".into())).unwrap();
    assert!(text(&mut v).contains("Value: a界"));
    v.send(InputEvent::Key(
        Key::Char('z'),
        wove::Modifiers {
            ctrl: true,
            ..Default::default()
        },
    ))
    .unwrap();
    assert!(!text(&mut v).contains("a界"));
}

#[test]
fn textarea_notifications_preserve_multiline_value_and_undo() {
    fn app() -> Element {
        let mut value = use_signal(String::new);
        rsx! {textarea {value:"{value}",oninput:move |event|value.set(event.data.to_string())}}
    }
    let mut view = View::new(VirtualDom::new(app)).unwrap();
    view.focus_next(false).unwrap();
    view.send(InputEvent::Paste("one\ntwo".into())).unwrap();
    let id = view.tree().focused().unwrap();
    assert_eq!(
        view.tree()
            .get::<wove::elements::Textarea>(id)
            .unwrap()
            .editor
            .text(),
        "one\ntwo"
    );
    view.send(InputEvent::Key(
        Key::Char('z'),
        wove::Modifiers {
            ctrl: true,
            ..Default::default()
        },
    ))
    .unwrap();
    assert_eq!(
        view.tree()
            .get::<wove::elements::Textarea>(id)
            .unwrap()
            .editor
            .text(),
        ""
    );
}

#[test]
fn mouse_listeners_follow_capture_and_a_prevented_release_ends_it() {
    use std::cell::RefCell;
    use wove::{Button, Modifiers, Mouse, MouseKind};
    thread_local! {
        static SEEN: RefCell<Vec<(&'static str, MouseKind)>> = const { RefCell::new(Vec::new()) };
    }
    fn app() -> Element {
        rsx! { view { direction: "column",
            input { value: "first", onmouse: move |event| {
                if let InputEvent::Mouse(mouse) = *event.data {
                    SEEN.with(|seen| seen.borrow_mut().push(("first", mouse.kind)));
                    if mouse.modifiers.shift || matches!(mouse.kind, MouseKind::Up(_)) {
                        event.prevent_default();
                    }
                }
            } }
            input { value: "second", onmouse: move |event| {
                if let InputEvent::Mouse(mouse) = *event.data {
                    SEEN.with(|seen| seen.borrow_mut().push(("second", mouse.kind)));
                }
            } }
        } }
    }
    let mut view = View::new(VirtualDom::new(app)).unwrap();
    view.frame(20, 4).unwrap();
    let down = MouseKind::Down(Button::Left);
    let drag = MouseKind::Drag(Button::Left);
    let up = MouseKind::Up(Button::Left);
    view.send(InputEvent::Mouse(Mouse::new(1, 0, down)))
        .unwrap();
    let first = view.tree().focused().unwrap();
    let result = view
        .send(InputEvent::Mouse(Mouse::new(3, 1, drag)))
        .unwrap();
    assert_eq!(result.target, Some(first));
    SEEN.with(|seen| assert_eq!(*seen.borrow(), [("first", down), ("first", drag)]));
    assert_eq!(
        view.tree().get::<Input>(first).unwrap().editor.selection(),
        1..3
    );
    let cancelled = Mouse {
        modifiers: Modifiers {
            shift: true,
            ..Modifiers::default()
        },
        ..Mouse::new(4, 1, drag)
    };
    view.send(InputEvent::Mouse(cancelled)).unwrap();
    assert_eq!(
        view.tree().get::<Input>(first).unwrap().editor.selection(),
        1..3
    );
    view.send(InputEvent::Mouse(Mouse::new(3, 1, up))).unwrap();
    let second = view
        .tree()
        .target(&InputEvent::Mouse(Mouse::new(3, 1, drag)))
        .unwrap();
    assert_ne!(second, first, "even a prevented release must end capture");
    view.send(InputEvent::Mouse(Mouse::new(1, 1, down)))
        .unwrap();
    assert_eq!(view.tree().focused(), Some(second));
    SEEN.with(|seen| {
        assert_eq!(
            *seen.borrow(),
            [
                ("first", down),
                ("first", drag),
                ("first", drag),
                ("first", up),
                ("second", down),
            ]
        )
    });
}

#[test]
fn an_equivalent_value_with_controls_keeps_the_cursor() {
    fn app() -> Element {
        let mut value = use_signal(|| "a\u{7}b".to_string());
        rsx! {input {value:"{value}",onkey:move |event|{if let InputEvent::Key(Key::Char('!'),_)=*event.data {value.set("a\u{7}\u{7}b".into());event.prevent_default();}}}}
    }
    let mut view = View::new(VirtualDom::new(app)).unwrap();
    view.focus_next(false).unwrap();
    view.send(Key::Home.into()).unwrap();
    view.send(Key::Char('!').into()).unwrap();
    let id = view.tree().focused().unwrap();
    let editor = &view.tree().get::<Input>(id).unwrap().editor;
    assert_eq!((editor.text(), editor.cursor()), ("ab", 0));
}

#[test]
fn registered_tags_take_layout_attributes_and_restore_their_own_default() {
    use wove::layout::{length, Dimension};
    use wove_dioxus::{Error, Registry};
    fn create(t: &mut Tree) -> Result<Id, Error> {
        let id = t.create(Text::new("custom"))?;
        let mut layout = t.layout(id)?.clone();
        layout.size.width = length(7.0);
        t.set_layout(id, layout)?;
        Ok(id)
    }
    fn set(_: &mut Tree, _: Id, name: &str, _: &AttributeValue) -> Result<(), Error> {
        Err(Error::Unsupported(name.into()))
    }
    fn app() -> Element {
        let mut narrow = use_signal(|| true);
        rsx! {view {onkey:move |_|narrow.toggle(),
            input {}
            custom-label {width: if narrow() { Some(3) } else { None }}
        }}
    }
    let mut registry = Registry::default();
    registry.register("custom-label", create, set);
    let mut view = View::with_registry(VirtualDom::new(app), registry).unwrap();
    let width = |view: &View| -> Dimension {
        let id = view.tree().children(view.tree().root()).unwrap()[0];
        let id = view.tree().children(id).unwrap()[1];
        view.tree().layout(id).unwrap().size.width
    };
    assert_eq!(width(&view), length(3.0));
    view.focus_next(false).unwrap();
    view.send(Key::Char('w').into()).unwrap();
    assert_eq!(width(&view), length(7.0));
}
