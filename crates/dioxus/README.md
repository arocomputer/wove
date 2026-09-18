# wove-dioxus

Dioxus components for Wove terminal user interfaces. This optional adapter maps
RSX elements and component updates onto Wove's persistent widget tree.

See the [guide](https://github.com/intuitums/wove/blob/main/crates/dioxus/docs/start.md)
and run `cargo run -p wove-dioxus --example counter` from the workspace.

The API is experimental. Applications own their event loop and executor.
