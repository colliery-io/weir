---
id: release-artifacts-that-work-trunk
level: task
title: "Release artifacts that work: trunk + staged connectors in tarballs, ghcr.io image publish"
short_code: "WEIR-T-0165"
created_at: 2026-08-16T15:24:01.663940+00:00
updated_at: 2026-08-16T15:24:01.663940+00:00
parent: WEIR-I-0042
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/todo"


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

- [ ] release.yml builds the UI (trunk) before cargo build, and tarballs include staged connectors + vendored manifests/ and dest-manifests/ — OR the release explicitly designates the container image as the primary artifact and states the tarball's limits in the release notes
- [ ] A release job builds and pushes the Docker image to ghcr.io/colliery-io/weir tagged with the version and latest
- [ ] `weir-api/build.rs` fails loudly (or emits an unmissable warning) when a release-profile build has no `weir-ui/dist` — no more silently-headless binaries
- [ ] Tag ↔ Cargo version consistency enforced in the release path (`angreal version verify` or equivalent)
- [ ] CHANGELOG.md carries a real entry for the first published release; the Helm chart default image resolves after a release runs

## Implementation Notes

Caveats from the review: integration.yml/connectors-live.yml carry "Actions are disabled repo-wide while private" notes — confirm Actions are actually enabled on the public repo before trusting any of this pipeline. Wasm connector builds need the wasm32-wasip2 target on the release runner; the ghcr push job needs packages:write. Soft ordering: land before the next v* tag is cut.

## Status Updates **[REQUIRED]**

*To be added during implementation*
