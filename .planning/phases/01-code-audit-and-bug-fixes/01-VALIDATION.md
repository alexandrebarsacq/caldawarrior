---
phase: 1
slug: code-audit-and-bug-fixes
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-18
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml |
| **Quick run command** | `cargo test` |
| **Full suite command** | `cargo test && CURRENT_UID=$(id -u) CURRENT_GID=$(id -g) docker compose -f tests/robot/docker-compose.yml run --rm robot` |
| **Estimated runtime** | ~30 seconds (unit), ~120 seconds (full with RF) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test`
- **After every plan wave:** Run `cargo test && CURRENT_UID=$(id -u) CURRENT_GID=$(id -g) docker compose -f tests/robot/docker-compose.yml run --rm robot`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 01-01-01 | 01 | 1 | AUDIT-01 | unit | `cargo test categories` | ❌ W0 | ⬜ pending |
| 01-02-01 | 02 | 1 | AUDIT-02 | unit | `cargo test xml` | ❌ W0 | ⬜ pending |
| 01-03-01 | 03 | 1 | AUDIT-03 | unit | `cargo test error` | ❌ W0 | ⬜ pending |
| 01-04-01 | 04 | 1 | AUDIT-04 | unit | `cargo test etag` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Test stubs for AUDIT-01 (comma-in-tags round-trip)
- [ ] Test stubs for AUDIT-02 (namespace-agnostic XML parsing)
- [ ] Test stubs for AUDIT-03 (error context preservation)
- [ ] Test stubs for AUDIT-04 (weak ETag normalization)

*Existing cargo test infrastructure covers framework needs. Stubs needed for new bug-fix scenarios.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Full sync round-trip with Radicale | AUDIT-01 | Requires live CalDAV server | RF blackbox test suite covers this |
| XML parsing with Nextcloud responses | AUDIT-02 | Requires real Nextcloud REPORT response | RF test with mock server fixture |

*Robot Framework E2E tests cover both manual scenarios above.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
