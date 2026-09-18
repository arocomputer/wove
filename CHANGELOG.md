# Changelog

## Unreleased

- Rename the formatting package to `wove-format`, with Rust imports under `wove_format`.

- Add an SSH package with public-key authentication, isolated applications, remote input, and resize handling.
- Move the documentation website into `crates/web`, outside the Cargo member list.
- Separate element contracts, tree painting, Dioxus view ownership, and typed attributes.
- Use readable library names and a documentation index in the README.

- Withdraw the premature release and keep 0.0.1 unpublished.
- Remove the top-level timing suite and CI performance budgets.

- Add multiline editing, styled spans, word wrapping, and edit-based undo history.
- Add viewport-driven lists and tables, frame recording, and explicit animation clocks.
- Add separate formatting, keymap, and unpublished example packages.
- Drive Dioxus updates asynchronously alongside terminal input.
- Add main-screen sessions and explicit terminal suspension and resumption.
- Reuse frame storage when terminal dimensions stay unchanged.
- Move published docs into a minimal Astro and MDX website under `crates/web/`.

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
