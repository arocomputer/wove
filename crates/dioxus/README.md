# weft-dioxus

Dioxus components for weft terminal user interfaces. This optional adapter maps
RSX elements and component updates onto `weft-core`'s persistent widget tree.

See the [guide](https://github.com/intuitums/weft/blob/feat/core/docs/dioxus.md)
and run `cargo run -p weft-dioxus --example counter` from the workspace.

The API is experimental. Applications own their event loop and executor.
