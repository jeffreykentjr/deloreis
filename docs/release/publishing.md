---
status: live
verdict: "How to prepare release candidates and publish De Lore Is."
---

# Publishing a release

The workspace version in `Cargo.toml` is the release version. The `deloreis`
command and its `dli-*` dependencies are published together. The wire contract
has its own version.

Routine CI runs on Linux. It does not build macOS archives or run on a weekly
schedule. Release archives are built only when their workflows are started
manually:

- **Linux x86-64:** `Build Linux release archive` accepts a matching draft
  release or release candidate.
- **macOS ARM:** `Build approved macOS RC archive` accepts only a matching
  draft prerelease tagged `vVERSION-rc.N`. It first validates the candidate on
  Linux; the macOS job then waits for approval in the `macos-rc` environment.
  Do not start it without the owner's approval of that candidate.

The `macos-rc` environment must have a required human reviewer configured.
An environment name by itself does not provide a review gate. Never remove that
protection or approve a run on the owner's behalf.

When a candidate is ready:

1. Merge the candidate changes and wait for main's checks to pass. Check the
   version, changelog and clean checkout.
2. Run `python3 scripts/publish-crates.py --dry-run --version VERSION`. It
   verifies all release crates without reading a token or publishing them.
3. Create an annotated `vVERSION-rc.N` tag on the checked commit and a draft
   GitHub prerelease for that tag. Run the Linux archive workflow if needed.
4. After the owner approves that candidate, dispatch the macOS RC workflow
   with its tag and full commit SHA. The environment review must pass before a
   macOS runner starts. Do not run it for ordinary commits or unapproved RCs.
5. Verify the two archives, their SHA-256 files and attestations. Test the Linux
   archive in a clean environment. Preserve the approved macOS RC archive and
   its attestation when carrying it into the stable release.
6. When the release is ready, tag the same checked commit `vVERSION` and
   create its stable release as a draft. Run the Linux workflow for the stable
   tag. Copy the approved RC's macOS ARM archive and checksum to that draft
   without modifying them; verify that the RC and stable tags name the same
   commit and that the copied archive's attestation still verifies.
7. Check both archives and their checksums, then publish the stable release.
   The `Publish crates` workflow verifies it and publishes the crates using the
   `CARGO_REGISTRY_TOKEN` Actions secret. Prereleases do not start publication.
8. Confirm `cargo install deloreis --locked` in a clean environment.

For a partial crate publication, rerun `Publish crates` with the published tag
and full commit SHA. It skips a published version only if the registry checksum
matches the locally verified package; a mismatch or yanked version stops the
release. Versions on crates.io cannot be replaced.

A checksum verifies that a download matches its accompanying checksum file.
To check GitHub's signed provenance as well, run:

```sh
gh attestation verify deloreis-0.1.0-x86_64-unknown-linux-gnu.tar.gz --repo jeffreykentjr/deloreis
```
