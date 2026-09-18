# Wove keymap

Scoped bindings and key sequences for terminal applications. Applications own
command execution, focus ancestry, and the clock. Call `expire` before dispatching
a new key and when the pending deadline arrives; execute its returned command.
Call `clear` on focus changes and `remove` when a scoped element is destroyed.

Bindings in the focused scope override ancestors and globals. Newer bindings win
within a scope. An exact match that also prefixes a longer sequence waits for the
configured timeout. An unmatched continuation is retried as a fresh sequence.
