# Keymap

Key bindings and multi-key sequences for [Wove](https://wovetui.com) apps.

```rust
let mut keys = Keymap::new(Duration::from_millis(300));
keys.bind(None, [Key::Char('g').into(), Key::Char('g').into()], "top");

match keys.feed(Key::Char('g').into(), &scopes, now) {
    Match::Command(command) => run(command),
    Match::Pending => {} // Wait for the next key.
    _ => {}
}
```

Bindings can be global or scoped to a node. You run the commands and supply
the time.

See the [Keymap guide](https://wovetui.com/docs/keymap/) for scopes
and timeouts.
