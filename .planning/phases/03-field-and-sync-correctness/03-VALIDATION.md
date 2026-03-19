---
phase: 3
slug: field-and-sync-correctness
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-19
---

# Phase 3 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Robot Framework (Docker) + cargo test |
| **Config file** | `tests/robot/resources/common.robot` (RF), `Cargo.toml` (Rust) |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `cargo test && CURRENT_UID=$(id -u) CURRENT_GID=$(id -g) docker compose -f tests/robot/docker-compose.yml run --rm robot` |
| **Estimated runtime** | ~120 seconds (cargo test ~10s + RF suite ~110s) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib`
- **After every plan wave:** Run `cargo test && CURRENT_UID=$(id -u) CURRENT_GID=$(id -g) docker compose -f tests/robot/docker-compose.yml run --rm robot`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 120 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 03-01-01 | 01 | 1 | FIELD-01 | E2E (RF) | `docker compose ... --include field-mapping` | Partial (07) | ⬜ pending |
| 03-01-02 | 01 | 1 | FIELD-02 | E2E (RF) | `docker compose ... --include status-mapping` | Partial (04) | ⬜ pending |
| 03-01-03 | 01 | 1 | FIELD-03 | E2E (RF) + unit | `cargo test cancelled && docker compose ... --include deletion` | Partial | ⬜ pending |
| 03-01-04 | 01 | 1 | FIELD-04 | E2E (RF) | `docker compose ... --include idempotency` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/robot/suites/08_idempotency.robot` — stubs for FIELD-04
- [ ] Fix `CalDAVLibrary.py modify_vtodo_status` — add LAST-MODIFIED bump and COMPLETED clearing
- [ ] New CalDAV keyword: `Modify VTODO Field` — generic field update for E2E tests
- [ ] New CalDAV keyword: `VTODO Should Not Have Property` — field clear assertions
- [ ] Update `tests/robot/docs/CATALOG.md` with new scenario IDs

*Existing infrastructure covers unit tests and basic RF patterns. Wave 0 fills E2E gaps.*

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
