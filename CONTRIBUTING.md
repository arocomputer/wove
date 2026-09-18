# Contributing to Wove

Wove is a Rust library to build terminal user interfaces. Good contributions solve
a concrete application need, keep dependencies optional where practical, and leave
the code easy to understand and verify. Explain why a change needs a new crate,
configuration option, or abstraction before adding one.

## Getting set up

```sh
git clone https://github.com/intuitums/wove
cd wove
./x hooks
cargo build --workspace
./x test
```

Rustup uses the pinned `rust-toolchain.toml`. Install Python 3.12 or newer for
repository tooling and Bun 1.4.2 or newer for documentation work. Contributors
using the managed `~/Code` collection should follow its README and use a worktree.

Read [AGENTS.md](AGENTS.md) for the code map, focused test commands, and library
boundaries. The [architecture guide](crates/web/src/content/docs/architecture.mdx)
explains ownership and rendering. These rules apply to people and agents alike.

## Commit checks

Run `./x hooks` once in each contributing checkout or worktree. Setup preserves
an existing executable pre-commit hook and refuses to silently disable other hooks.
It configures only this worktree's hook path.

The hook checks staged whitespace, conflict markers, and Rust formatting. It reads
the staged files, leaves unstaged edits alone, and never rewrites or stages content.
Builds and tests remain separate. CI runs the repository guard and full formatting
checks, so local hooks are not the only verification.

## Reporting issues

