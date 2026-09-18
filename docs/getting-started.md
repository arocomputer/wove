# Getting started

Run `cargo run --example counter` for the smallest interactive application.
Run `cargo run --example explorer` for nested layouts and selection. Both
examples live in `crates/weft/examples/` and are compiled by the test suite.

## Application state

Implement `Application::view` to draw state into a blank `Buffer`. Implement
`Application::update` to handle an input or resize event. Return false to exit.
`weft::run` paints once, then sleeps until input arrives. It exits on Ctrl-C
and ignores key-release events.

For background work, timers, or async runtimes, own a `Terminal` directly.
Obtain its size, build a frame, and call `draw`. Integrate crossterm input with
your event source; do not run competing readers on stdin. Terminal ownership
is exclusive within a process. Do not print to stdout during a session.

## Layout and composition

`Layout` divides a `Rect` horizontally or vertically. `Fixed` tracks reserve
cells in declaration order. `Fill` tracks share what remains by weight. Gaps
consume available space first. Nest layouts for more complex arrangements.

A `Widget` renders into a rectangle. Implement this trait for your own views.
`Border` gives its child the interior rectangle. `Text` clips multiline text.
`List` takes selection and scroll offset from the application; it does not
choose keybindings or mutate state.

## Headless use

With `default-features = false`, weft has no terminal backend. Render widgets
into a `Buffer`, inspect its `lines()` or individual cells, and assert the
output. Colors and attributes can be inspected without parsing escape codes.

For local API documentation, run `./x docs` and open `target/doc/weft/index.html`.
