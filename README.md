<p align="center">
  <a href="https://wovetui.com">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="crates/web/public/logo/dark.svg">
    <img src="crates/web/public/logo/light.svg" alt="Wove logo" height="42">
  </picture>
  </a>
</p>

<div align="center">
  <a href="https://github.com/intuitums/wove/actions/workflows/checks.yml"><img alt="Build status" src="https://img.shields.io/github/actions/workflow/status/intuitums/wove/checks.yml?branch=main&amp;style=flat-square" /></a>
</div>

A Rust library for building terminal user interfaces.

- Build with Rust elements or Dioxus components and signals.
- Arrange content with flexbox and grid layouts.
- Add text, inputs, selects, panels, and scrolling views.
- Handle keyboard and mouse input, with text selection and undo.
- Test rendering and input without opening a terminal.

Wove is early in development, and its API is still changing.

[Documentation](crates/web/README.md)

## Libraries

| Library | Purpose |
| --- | --- |
| [Core](crates/core) | Elements, layout, text editing, rendering, and optional formatting. |
| [Dioxus](crates/dioxus) | Optional RSX components, signals, and lifecycles. |
| [Keymap](crates/keymap) | Scoped command bindings and key sequences. |
| [SSH](crates/ssh) | Authenticated remote terminal applications. |

[Examples](crates/examples) contains runnable applications.
[Web](crates/web) contains the documentation and minimal Astro setup.
All packages live under `crates/`; Web uses Bun and is excluded from Cargo.

Core offers optional `markdown`, `syntax`, and `diff` features.
Enable only the features your application uses.

## Get started

Add the current Wove source to your Rust project. Version 0.0.1 is not yet
published. The earlier crates.io version was withdrawn:

```sh
cargo add wove --git https://github.com/intuitums/wove --branch main
```

The [core guide](crates/web/src/content/docs/start.mdx) starts with a runnable terminal app.
For components and signals, see the [Dioxus guide](crates/web/src/content/docs/dioxus.mdx).

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
[architecture](crates/web/src/content/docs/architecture.mdx) for the crate boundaries.

## Contributing

Contributions are welcome. Bring a small example of what you want to build or a
reproduction of a bug. [CONTRIBUTING.md](CONTRIBUTING.md) covers setup, checks,
documentation, and releases.

## License

MIT.
