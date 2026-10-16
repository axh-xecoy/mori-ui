---
name: mori-release
description: Prepare and publish Windows releases of the Mori Rust/GPUI chat client, synchronize version metadata, write annotated release tags, and follow GitHub Actions through publication. Use for Mori release preparation or an explicitly requested Mori version publication.
---

# Mori release

Work in the Mori checkout identified by Cargo package `mori-ui` and `.github/workflows/release.yml`. Read the checkout's [release guide](../../../docs/RELEASING.md) for packaging commands and tag-note behavior. In a globally installed copy, locate that guide from the current Mori repository root instead of resolving this relative link.

## Prepare the requested version

- Use the version requested by the user. Inspect the working tree, current branch, configured remote and existing local/remote tags before changing release state. Preserve unrelated user changes.
- Keep the Cargo package version (`Cargo.toml` and root package entry in `Cargo.lock`) and Windows version resource (`assets/mori.rc`, numeric and string values) consistent. The settings UI reads `CARGO_PKG_VERSION`; check for any other stale hardcoded UI version.
- Use only `target/release` for build and package outputs. Run `cargo build --release --locked` and `./scripts/package-release.ps1`; do not create a `dist` directory or override Cargo's output with `cargo rustc -o`.
- Run formatting, relevant tests and the repository's required checks. Check executable version metadata, ZIP contents and SHA-256 files. Do not modify user data to validate a release.

## Publish when authorized

Preparing files or reading this skill does not authorize publication. A user request to publish a specific version is authorization for the necessary release commit, push, annotated tag and automated GitHub Release; do not ask again for the same action.

- Write concise user-facing Markdown notes from verified behavior. Distinguish existing limitations from implemented features. Store the notes in a UTF-8 file and pass it to `git tag -a vVERSION -F FILE` rather than assembling shell-escaped multiline text.
- Commit release changes, push the release commit, then push the requested annotated tag. Do not create a tag that already exists, move a published tag, or delete an existing Release without explicit authorization.
- The `v*` tag workflow builds Windows x86_64, verifies the artifacts, and publishes `mori.exe`, `mori-windows-x86_64.zip`, and `SHA256SUMS.txt`. Annotated tag text supplies the Release body; a hyphenated tag is a prerelease.
- Follow the matching Actions run through build and publish. Use available GitHub tools/CLI or the public REST API without exposing credentials. Verify the published Release tag, target commit, body and all three asset names; verify artifact digests when downloadable.
- Investigate a failed run before retrying. One retry is reasonable for a demonstrated transient infrastructure failure. Source or workflow failures need a concrete fix; do not repeatedly retry unchanged failures or silently force-move the tag to a new commit.
- Report the real outcome: include the Release URL when publication succeeds. A pushed tag, queued run or successful local build alone does not prove a release was published; if blocked, name the failed step and retain recoverable build outputs.