Use the [bug or feature forms](https://github.com/intuitums/wove/issues/new/choose).
A bug report needs a minimal example, expected and actual behavior, enabled Cargo
features, and the affected version or commit. For terminal bugs, include the
terminal, operating system, and relevant resize or input sequence.

Feature requests should start with an application use case. Explain why existing
elements or a custom element cannot meet it. Implementation sketches are welcome;
a new package is not a requirement for a new capability.

Report security-sensitive findings privately using [SECURITY.md](SECURITY.md).
Never paste host keys, tokens, private application data, or unreviewed logs.

## Before opening a PR

```sh
./x check    # formatting, lint, tests, rustdoc, packaged consumers, guard
./x ui       # Unix PTY frames, input, resize, terminal cleanup
./x web      # documentation types and static site build
```

Run `./x check` for every submission, `./x ui` for rendering or terminal/input
changes, and `./x web` for documentation or website changes. The `unit` workflow checks Rust on Linux, macOS, and Windows. The `e2e` workflow
runs the PTY suite on Linux and macOS; `docs` builds the website. Workflow commands come from `./x`.

A regression test must fail on the original defect. Keep tests focused on public
behavior, and include a captured frame when appearance changes. Do not weaken
assertions to make a regression pass. Update comments and guides with code changes.
Describe user-visible changes and migration steps in the PR so maintainers can
prepare release notes. GitHub Releases are the changelog; do not keep a second
changelog file or roadmap.

The optional `cargo run -p wove --release --example timing` command measures local
frame timings. Include the setup and before/after measurements when claiming a
speed improvement. Wove has no performance budgets or benchmark CI gate.

## Review

Use a short branch such as `fix/input-selection`. PR titles use conventional
commits, for example `fix(core): preserve selection when undoing deletion`.
Optional scopes are `core`, `dioxus`, `keymap`, `ssh`, `web`, `infra`, and `docs`.
The title should make sense as a squash commit on main.

State the problem, resulting behavior, and validation. Keep each PR about one
coherent change; leave unrelated cleanup for another contribution. API and Cargo
feature changes need migration notes and updates to affected adapters and examples.
Do not promise compatibility or performance that has not been checked.

Maintainers decide whether a change merges. [CODEOWNERS](.github/CODEOWNERS) calls
out terminal output, SSH, dependencies, and automation for review; it does not by
itself configure branch protection. Passing checks are evidence for review, not
permission to merge or publish.

Main requires a pull request and a squash merge. The branch must be current,
review conversations resolved, and unit, e2e, docs, and audit checks
successful. The active repository rule has no bypass actors.

## AI/LLM assistance

AI-assisted issues and pull requests are welcome when the contributor owns the
result and can explain it.

- Review generated code, tests, prose, and commit messages before requesting review.
- Do not attribute commits to AI/LLM tools as author, co-author, committer, or
  signatory. Do not add `Assisted-by`, AI `Co-authored-by`, or model/harness footers.
- Answer maintainer questions yourself. Generated text is input to your response,
  not a substitute for understanding the change.
- Keep one AI-assisted pull request open at a time.

If you cannot explain or maintain the proposed change, revise or close the
submission rather than passing that responsibility to reviewers.

## Documentation

Published guides live in `crates/web/src/content/docs/`. Crate READMEs introduce
their package and link to the guides. Root markdown files describe repository
policy. Keep one authoritative account of each contract; use Git history for
past decisions. Web is a Bun/Astro package under `crates/`, excluded from Cargo.

## Releases

Wove is in development. Workspace version 0.0.1 is unpublished; the premature
`wove` 0.2.0 package is yanked. Commits and pull requests do not authorize releases.

Publishable packages are `wove`, `wove-dioxus`, `wove-keymap`, and `wove-ssh`.
Examples are private and Web is not a Cargo package.

After a maintainer explicitly authorizes a release:

1. Choose the version and update the workspace version and every exact internal
   dependency. Update Cargo.lock and migration guidance.
2. Run `./x check`, `./x ui`, and `./x web` on the release commit. Review platform
   CI and the scheduled dependency audit, including maintenance warnings.
3. Inspect the archives produced by `./x package`. Its external consumer verifies
   the extracted packages with optional features enabled and disabled.
4. If registry publication is authorized, publish core before its dependent
   packages. Verify owners and credentials; never place tokens in chat or Git.
5. Push `v<version>` only when the GitHub release is authorized. The `publish`
   workflow checks the version and uploads crate archives and checksums to a
   draft GitHub release. Generated PR notes are a starting point for editing.
6. Review the draft body using the format below, then publish it on GitHub. The
   workflow does not publish packages to crates.io.

Before 1.0, a published incompatible API change increments the minor version.
For the unpublished initial version, describe breaking changes in the PR.
Never publish from a pull request or weaken tag protection to run a release.

## Release notes

[GitHub Releases](https://github.com/intuitums/wove/releases) are the published
history. Write a short release title and introduction, followed by these groups
in order. Omit empty groups.

```markdown
### Release title

A short explanation of the changes that matter to callers.

### New features
- Describe the new capability and where to use it.

### Improvements
- **Upgrade:** Describe required migration steps before other improvements.

### Fixes
- **Security:** Describe security fixes before other fixes.
```

Use concrete behavior, not commit subjects or implementation inventories.
The release tag provides the version and GitHub supplies the publication date.
Keep unreleased details in PRs until preparing a draft release. A future website
changelog should consume published releases rather than duplicate their contents.

## Website

`crates/web/` contains the Astro website and MDX guides, using Bun 1.4.2 or newer.
Run `./x web` for dependency, type, and build checks.

### Deployment

`.github/workflows/deploy.yml` runs `./x web`, uploads `crates/web/dist/`, and
deploys to GitHub Pages on pushes to `main`. It can also be run manually from
`main`. Pull requests only run checks. Deployment uses GitHub's workflow
credentials and does not require a separate hosting secret.

The production origin is `https://wovetui.com`, configured in
`crates/web/astro.config.mjs`. To set up hosting:

1. An `intuitums` organization owner must allow public GitHub Pages sites in
   the organization's Settings → Member privileges → Pages creation.
2. In the repository's Settings → Pages, select **GitHub Actions** as the source
   and save `wovetui.com` as the custom domain before changing DNS.
3. In Cloudflare DNS, replace conflicting records for the apex and `www` with
   the following records. Set each to **DNS only**, with the proxy disabled.

   | Type | Name | Target |
   | --- | --- | --- |
   | A | `@` | `185.199.108.153` |
   | A | `@` | `185.199.109.153` |
   | A | `@` | `185.199.110.153` |
   | A | `@` | `185.199.111.153` |
   | CNAME | `www` | `intuitums.github.io` |

   Remove old apex AAAA records or replace them with GitHub Pages IPv6 records
   from the [custom-domain documentation](https://docs.github.com/en/pages/configuring-a-custom-domain-for-your-github-pages-site/managing-a-custom-domain-for-your-github-pages-site).
   Preserve unrelated records, including email records.
4. Merge the deployment workflow to `main`. After DNS propagates and GitHub
   provisions the certificate, enable **Enforce HTTPS** in Settings → Pages.
5. Verify the homepage, `/docs/`, an unknown URL returning the 404 page, and
   the redirect from `www.wovetui.com` to `wovetui.com` over HTTPS.

GitHub Pages manages the custom domain in repository settings. This Actions
deployment does not require a `CNAME` file in the build output.


## License

Contributions are released under the repository's [MIT license](LICENSE).
