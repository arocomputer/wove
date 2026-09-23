# wove-dioxus

Write [Wove](https://wovetui.com) terminal apps with Dioxus components and
signals.

```rust
fn app() -> Element {
    let mut name = use_signal(String::new);
    rsx! {
        view { direction: "column",
            input {
                value: "{name}",
                oninput: move |event| name.set(event.data.to_string()),
            }
            text { content: "Hello, {name}!" }
        }
    }
}
```

Try the example:

```sh
cargo run -p wove-dioxus --example counter
```

See the [Dioxus guide](https://wovetui.com/docs/dioxus/) for setup,
tags, and events.
