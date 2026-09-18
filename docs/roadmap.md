# Roadmap

The retained core and optional Dioxus adapter are implemented. The current
examples exercise editing, layout, selection, scrolling, and component updates.
The API needs experience from independent applications before a stable release.

Next work should use concrete examples to choose scope:

- A multiline editor with vertical movement and selection painting.
- Rich text spans and word wrapping with matching measurement.
- Virtualized lists and tables, measured with large datasets.
- Sparse painting and fewer frame allocations, guided by benchmarks.
- Broader terminal compatibility and accessibility work.
- Optional main-screen and scrollback rendering with explicit ownership rules.

Core will remain usable without a component runtime. Additional adapters belong
in separate crates and must use public core operations. Application-specific
behavior belongs in applications and their own elements.
