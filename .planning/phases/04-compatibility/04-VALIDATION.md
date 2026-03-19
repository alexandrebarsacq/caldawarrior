---
phase: 4
slug: compatibility
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-19
---

# Phase 4 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust unit + integration) + Robot Framework (E2E) |
| **Config file** | `Cargo.toml` (test harness) + `tests/robot/docker-compose.yml` (RF) |
| **Quick run command** | `cargo test` |
| **Full suite command** | `cargo test && CURRENT_UID=$(id -u) CURRENT_GID=$(id -g) docker compose -f tests/robot/docker-compose.yml run --rm robot` |
| **Estimated runtime** | ~60 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test`
- **After every plan wave:** Run `cargo test && CURRENT_UID=$(id -u) CURRENT_GID=$(id -g) docker compose -f tests/robot/docker-compose.yml run --rm robot`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 60 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 04-01-01 | 01 | 1 | COMPAT-01 | unit | `cargo test caldav_adapter::tests::test_parse_multistatus_large -x` | ❌ W0 | ⬜ pending |
| 04-01-02 | 01 | 1 | COMPAT-01 | unit | `cargo test caldav_adapter::tests::test_parse_multistatus_special_chars -x` | ❌ W0 | ⬜ pending |
| 04-01-03 | 01 | 1 | COMPAT-01 | unit | `cargo test caldav_adapter::tests::test_parse_multistatus_empty -x` | ❌ W0 | ⬜ pending |
| 04-02-01 | 02 | 1 | COMPAT-02 | unit | `cargo test ical::tests::test_date_only_due_parsed -x` | ❌ W0 | ⬜ pending |
| 04-02-02 | 02 | 1 | COMPAT-02 | unit | `cargo test ical::tests::test_date_only_due_serialized -x` | ❌ W0 | ⬜ pending |
| 04-02-03 | 02 | 1 | COMPAT-02 | E2E | RF suite 09_compatibility.robot | ❌ W0 | ⬜ pending |
| 04-02-04 | 02 | 1 | COMPAT-02 | unit | `cargo test ical::tests::test_tw_originated_due_datetime -x` | ❌ W0 | ⬜ pending |
| 04-03-01 | 03 | 1 | COMPAT-03 | unit | `cargo test ical::tests::test_tzid_conversion` | ✅ | ⬜ pending |
| 04-03-02 | 03 | 1 | COMPAT-03 | unit | `cargo test ical::tests::test_tzid_spring_forward_gap -x` | ❌ W0 | ⬜ pending |
| 04-03-03 | 03 | 1 | COMPAT-03 | unit | `cargo test ical::tests::test_tzid_fall_back_ambiguous -x` | ❌ W0 | ⬜ pending |
| 04-03-04 | 03 | 1 | COMPAT-03 | unit | `cargo test ical::tests::test_tzid_paris_summer -x` | ❌ W0 | ⬜ pending |
| 04-03-05 | 03 | 1 | COMPAT-03 | unit | `cargo test ical::tests::test_tzid_paris_winter -x` | ❌ W0 | ⬜ pending |
| 04-04-01 | 04 | 1 | COMPAT-04 | E2E | RF suite 09_compatibility.robot | ❌ W0 | ⬜ pending |
| 04-04-02 | 04 | 1 | COMPAT-04 | E2E | RF suite 09_compatibility.robot | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/robot/suites/09_compatibility.robot` — DATE-only E2E, X-property E2E, edge-case tests
- [ ] New CalDAVLibrary.py keyword `put_vtodo_raw_ical` — enables custom iCal content in E2E tests
- [ ] Unit tests for COMPAT-01 edge cases (large, special chars, empty) in `src/caldav_adapter.rs`
- [ ] Unit tests for COMPAT-02 DATE-only parsing/serialization in `src/ical.rs`
- [ ] Unit tests for COMPAT-03 DST edge cases (spring-forward, fall-back, Paris) in `src/ical.rs`

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
