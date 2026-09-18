#![cfg(feature = "keymap")]
use std::time::Duration;
use wove::keymap::{Keymap, Match};
use wove::{elements::Input, Key, Tree};

#[test]
fn focused_bindings_override_global_bindings_and_removal_reveals_global() {
    let mut tree = Tree::new();
    let id = tree.add(tree.root(), Input::default()).unwrap();
    let mut keys = Keymap::new(Duration::from_millis(300));
    keys.bind(None, [Key::Char('x').into()], "global");
    keys.bind(Some(id), [Key::Char('x').into()], "local");
    assert_eq!(
        keys.feed(Key::Char('x').into(), &[id], Duration::ZERO),
        Match::Command("local")
    );
    keys.remove(id);
    assert_eq!(
        keys.feed(Key::Char('x').into(), &[id], Duration::ZERO),
        Match::Command("global")
    );
}
#[test]
fn ambiguous_sequences_wait_for_timeout_and_focus_changes_clear_pending() {
    let mut tree = Tree::new();
    let id = tree.add(tree.root(), Input::default()).unwrap();
    let mut keys = Keymap::new(Duration::from_millis(300));
    keys.bind(None, [Key::Char('g').into()], "single");
    keys.bind(
        None,
        [Key::Char('g').into(), Key::Char('g').into()],
        "double",
    );
    assert_eq!(
        keys.feed(Key::Char('g').into(), &[], Duration::ZERO),
        Match::Pending
    );
    assert_eq!(keys.expire(Duration::from_millis(299)), None);
    assert_eq!(keys.expire(Duration::from_millis(300)), Some("single"));
    keys.feed(Key::Char('g').into(), &[], Duration::ZERO);
    assert_eq!(
        keys.feed(Key::Char('g').into(), &[id], Duration::ZERO),
        Match::Pending
    );
    assert_eq!(
        keys.feed(Key::Char('g').into(), &[id], Duration::ZERO),
        Match::Command("double")
    );
}
