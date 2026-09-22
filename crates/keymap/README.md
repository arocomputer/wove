# Keymap

Scoped bindings and key sequences for terminal applications. Applications own
command execution, focus ancestry, and the clock. Pass the current time to `feed`
with each key, and call `expire` when the pending deadline arrives without one;
execute its returned commands in order. Call `clear` on focus changes and `remove` when a
scoped element is destroyed.

Bindings in the focused scope override ancestors and globals. Newer bindings win
within a scope. An exact match that also prefixes a longer sequence waits for the
configured timeout. When the timeout passes or a key does not continue the
sequence, the longest exact binding typed so far runs and the keys after it are
replayed as a fresh sequence. `feed` then returns `Match::Flushed` with the
commands that became final and the key's own result.
