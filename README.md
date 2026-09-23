<p align="center">
  <a href="https://wovetui.com">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="crates/web/public/logo/dark.svg">
    <img src="crates/web/public/logo/light.svg" alt="Wove logo" height="42">
  </picture>
  </a>
</p>

<div align="center">
  <a href="https://github.com/arocomputer/wove/actions/workflows/core.yml"><img alt="Core build status" src="https://img.shields.io/github/actions/workflow/status/arocomputer/wove/core.yml?branch=main&amp;style=flat-square" /></a>
</div>

Wove is a Rust library for building terminal user interfaces.

- Build with Rust elements or Dioxus components and signals.
- Arrange content with flexbox and grid layouts.
- Add text, inputs, lists, tables, and scrolling views.
- Handle keyboard and mouse input, with text selection and undo.
- Test rendering and input without opening a terminal.

> [!NOTE]
> Wove is early in development, and its API is still changing.

[Website](https://wovetui.com) | [Documentation](https://wovetui.com/docs) | [Crates](https://wovetui.com/crates)

## Crates

- [`wove`](crates/core): elements, layout, input, and rendering.
- [`wove-dioxus`](crates/dioxus): Dioxus components and signals.
- [`wove-keymap`](crates/keymap): key bindings and sequences.
- [`wove-ssh`](crates/ssh): serve apps over SSH.

## Get started

Wove is not on crates.io yet. Add it from Git:

```sh
cargo add wove --git https://github.com/arocomputer/wove
```

Then follow the [quickstart](https://wovetui.com/docs/quickstart/).

## Development

Development requires Rust 1.98 or newer and Python 3.12 or newer.

```sh
cargo build --workspace
./x check
```

Run the examples:

```sh
cargo run -p wove --example gallery
cargo run -p wove-dioxus --example counter
```

See [AGENTS.md](AGENTS.md) for repository conventions and
[CONTRIBUTING.md](CONTRIBUTING.md) for checks and releases.

## Contributing

Contributions are welcome. Bring a small example of what you want to build or a
reproduction of a bug.

## License

Wove is released under the [MIT License](LICENSE).
