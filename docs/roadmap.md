# Roadmap

The first milestone is a small usable library with reproducible contracts,
examples, CI, package validation, and terminal checks. This is an early
foundation, not a complete replacement for established TUI libraries.

Next work, subject to concrete application examples:

- Grapheme-aware editing, cursor placement, and text wrapping.
- Focus management and keyboard/mouse routing across composed widgets.
- Scrolling containers, tables, and a dashboard example.
- Benchmarks for sparse updates, full frames, resize, and large lists.
- Broader terminal compatibility tests and accessibility review.
- Optional main-screen and scrollback rendering with explicit ownership rules.

Keep the core independent of a runtime. Do not add a virtual DOM, reactive
scheduler, template language, or plugin ABI without evidence that consumers
need it. A stable 1.0 API requires experience from independent applications.
