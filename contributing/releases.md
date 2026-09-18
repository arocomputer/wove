# Releasing

The package is `intuitums-weft`; consumers normally alias it to `weft`.
No crates.io release has been made. Check name availability before the first
publication and configure the maintainer's publishing credentials locally.

1. Run `./x check`, `./x ui`, and `./x bench` on the release commit.
2. Choose the workspace version, move Unreleased changes into that version,
   update Cargo.lock, and commit the release preparation.
3. Run `cargo publish --dry-run --locked -p intuitums-weft`.
4. After explicit maintainer approval, run `cargo publish --locked -p intuitums-weft`.
5. Tag that commit `v<version>` and push the tag. The release workflow checks
   the tag against Cargo metadata, verifies the package, and creates a GitHub
   release with the crate archive and checksum. It does not publish to crates.io.

The workflow uses read-only permissions except for the release job's contents
permission. Actions are pinned by commit. Do not publish from a pull request.
