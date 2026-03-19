---
phase: 5
slug: ci-cd-and-packaging
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-19
---

# Phase 5 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) + Robot Framework 7.0.1 (Docker) |
| **Config file** | Cargo.toml (test config) + tests/robot/docker-compose.yml (E2E) |
| **Quick run command** | `cargo fmt --check && cargo clippy -- -D warnings && cargo test --lib` |
| **Full suite command** | `cargo test && cargo deny check && make test-robot` |
| **Estimated runtime** | ~120 seconds (excluding first Docker build) |

---

## Sampling Rate

- **After every task commit:** Run `cargo fmt --check && cargo clippy -- -D warnings && cargo test --lib`
- **After every plan wave:** Run `cargo test && cargo deny check`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 120 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 05-01-01 | 01 | 1 | PKG-01a | unit | `cargo fmt --check` | N/A (built-in) | ⬜ pending |
| 05-01-02 | 01 | 1 | PKG-01b | unit | `cargo clippy -- -D warnings` | N/A (built-in) | ⬜ pending |
| 05-01-03 | 01 | 1 | PKG-02a | smoke | `cargo build --release --target x86_64-unknown-linux-musl` | N/A (build test) | ⬜ pending |
| 05-02-01 | 02 | 1 | PKG-01 | manual-only | Push to GitHub, verify Actions pass | .github/workflows/ci.yml | ⬜ pending |
| 05-02-02 | 02 | 1 | PKG-01f | smoke | `cargo deny check` | deny.toml | ⬜ pending |
| 05-03-01 | 03 | 1 | PKG-02 | manual-only | Push v* tag, verify GitHub Release | .github/workflows/release.yml | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Fix 157 rustfmt violations — `cargo fmt` auto-fix (PKG-01a)
- [ ] Fix/allow 47 clippy warnings (PKG-01b)
- [ ] Update `Cargo.toml` reqwest to `default-features = false` for MUSL build (PKG-02a)
- [ ] `deny.toml` — cargo-deny configuration (PKG-01f)
- [ ] `.github/workflows/ci.yml` — CI workflow file (PKG-01)
- [ ] `.github/workflows/release.yml` — release workflow file (PKG-02)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| CI pipeline runs all checks on push/PR | PKG-01 | Workflow files are tested by running them on GitHub | Push workflow files to GitHub, trigger CI run, verify all 4 jobs pass green |
| Release binary published on tag push | PKG-02 | Release workflow requires a real v* tag push | Push a v* tag and verify GitHub Release contains binary + checksum |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
