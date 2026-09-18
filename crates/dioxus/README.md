# wove-dioxus

Dioxus components for wove terminal user interfaces. This optional adapter maps
RSX elements and component updates onto `wove`'s persistent widget tree.

See the [guide](https://github.com/intuitums/wove/blob/main/docs/dioxus.md)
and run `cargo run -p wove-dioxus --example counter` from the workspace.

The API is experimental. Applications own their event loop and executor.
