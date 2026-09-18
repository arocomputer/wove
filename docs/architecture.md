# Architecture

weft has two crates. `core` owns terminal behavior; `dioxus` depends on its public
API. Core never depends on Dioxus. There is no facade crate and no application
policy in either crate.

## Core

`Tree` owns widgets, callbacks, parent/child relationships, and focus. A node's
`Id` is unique across trees. Removing a node drops its subtree; reparenting keeps
its state and focus. Detached and hidden nodes do not paint or receive focus.

Taffy computes flex and grid layout in cells. Widgets supply intrinsic
measurement. The paint pass translates coordinates, clips children inside parent
borders, and applies viewport offsets. All descendants are currently visited;
scrolling does not imply virtualization. Paint order follows child order, and
pointer hit testing uses that same order in reverse.

`Widget` defines measurement, paint, input, focus eligibility, and viewport
behavior. `Canvas` clips drawing and filters terminal control characters.
Applications can implement widgets without registering them globally. `Text`
shares measurement and wrapping logic. `Editor` stores grapheme-aligned byte
positions and up to 100 undo snapshots. Undo memory scales with document size.

State changes invalidate layout and paint. An idle frame does neither. A changed
frame currently allocates and repaints the cell buffer; the terminal renderer
then emits only changed cells. This is not incremental subtree painting.

The optional crossterm backend owns raw mode, alternate screen, mouse input,
bracketed paste, and cursor restoration. Portable events keep backend types out
of widget contracts. Terminal restoration runs on ordinary return and panic
unwinding, not on process abort or uncatchable signals.

## Dioxus

Dioxus owns signals, components, and reconciliation. Its mutation writer creates,
updates, moves, and removes core nodes. Keyed moves preserve native widget state.
The adapter has terminal elements, not HTML or CSS emulation. Unknown tags and
attributes produce errors. A failed mutation invalidates the view, so subsequent
calls cannot silently use a partially applied update.

Events reach the nearest registered Dioxus listener and bubble through Dioxus
parents before core widget behavior. `prevent_default` cancels native behavior;
`stop_propagation` stops Dioxus parents while allowing native editing. An
`oninput` notification carries the new value after an edit. The host
owns scheduling and can await `View::wait_for_work` alongside terminal input.
The included counter uses a blocking loop because it has no asynchronous tasks.

`Registry` lets an application map extra tags and attributes to its own widgets.
An adapter must use the same public tree operations available to every consumer.

## Reference

OpenTUI's core owns persistent renderables, layout, input, and widgets. Its React
and Solid packages translate their respective reconciliation operations into
that core. That separation informs weft's crate boundary. Its source was studied
at commit `4954312d749f71e80664aa8b0e8a75384186eb99` in
[anomalyco/opentui](https://github.com/anomalyco/opentui/tree/4954312d749f71e80664aa8b0e8a75384186eb99/packages).
weft does not translate OpenTUI's TypeScript API or reproduce its Zig renderer.
