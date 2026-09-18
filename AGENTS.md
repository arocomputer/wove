# Working on Wove

Wove is a Rust library for building terminal user interfaces. Capitalize the project
name in prose; keep package names and Rust imports lowercase.
It must stand on its own across applications. Do not add agent, provider,
session, or other consumer-specific concepts to the library.

- Preserve unrelated changes and never expose secrets.
- Follow ~/Code/README.md. Use managed worktrees, never edit base checkouts.
- Read CONTRIBUTING.md and crates/web/src/content/docs/architecture.mdx before changing contracts.
- Use the smallest concrete design that meets the current requirement.
- Document public items and nontrivial private functions by purpose and contract.
- Keep rendering deterministic and testable without a terminal.
- No unsafe code in library modules. Terminal access stays in the optional backend.
- Keep crates/web/ minimal until website design is requested.
- Run ./x hooks once per worktree, ./x check for changes, ./x ui for rendering
  or terminal changes, and ./x web for documentation or website changes.
  Keep optional speed checks beside core; do not add performance budgets or CI gates.
- Tests pin behavior, not implementation details. Keep docs current.
- Keep published documentation in crates/web/src/content/docs/. Crate READMEs introduce
  their package; CONTRIBUTING.md covers contribution and release instructions.
- `main` is the default branch. Use short type/name topic branches, such as
  fix/resize, unless the user authorizes committing directly to main.
  Follow the naming rules in CONTRIBUTING.md.
- Do not open a pull request unless explicitly asked.
- Do not claim support or performance without a reproducible check.
