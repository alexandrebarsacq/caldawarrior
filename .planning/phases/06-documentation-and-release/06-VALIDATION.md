---
phase: 6
slug: documentation-and-release
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-19
---

# Phase 6 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust unit/integration) + Robot Framework (E2E) |
| **Config file** | `Cargo.toml` (Rust tests), `tests/robot/docker-compose.yml` (RF) |
| **Quick run command** | `cargo build --release` |
| **Full suite command** | `cargo test --lib && cargo test --test integration && cargo build --release` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo build --release`
- **After every plan wave:** Run `cargo test --lib && cargo test --test integration`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 06-01-01 | 01 | 1 | DOC-01 | manual-only | Visual review of README.md sections | N/A | ⬜ pending |
| 06-01-02 | 01 | 1 | DOC-02 | manual-only | Cross-reference `src/config.rs` fields against README table | N/A | ⬜ pending |
| 06-01-03 | 01 | 1 | DOC-03 | manual-only | Visual review of CHANGELOG.md structure | N/A | ⬜ pending |
| 06-01-04 | 01 | 1 | DOC-04 | manual-only | Visual review of compatibility matrix | N/A | ⬜ pending |
| 06-01-05 | 01 | 1 | N/A | unit | `cargo build --release` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements. This phase produces documentation files and a version field edit — no new test infrastructure needed.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| README covers installation, config, usage, scheduling | DOC-01 | Documentation deliverable — no runtime behavior | Verify README.md contains: `## Installation`, `## Configuration`, `## Usage`/`## First Sync`, `## Scheduling` sections |
| All config options documented with types/defaults | DOC-02 | Documentation completeness — requires cross-referencing code | Compare `src/config.rs` struct fields against README config table; every field must appear with type, default, example |
| CHANGELOG in Keep a Changelog format | DOC-03 | Format compliance — structural review | Verify CHANGELOG.md has `## [1.0.0]` header, uses standard categories (Added/Changed/Fixed), entries are curated |
| Compatibility matrix with tested/expected tiers | DOC-04 | Documentation completeness | Verify README contains compatibility table with Tested/Expected/Unknown tiers and DEPENDS-ON limitation note |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
