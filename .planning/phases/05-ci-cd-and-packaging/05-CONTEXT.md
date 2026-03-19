# Phase 5: CI/CD and Packaging - Context

**Gathered:** 2026-03-19
**Status:** Ready for planning

<domain>
## Phase Boundary

Automated CI pipeline running all checks (fmt, clippy, unit tests, integration tests, RF E2E tests, cargo-deny) on every push and PR, plus a release workflow that builds and publishes pre-built x86_64-linux binaries to GitHub Releases on version tags.

</domain>

<decisions>
## Implementation Decisions

### Release workflow
- Tag pattern: `v*` semver (e.g., v1.0.0, v1.2.3-rc1)
- Binary naming: `caldawarrior-v{version}-x86_64-linux` (version baked into filename)
- Standalone binary — no tar.gz archive, single statically-linked executable
- SHA256 checksum file alongside binary: `caldawarrior-v{version}-x86_64-linux.sha256`
- Separate `release.yml` workflow file, triggered only on `v*` tags

### CI job structure
- Single `ci.yml` workflow file with multiple parallel jobs
- Triggered on every push and PR
- Jobs: lint (fmt + clippy), test (unit + integration), e2e (RF), audit (cargo-deny)
- Cache cargo registry and target/ between runs for faster builds

### RF E2E tests in CI
- RF tests are a blocking check — must pass before merge
- Run Docker Compose directly on the GitHub Actions Ubuntu runner (Docker pre-installed)
- Same commands as local Makefile — no service container rewrite needed

### Security audit (cargo-deny)
- Claude's Discretion — configure a sensible default deny.toml covering licenses, advisories, and banned crates

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` — PKG-01 (CI pipeline) and PKG-02 (binary releases)
- `.planning/ROADMAP.md` — Phase 5 success criteria (2 conditions that must be TRUE)

### Existing build infrastructure
- `Makefile` — Existing targets: `test-robot`, `build-robot`, `test-integration`, `test-all`
- `Cargo.toml` — Package config, edition 2024, dependencies list (needed for cargo-deny config)
- `tests/robot/docker-compose.yml` — RF E2E Docker setup with TW + Radicale + UID/GID passthrough

### Prior phase context
- `.planning/phases/01-code-audit-and-bug-fixes/01-CONTEXT.md` — Test philosophy: spec-oriented, E2E mandatory

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `Makefile` targets: `test-robot` (RF E2E), `test-integration` (Rust integration), `test-all` (both) — CI jobs can invoke these directly
- `tests/robot/docker-compose.yml` — Complete Docker Compose setup for RF E2E (Radicale server, TW container, Robot runner)
- `tests/robot/Dockerfile` — RF test runner image build

### Established Patterns
- `CURRENT_UID=$(id -u) CURRENT_GID=$(id -g)` env vars passed to docker compose — CI must replicate this
- `cargo test --test integration` for Rust integration tests (separate binary in `tests/integration/mod.rs`)
- RF results written to `tests/robot/results/` — CI should upload these as artifacts on failure

### Integration Points
- No `.github/` directory exists — entirely greenfield for CI/CD
- No `deny.toml` exists — cargo-deny config must be created from scratch
- Binary built via `cargo build --release` — target at `target/release/caldawarrior`

</code_context>

<specifics>
## Specific Ideas

- RF E2E tests use the same Docker Compose setup locally and in CI — no CI-specific test infrastructure
- cargo-deny explained to user: licenses (prevent accidental GPL deps), advisories (RustSec DB), banned crates. User chose sensible defaults over configuring each policy

</specifics>

<deferred>
## Deferred Ideas

- aarch64-linux and macOS binary releases — v2 (PKG-03 in REQUIREMENTS.md)
- crates.io publishing via cargo install — v2 (PKG-04 in REQUIREMENTS.md)

</deferred>

---

*Phase: 05-ci-cd-and-packaging*
*Context gathered: 2026-03-19*
