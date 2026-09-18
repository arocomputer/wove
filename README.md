<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/assets/logo/dark.svg">
    <img src="docs/assets/logo/light.svg" alt="Wove logo" height="42">
  </picture>
</p>

<div align="center">
  <a href="https://crates.io/crates/wove"><img alt="Crates.io version" src="https://img.shields.io/crates/v/wove?style=flat-square&amp;label=crate" /></a>
  <a href="https://github.com/intuitums/wove/actions/workflows/checks.yml"><img alt="Build status" src="https://img.shields.io/github/actions/workflow/status/intuitums/wove/checks.yml?branch=main&amp;style=flat-square" /></a>
</div>

Wove is a Rust library to build terminal user interfaces.

- Build with Rust elements or Dioxus components and signals.
- Arrange content with flexbox and grid layouts.
- Add text, inputs, selects, panels, and scrolling views.
- Handle keyboard and mouse input, with text selection and undo.
- Test rendering and input without opening a terminal.

Wove is early in development, and its API is still changing.

[Documentation](crates/core/docs/start.md) | [API reference](https://docs.rs/wove) | [Roadmap](docs/roadmap.md)

## Crates

- [`wove`](crates/core) provides elements, layout, text editing, input, and terminal rendering.
- [`wove-dioxus`](crates/dioxus) adds RSX, signals, and component lifecycles over the core library. It is optional and currently available from a source checkout.

## Get started

Add the current Wove source to your Rust project. Version 0.0.1 is not yet
published; crates.io still has the earlier 0.2.0 API:

```sh
cargo add wove --git https://github.com/intuitums/wove --branch main
```

The [core guide](crates/core/docs/start.md) starts with a runnable terminal app.
For components and signals, see the [Dioxus guide](crates/dioxus/docs/start.md).

## Development

Development requires Rust 1.98 or newer and Python 3.12 or newer. The checked-in
Rust toolchain file selects the compiler.

```sh
cargo build --workspace
./x check
```

Run the examples from the repository root:

```sh
cargo run -p wove --example gallery
cargo run -p wove-dioxus --example counter
```

`./x ui` checks input, resize, and terminal restoration through real PTYs on Unix.
`./x bench` measures release builds and checks performance limits.

See [AGENTS.md](AGENTS.md) for repository conventions and
[architecture](docs/architecture.md) for the crate boundaries.

## Contributing

Contributions are welcome. Bring a small example of what you want to build or a
reproduction of a bug. [CONTRIBUTING.md](CONTRIBUTING.md) covers setup, checks,
documentation, and releases.

## License

[MIT](LICENSE).
