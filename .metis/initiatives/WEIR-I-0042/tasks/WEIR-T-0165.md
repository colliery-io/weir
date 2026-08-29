---
id: release-artifacts-that-work-trunk
level: task
title: "Release artifacts that work: trunk + staged connectors in tarballs, ghcr.io image publish"
short_code: "WEIR-T-0165"
created_at: 2026-08-16T15:24:01.663940+00:00
updated_at: 2026-08-25T02:21:27.544856+00:00
parent: WEIR-I-0042
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0042
---

# Release artifacts that work: trunk + staged connectors in tarballs, ghcr.io image publish

## Parent Initiative

[[WEIR-I-0042]]

## Objective **[REQUIRED]**

Release artifacts are unusable and nothing is published: `release.yml` runs plain `cargo build --release` — no trunk step, so `weir-api/build.rs` silently embeds an EMPTY web UI — and stages no connectors or manifests; meanwhile the Helm chart's default image `ghcr.io/colliery-io/weir:latest` is pushed by nothing. Make a tagged release produce artifacts that actually run pipelines, and publish the container image.

## Evidence (2026-08-16 alpha review)

- `.github/workflows/release.yml` — cargo build + tar only; no trunk, no connector staging, no image push (verified).
- `crates/weir-api/build.rs` — silently embeds an empty UI set when `weir-ui/dist` is absent (verified).
- `scripts/stage-connectors.sh` — works; release never calls it.
- `charts/README.md` — default image `ghcr.io/colliery-io/weir:latest` is a dangling reference.
- `Dockerfile` — already builds a complete self-contained image (UI + staged connectors + vendored manifests).
- `.angreal/task_version.py` — `angreal version verify` exists but is wired into nothing; CHANGELOG.md is an 8-line stub; workspace version 0.0.1.

## Acceptance Criteria **[REQUIRED]**

- [x] release.yml builds the UI (trunk) before cargo build, and tarballs include staged connectors + vendored manifests/ and dest-manifests/ (plus a README.txt with env-var run instructions)
- [x] A release job builds and pushes the Docker image to ghcr.io/colliery-io/weir tagged with the version and latest (per-arch native builds on ubuntu-latest + ubuntu-24.04-arm, stitched with `docker buildx imagetools create` — no QEMU)
- [x] `weir-api/build.rs` fails loudly when a release build has no `weir-ui/dist`: always emits a cargo:warning, and hard-fails when `WEIR_REQUIRE_UI` is set (set by release.yml and the Dockerfile) — the panic path was verified by hiding dist locally
- [x] Tag ↔ Cargo version consistency enforced: a `verify-version` job gates build+image, comparing the tag to the workspace version
- [x] CHANGELOG.md carries a real Unreleased entry targeting v0.1.0 with the v0-unstable policy stated; the Helm chart default image resolves once the first tagged release runs

## Implementation Notes

Caveats from the review: integration.yml/connectors-live.yml carry "Actions are disabled repo-wide while private" notes — confirm Actions are actually enabled on the public repo before trusting any of this pipeline. Wasm connector builds need the wasm32-wasip2 target on the release runner; the ghcr push job needs packages:write. Soft ordering: land before the next v* tag is cut.

## Status Updates **[REQUIRED]**

**2026-08-18/25 — implemented + verified (ralph run).**

- `.github/workflows/release.yml` rewritten: `verify-version` gate (tag == workspace Cargo version, dependency-free bash); per-target build jobs install the pinned toolchain + wasm targets + trunk (same pattern as ui-e2e.yml), build the UI, run `scripts/stage-connectors.sh`, build with `WEIR_REQUIRE_UI=1`, and package `weir + README.txt + connectors/ + manifests/ + dest-manifests/` tarballs; `image` jobs build per-arch on native runners (amd64 + arm64, no QEMU emulation of a Rust build) pushing `:vX.Y.Z-{arch}` tags; `publish-image` stitches `:X.Y.Z` + `:latest` manifests via `docker buildx imagetools create`; the GitHub Release job depends on both. `permissions: packages: write` added. YAML validity checked.
- `crates/weir-api/build.rs`: missing `weir-ui/dist` now always emits a `cargo:warning`, and panics with a clear message when `WEIR_REQUIRE_UI` is set (used by release.yml + Dockerfile). Panic path verified live by temporarily hiding dist.
- `Dockerfile`: cargo build now runs with `WEIR_REQUIRE_UI=1`; toolchain install restructured onto `rust-toolchain.toml` (see [[WEIR-T-0163]]'s status for the target bug this fixed) as a cacheable pre-source layer. The full image build + cold-start ran green on this machine (195MB image).
- `CHANGELOG.md`: real Unreleased entry targeting v0.1.0, stating the [[WEIR-A-0006]] v0-unstable policy and this wave's changes.
- Caveats for the human: the pipeline itself can only be end-to-end proven by pushing a `v*` tag with GitHub Actions enabled on the public repo (the workflows carry "Actions disabled while private" notes — confirm before the first tag); bump the workspace version (`angreal version bump`) before tagging or `verify-version` will refuse.

**2026-08-29 — first live run (v0.0.1-alpha) + macOS libpq fix.** The first real tag exercise (run 33250729103) proved most of the pipeline: verify-version passed, the aarch64-linux tarball leg and BOTH image jobs succeeded, and `ghcr.io/colliery-io/weir:0.0.1-alpha` + `:latest` published (the Helm charts' dangling default image now resolves). One real gap surfaced: the aarch64-apple-darwin leg failed at link — `ld: library 'pq' not found` — diesel's pq-sys links libpq dynamically and macOS runners keep brew's libpq keg-only, off the linker path (Linux runner images ship it). Fixed in release.yml: a macOS-only step installs libpq and exports `PQ_LIB_DIR`; the tarball README.txt now states the dynamic runtime libraries (libpq, sqlite3, bzip2) with per-OS install lines. Tag force-moved to the fixed commit (nothing had consumed it — no Release page existed) and re-run. Note for the future: the shipped binaries link libpq/sqlite3/bz2 dynamically; a bundled/static build (pq-src, libsqlite3-sys bundled) is the follow-up if portability complaints arrive.

**2026-08-29 — scope decision: Linux-only artifacts.** Mid-rerun the user cut the darwin leg entirely: the release surface is Linux tarballs (x86_64 + aarch64/Graviton) plus the multi-arch amd64/arm64 image — macOS users run the container. release.yml drops the aarch64-apple-darwin matrix entry and the libpq workaround; the tag was moved to the Linux-only commit and re-cut (run 33254504699).
