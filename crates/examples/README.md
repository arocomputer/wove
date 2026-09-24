# Examples

Example apps. This package is not published.

```sh
cargo run -p wove-examples --bin editor
cargo run -p wove-examples --release --bin chess -- robot
```

`editor` is a multiline editor with styled text and a status line.

`chess` has the complete rules, a board drawn as pixels, a robot
to play against, and shared games over SSH:

```sh
cargo run -p wove-examples --release --bin chess -- robot   # you are White; `robot black` to sit as Black
cargo run -p wove-examples --bin chess                       # two players, one terminal
ssh-keygen -t ed25519 -N '' -f chess-host-key                # once, for the server
cargo run -p wove-examples --bin chess -- serve chess-host-key
ssh -p 2222 alice@127.0.0.1                                   # any key is accepted
```

The board grows with the terminal. Once its squares are four rows tall it
is drawn as pixels: pieces defined as geometry are rasterized at the
square's size with an outline and lighting, glide to their squares,
captures flash, a king in check glows, and legal moves show as dots and
rings. A cell holds two pixels with half blocks, which every terminal draws,
or six with sextants, which Ghostty, WezTerm, kitty, foot, and Contour draw
natively and where the pieces get their detail; `p` switches. An
evaluation bar beside the board follows the engine's view of a game against
the robot, and `t` cycles the board's colors.

The robot thinks for a second per move on a worker thread and wakes the
terminal loop when it has played; a release build searches several plies
deeper than a debug one. Each SSH player gets a seat and their own screen;
a move by one player wakes the other's session through the peer's `Waker`,
so both boards change at once.

Smaller examples live next to their crates:

```sh
cargo run -p wove --example gallery
cargo run -p wove --example inline
cargo run -p wove-dioxus --example counter
cargo run -p wove-gpu --example shader
```
