<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="web/public/logo/dark.svg">
    <img src="web/public/logo/light.svg" alt="Wove logo" height="42">
  </picture>
</p>

<div align="center">
  <a href="https://github.com/intuitums/wove/actions/workflows/checks.yml"><img alt="Build status" src="https://img.shields.io/github/actions/workflow/status/intuitums/wove/checks.yml?branch=main&amp;style=flat-square" /></a>
</div>

Wove is a Rust library to build terminal user interfaces.

- Build with Rust elements or Dioxus components and signals.
- Arrange content with flexbox and grid layouts.
- Add text, inputs, selects, panels, and scrolling views.
- Handle keyboard and mouse input, with text selection and undo.
- Test rendering and input without opening a terminal.

Wove is early in development, and its API is still changing.

[Documentation](web/src/content/docs/start.mdx) | [Roadmap](web/src/content/docs/roadmap.mdx)

## Crates

- [`wove`](crates/core) provides elements, layout, text editing, input, and terminal rendering.
- [`wove-content`](crates/content) provides Markdown, highlighted code, and line diffs.
- [`wove-keymap`](crates/keymap) provides scoped bindings and key sequences.
- [`wove-dioxus`](crates/dioxus) adds RSX, signals, and component lifecycles over the core library. It is optional and currently available from a source checkout.

The unpublished [`examples`](crates/examples) package contains application examples.
`web/` holds the documentation and minimal website setup.

## Get started

Add the current Wove source to your Rust project. Version 0.0.1 is not yet
published. The earlier crates.io version was withdrawn:

```sh
cargo add wove --git https://github.com/intuitums/wove --branch main
```

The [core guide](web/src/content/docs/start.mdx) starts with a runnable terminal app.
For components and signals, see the [Dioxus guide](web/src/content/docs/dioxus.mdx).

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
For an optional local speed check, run
`cargo run -p wove --release --example timing`.
For documentation changes, run `./x web` with Bun 1.4.2 or newer.

See [AGENTS.md](AGENTS.md) for repository conventions and
[architecture](web/src/content/docs/architecture.mdx) for the crate boundaries.

## Contributing

Contributions are welcome. Bring a small example of what you want to build or a
reproduction of a bug. [CONTRIBUTING.md](CONTRIBUTING.md) covers setup, checks,
documentation, and releases.

## License

[MIT](LICENSE).
