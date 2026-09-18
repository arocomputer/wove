# Working on weft

weft is a Rust library to build terminal user interfaces. Keep the name lowercase.
It must stand on its own across applications. Do not add agent, provider,
session, or other consumer-specific concepts to the library.

- Preserve unrelated changes and never expose secrets.
- Follow ~/Code/README.md. Use managed worktrees, never edit base checkouts.
- Read CONTRIBUTING.md and docs/architecture.md before changing contracts.
- Use the smallest concrete design that meets the current requirement.
- Document public items and nontrivial private functions by purpose and contract.
- Keep rendering deterministic and testable without a terminal.
- No unsafe code in library modules. Terminal access stays in the optional backend.
- Run ./x hooks once per worktree, ./x check for changes, ./x ui for rendering
  or terminal changes, and ./x bench for performance-sensitive changes.
- Tests pin behavior, not implementation details. Keep docs and changelog current.
- Use a conventional type/slug branch before pushing. Never push main.
- Do not open a pull request unless explicitly asked.
- Do not claim support or performance without a reproducible check.
