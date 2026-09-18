# Releasing

The workspace packages are `weft-core` and `weft-dioxus`. No registry release has
been made. Package names remain provisional until the first publication.

1. Run `./x check`, `./x ui`, and `./x bench` on the release commit.
2. Choose the workspace version, update the adapter's exact core dependency,
   move Unreleased changes into that version, update Cargo.lock, and commit.
3. `./x package` builds both archives and runs an external consumer against their
   extracted contents, with and without the terminal backend.
4. After a maintainer authorizes registry publication, publish core first, then
   the adapter. Verify package ownership and credentials before the first release.
5. Tag the release commit `v<version>` and push the tag. The workflow checks the
   version and publishes both archives and their checksums to a GitHub release.
   It does not publish to crates.io.

Actions are pinned by commit. Only the release job has contents write permission.
Do not publish from a pull request.
