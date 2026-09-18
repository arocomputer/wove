# Changelog

## Unreleased

- Reset the workspace version to 0.0.1 for ongoing development.

- Breaking: rename `Widget` to `Element` and `wove::widgets` to `wove::elements`.
  Update custom implementations and imports to use the new names.

- Update the Wove wordmark and README with a runnable getting-started example.
- Rename the source repository to `intuitums/wove`.

## 0.2.0

- Name the core package `wove` and the optional adapter `wove-dioxus`.

- Replace the initial drawing API with a retained widget tree in `crates/core`.
- Add flex and grid layout, persistent node identity, focus, bubbling input,
  clipped scrolling, and custom widgets.
- Add grapheme-aware input, selection, undo, and cursor rendering.
- Add an optional Dioxus adapter in `crates/dioxus`, with terminal RSX elements,
  keyed reconciliation, input notifications, event cancellation, and custom tags.
- Add a headless screen helper, widget gallery, adapter example, and tests for
  ownership, input, rendering, and component updates.

## 0.1.0

- Initial unpublished prototype with cell rendering and terminal ownership.
