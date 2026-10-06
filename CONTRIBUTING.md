# Contributing to Wove

Wove is a Rust library for building terminal user interfaces. Good contributions solve
a concrete application need, keep dependencies optional where practical, and leave
the code easy to understand and verify. Explain why a change needs a new crate,
configuration option, or abstraction before adding one.

## Getting set up

```sh
git clone https://github.com/arocomputer/wove
cd wove
./x hooks
cargo build --workspace
./x test
```

Rustup uses the pinned `rust-toolchain.toml`. Install Python 3.12 or newer for
repository tooling and Bun 1.4.2 or newer for documentation work.

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

Use the [bug or feature forms](https://github.com/arocomputer/wove/issues/new/choose).
A bug report needs a minimal example, expected and actual behavior, enabled Cargo
features, and the affected version or commit. For terminal bugs, include the
terminal, operating system, and relevant resize or input sequence.

Feature requests should start with an application use case. Explain why existing
elements or a custom element cannot meet it. Implementation sketches are welcome;
a new package is not a requirement for a new capability.

Report security-sensitive findings privately using [SECURITY.md](SECURITY.md).
Never paste host keys, tokens, private application data, or unreviewed logs.

## Before opening a PR

Choose verification by the changed contract:

| Change | Checks |
| --- | --- |
| Rust APIs, implementations, or dependencies | `./x check` |
| Rendering, input, or terminal lifecycle | Also `./x ui` |
| Published docs or website | `./x web` |
| Quickstart or the core API it uses | Also `./x guides` |
| Repository prose only | `git diff --check` |

Use package commands such as `./x ssh` for fast development loops. A regression
test must fail on the original defect. Update affected consumers and guides with
contract changes; review captured frames before updating PTY goldens.

## Continuous integration

`.github/workflows/ci.yml` selects changes once and reports one required result,
**CI**. That result succeeds only when selection succeeded and all selected jobs
passed. Unexpected skips, cancellations, and failures fail the gate. Unaffected
jobs visibly skip rather than allocating runners to report success.

Shared formatting, Clippy, rustdoc, packaged-consumer, and tooling checks run
once on Linux. Package tests run on Linux, macOS, and Windows for implementation
or dependency changes; Core and Dioxus also run PTY scenarios on Unix. Rust caches
are separated by job and platform and only saved by trusted pushes to main.

| Changed paths | Selected work |
| --- | --- |
| Core implementation | Core and all four consumers on three platforms, shared quality, quickstart compilation |
| Adapter implementation | That package on three platforms and shared quality |
| Platform-independent package tests only | That package on Linux and shared quality |
| Published docs/site or root README | Website build and generated-link checks |
| Quickstart | Website plus compilation of its actual Rust source |
| Repository prose or GitHub templates | Diff validation and the final gate; no builds |
| PTY tooling | Core and Dioxus PTYs on Unix; no Windows or unit suites |
| Cargo.lock | Old/new dependency graphs select affected owners, shared quality, and Cargo audit |
| Bun manifests/lockfile | Website and Bun audit |
| PTY dependency pins | PTYs, tooling checks, and Python audit |
| Maintenance workflows, tool pins, and reporting scripts | Tooling tests and workflow scans |
| Fuzz targets | Tooling tests and compilation of the two fuzz targets |
| Shared CI, `x`, or unknown inputs | All checks |

A website-only PR needs three running jobs after settings migration: selection,
website, and the final gate. A policy-only PR needs two. A test-only PR needs four.
Core contract changes retain all consumer/platform coverage; they are deliberately
larger. Inspect the plan in the workflow summary or locally with:

```sh
./x affected plan --base origin/main
```

Platform-specific or deleted test files retain all three package platforms.

PR selection uses the merge base, checks both sides of renames, and validates the
diff. Main pushes check the entire pushed range. Unknown inputs and ambiguous
lockfile graphs select conservatively; selection errors cannot turn into skips.
Manual runs check everything. Weekly runs audit Cargo, Bun, and Python without
rebuilding unrelated packages.

### Repository maintenance

`./x workflows` runs checksum-pinned actionlint and offline zizmor. It runs in
change selection even when policy changes select no builds. `./x quality` runs
it locally too. Tool versions and Linux/macOS hashes live in
`.github/infra-tools.json`; update both when upgrading. Release checks restore
neither Rust build caches nor Bun setup caches. Published crate archives receive
GitHub build attestations alongside their checksums and source identity.

Weekly `links` checks external README and guide links with bounded retries.
External availability stays outside the required PR gate; generated internal
links and anchors remain part of `./x web`. Link reports expire after fourteen
days. Closing a PR deletes only that PR's merge-ref caches, after snapshotting
all pages; main and active-PR caches remain intact.

The scheduled reporter opens one bot-owned issue per failing CI, link, or fuzz
workflow, updates it on repeated failures, and closes it after recovery. Healthy
runs stay quiet. It reads metadata only and uses trusted default-branch code;
it never executes the triggering run's code or artifacts. Closed-PR cache
cleanup follows the same trusted-code boundary.

The `fuzz` workspace exercises input fragmentation and bounded paste delivery,
and verifies arbitrary text cannot place controls in rendered cells. Run
`./x fuzz-check` locally; changed targets compile in the quality job. Scheduled
fuzzing uses pinned nightly/cargo-fuzz, bounded inputs and execution time, and
retains crash inputs for fourteen days. Turn discovered crashes into deterministic
core regression tests. Cargo audits include this active development lockfile.

These schedules and privileged housekeeping activate only after merge to main.

### Migrating required checks and protecting releases

The workflow temporarily forwards the seven existing required check names to
CI. This keeps current branch rules working while the change is reviewed and
merged. An administrator then runs:

```sh
./x settings          # inspect the proposed changes using authenticated gh
./x settings --apply  # apply only after CI passed on current main
```

The command preserves existing main protections and review policy, replaces its
required checks with CI, restricts creation of `v*` tags to repository admins,
and forbids moving or deleting release tags. Only after the new gate is enforced
does it set `WOVE_LEGACY_CHECKS=false` to skip compatibility jobs. It backs up the
previous main protection policy in `artifacts/settings/`. Do not manually disable compatibility
checks before migrating protection. A failed settings call must be resolved and
rerun; it does not establish that protection changed.

CODEOWNERS routes sensitive changes to the maintainer. With the current solo
maintainer, independent approval is not required. When a second maintainer joins,
require an approving review and CODEOWNER approval for sensitive paths.

GitHub Releases are the changelog; describe user-visible changes and migration
steps in PRs. The optional `timing` example provides local measurements, not a
performance gate. Include setup and before/after evidence for performance claims.

## Review

Use a short branch such as `fix/input-selection`. PR titles use conventional
commits, for example `fix(core): preserve selection when undoing deletion`.
Optional scopes are `core`, `dioxus`, `keymap`, `ssh`, `web`, `infra`, and `docs`.
The title should make sense as a squash commit on main.

Use the [PR template](.github/pull_request_template.md). Link a related issue when
one exists, select the change type, explain the problem and why the change works,
and list verification commands and results. Include screenshots or captured frames
for visual changes; remove that section when it does not apply. Write enough detail
to review the change without a fixed sentence limit.

Keep each PR about one coherent change; leave unrelated cleanup for another
contribution. API and Cargo feature changes need migration notes and updates to
affected adapters and examples. Do not promise compatibility or performance that
has not been checked.

PRs do not use labels. Change types belong in the title and template, not automatic
path-based or dependency labels. Issues can still use labels. Template guidance is
for review; automation does not label or close PRs for template formatting.

Maintainers decide whether a change merges. [CODEOWNERS](.github/CODEOWNERS) calls
out terminal output, SSH, dependencies, and automation for review; it does not by
itself configure branch protection. Passing checks are evidence for review, not
permission to merge or publish.

Main requires a pull request and a squash merge. The branch must be current,
review conversations resolved, and the required CI gate successful. Existing check
names are retained until the settings migration above. Main has no bypass actors.

## Agent and AI assistance

Agent-assisted issues and pull requests are welcome. A named contributor must
review the result, understand the change, answer maintainer questions, and address
feedback. Generated text is input to their work, not a substitute for understanding.

- Review generated code, tests, prose, and commit messages before requesting review.
- Preserve accurate authorship and provenance. Cloud-service bot authors or
  committers, platform signatures, and attribution trailers or PR markers are allowed.
- Git author, committer, pusher, PR creator, and signer are separate identities.
  A service identity does not replace the accountable contributor.
- Do not impersonate another contributor or rewrite history simply to hide agent use.
- Keep PRs coherent and ready for review. Multiple independently reviewed agent
  contributions are allowed; maintainers may limit their review queue by capacity.

If you cannot explain or maintain a change, revise it before requesting review.
Do not pass that responsibility to reviewers.

## Documentation

Published guides live in `crates/web/src/content/docs/`. Crate READMEs introduce
their package and link to the guides. Root markdown files describe repository
policy. Keep one authoritative account of each contract; use Git history for
past decisions. Web is a Bun/Astro package under `crates/`, excluded from Cargo.

## Releases

Wove is in development. Workspace version 0.0.1 is unpublished; the premature
`wove` 0.2.0 package is yanked. Commits and pull requests do not authorize releases.

Publishable packages are `wove`, `wove-dioxus`, `wove-keymap`, `wove-ssh`, and `wove-gpu`.
Examples are private and Web is not a Cargo package.

After a maintainer explicitly authorizes a release:

1. Choose the version and update the workspace version and every exact internal
   dependency. Update Cargo.lock and migration guidance.
2. Run `./x check`, `./x ui`, `./x web`, `./x guides`, and `./x audit` on the
   release commit. Review platform CI and advisory maintenance warnings.
3. Inspect the archives produced by `./x package`. Its external consumer verifies
   the extracted packages with optional features enabled and disabled.
4. If registry publication is authorized, publish `wove` first, then Dioxus,
   Keymap, SSH, and GPU. Verify owners and credentials; never place tokens in chat
   or Git. Prefer crates.io trusted publishing through GitHub OIDC, restricted to
   this repository, its release workflow, and an approved publishing environment.
   Configure a trusted publisher for each package with owner `arocomputer`, repo
   `wove`, workflow `registry.yml`, and environment `crates-io`. Configure that
   environment to allow main and require a maintainer's approval. The manual
   **Publish registry packages** workflow runs only from main for an authorized
   existing tag, packages and checksums the archives before any other tooling runs,
   verifies all release checks without credentials, and only then publishes those
   archives in a separate job using a temporary OIDC token.
   Upload metadata is prepared without credentials; the publishing job runs no
   crate code and verifies each published archive checksum before its consumers. No long-lived
   registry token is needed. Reruns skip identical published versions and reject
   conflicting or yanked versions. Account-side trusted-publisher setup is required;
   adding the workflow does not configure crates.io or authorize a release.
5. Push `v<version>` only when the GitHub release is authorized. The `publish`
   workflow rejects tags outside main, checks exact internal dependency versions,
   packages and checksums the archives before other tooling runs,
   reruns Rust/PTY/site/guide/audit checks, and uploads those archives, checksums, and
   source commit metadata and build attestations to a draft GitHub release. Generated notes need review.
6. Review the draft body using the format below, then publish it on GitHub. The
   workflow does not publish packages to crates.io.

Before 1.0, a published incompatible API change increments the minor version.
For the unpublished initial version, describe breaking changes in the PR.
Never publish from a pull request or weaken tag protection to run a release.
GitHub draft-release creation and registry publication are separate actions; neither
a commit nor a successful CI run starts registry publication.

## Release notes

[GitHub Releases](https://github.com/arocomputer/wove/releases) are the published
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

The website job in `.github/workflows/ci.yml` runs `./x web`. On pushes to `main` that affect
the site, it uploads `crates/web/dist/` and deploys to GitHub Pages. A manual run
from `main` rebuilds and deploys the site. Pull requests only run checks.
Deployment waits for CI, uses GitHub’s workflow credentials, and needs no separate
hosting secret. A post-deployment smoke check verifies HTTPS, docs, the 404 page,
and the canonical `www` redirect. Run `./x smoke` to repeat those checks.

The production origin is `https://wovetui.com`, configured in
`crates/web/astro.config.mjs`. To set up hosting:

1. An `arocomputer` organization owner must allow public GitHub Pages sites in
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
   | CNAME | `www` | `arocomputer.github.io` |

   Remove old apex AAAA records or replace them with GitHub Pages IPv6 records
   from the [custom-domain documentation](https://docs.github.com/en/pages/configuring-a-custom-domain-for-your-github-pages-site/managing-a-custom-domain-for-your-github-pages-site).
   Preserve unrelated records, including email records.
4. Merge the deployment workflow to `main`. After DNS propagates and GitHub
   provisions the certificate, enable **Enforce HTTPS** in Settings → Pages.
5. Verify the homepage, `/docs/`, an unknown URL returning the 404 page, and
   the redirect from `www.wovetui.com` to `wovetui.com` over HTTPS.

GitHub Pages manages the custom domain in repository settings. This Actions
deployment does not require a `CNAME` file in the build output.

### Rollback

Revert the problematic site change through a PR; the successful main build deploys
that revert. Keep the revert scoped to the affected site files. For an urgent rollback,
restore the affected site files from a known successful revision in a corrective
PR; merge it through the same CI gate. A manual workflow run on main redeploys
the current source, so it does not by itself restore an older version. Verify `./x smoke`
after either path. Do not move release tags to roll back a website.

## License

Contributions are released under the repository's [MIT license](LICENSE).
