---
status: live
verdict: "How to publish De Lore Is crates and macOS and Linux archives."
---

# Publishing a release

The workspace version in `Cargo.toml` is the release version. The command-line
crate is `deloreis`; its `dli-*` dependencies are published with it. The wire
contract has its own version.

1. Merge the release changes and wait for main's checks to pass. Keep the
   checkout clean and check the version and changelog.
2. Run `python3 scripts/publish-crates.py --dry-run --version VERSION`. It
   packages and verifies all release crates, checks their licenses and bundled
   example, and checks the public registry without reading a token.
3. Tag the checked main commit `vVERSION` and push the tag. Create a draft
   GitHub release for that tag.
4. Run the `Build release archives` workflow with the tag and full commit SHA.
   It builds and smoke-tests Intel and ARM archives on Linux and macOS, includes
   dependency and Rust standard-library notices, records attestations, checks
   the SHA-256 files, and attaches them to the draft.
5. Check the draft, install an archive in a clean environment, and publish the
   release. The `Publish crates` workflow verifies the same tag and commit,
   then publishes the crates using the `CARGO_REGISTRY_TOKEN` Actions secret.
6. Confirm `cargo install deloreis --locked` in a clean environment and check
   that the release page contains all four archives, checksums and attestations.

For a partial crate publication, rerun `Publish crates` with the published tag
and full commit SHA. It skips a published version only if the registry checksum
matches the locally verified package; a mismatch or yanked version stops the
release. Versions on crates.io cannot be replaced.

A checksum verifies that a download matches its accompanying checksum file.
To check GitHub's signed provenance as well, run:

```sh
gh attestation verify deloreis-0.1.0-x86_64-unknown-linux-gnu.tar.gz --repo jeffreykentjr/deloreis
```
