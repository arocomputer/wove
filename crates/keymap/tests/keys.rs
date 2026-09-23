use std::time::Duration;
use wove::{elements::Input, Key, Tree};
use wove_keymap::{Keymap, Match};

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
    assert!(keys.expire(Duration::from_millis(299)).is_empty());
    assert_eq!(keys.expire(Duration::from_millis(300)), ["single"]);
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
#[test]
fn unmatched_continuation_flushes_the_deferred_command_before_the_retry() {
    let mut keys = Keymap::new(Duration::from_millis(300));
    keys.bind(None, [Key::Char('g').into()], "single");
    keys.bind(
        None,
        [Key::Char('g').into(), Key::Char('g').into()],
        "double",
    );
    keys.bind(None, [Key::Char('x').into()], "x");
    keys.feed(Key::Char('g').into(), &[], Duration::ZERO);
    assert_eq!(
        keys.feed(Key::Char('x').into(), &[], Duration::ZERO),
        Match::Flushed(vec!["single"], Box::new(Match::Command("x")))
    );
    keys.feed(Key::Char('g').into(), &[], Duration::ZERO);
    assert_eq!(
        keys.feed(Key::Char('y').into(), &[], Duration::ZERO),
        Match::Flushed(vec!["single"], Box::new(Match::Unbound))
    );
}

#[test]
fn changing_focus_discards_an_expired_command_from_the_previous_scope() {
    let mut tree = Tree::new();
    let old = tree.add(tree.root(), Input::default()).unwrap();
    let new = tree.add(tree.root(), Input::default()).unwrap();
    let mut keys = Keymap::new(Duration::from_millis(300));
    keys.bind(Some(old), [Key::Char('g').into()], "old");
    keys.bind(Some(old), [Key::Char('g').into(); 2], "old double");
    keys.bind(Some(new), [Key::Char('x').into()], "new");
    assert_eq!(
        keys.feed(Key::Char('g').into(), &[old], Duration::ZERO),
        Match::Pending
    );
    assert_eq!(
        keys.feed(Key::Char('x').into(), &[new], Duration::from_millis(300)),
        Match::Command("new")
    );
}
#[test]
fn feed_resolves_an_expired_sequence_before_the_new_stroke() {
    let mut keys = Keymap::new(Duration::from_millis(300));
    keys.bind(None, [Key::Char('g').into()], "single");
    keys.bind(
        None,
        [Key::Char('g').into(), Key::Char('g').into()],
        "double",
    );
    keys.feed(Key::Char('g').into(), &[], Duration::ZERO);
    assert_eq!(
        keys.feed(Key::Char('g').into(), &[], Duration::from_millis(300)),
        Match::Flushed(vec!["single"], Box::new(Match::Pending))
    );
}
#[test]
fn a_dead_end_delivers_the_longest_exact_prefix_and_replays_the_rest() {
    let mut keys = Keymap::new(Duration::from_millis(300));
    keys.bind(None, [Key::Char('g').into()], "g");
    keys.bind(None, [Key::Char('g').into(); 3], "ggg");
    keys.bind(None, [Key::Char('x').into()], "x");
    keys.feed(Key::Char('g').into(), &[], Duration::ZERO);
    keys.feed(Key::Char('g').into(), &[], Duration::ZERO);
    assert_eq!(
        keys.feed(Key::Char('x').into(), &[], Duration::ZERO),
        Match::Flushed(vec!["g", "g"], Box::new(Match::Command("x")))
    );
}
#[test]
fn expiry_delivers_the_longest_exact_prefix_and_replays_the_rest() {
    let mut keys = Keymap::new(Duration::from_millis(300));
    keys.bind(None, [Key::Char('g').into()], "g");
    keys.bind(None, [Key::Char('g').into(); 3], "ggg");
    keys.feed(Key::Char('g').into(), &[], Duration::ZERO);
    keys.feed(Key::Char('g').into(), &[], Duration::ZERO);
    let late = Duration::from_millis(300);
    assert_eq!(keys.expire(late), ["g"]);
    assert_eq!(keys.deadline(), Some(late + Duration::from_millis(300)));
    assert_eq!(keys.expire(late + Duration::from_millis(300)), ["g"]);
}
