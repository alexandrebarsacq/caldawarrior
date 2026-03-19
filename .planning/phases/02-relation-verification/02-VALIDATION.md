---
phase: 2
slug: relation-verification
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-19
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust cargo test (unit/integration) + Robot Framework (E2E) |
| **Config file** | `Cargo.toml` (Rust), `tests/robot/` (RF) |
| **Quick run command** | `cargo test --lib -q` |
| **Full suite command** | `cargo test && CURRENT_UID=$(id -u) CURRENT_GID=$(id -g) docker compose -f tests/robot/docker-compose.yml run --rm robot` |
| **Estimated runtime** | ~60 seconds (cargo test ~5s + RF E2E ~55s) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib -q`
- **After every plan wave:** Run `cargo test && CURRENT_UID=$(id -u) CURRENT_GID=$(id -g) docker compose -f tests/robot/docker-compose.yml run --rm robot`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 60 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 02-01-01 | 01 | 1 | REL-02 | unit | `cargo test cyclic` | Yes (needs update) | ⬜ pending |
| 02-01-02 | 01 | 1 | REL-02 | E2E | RF: `05_dependencies.robot::Cyclic Dependency` (S-42) | Yes (needs assertion update) | ⬜ pending |
| 02-01-03 | 01 | 1 | REL-02 | E2E | RF: `05_dependencies.robot::3-Node Cyclic` | No — Wave 0 | ⬜ pending |
| 02-02-01 | 02 | 1 | REL-01 | E2E | RF: `05_dependencies.robot::TW Depends Syncs To CalDAV` (S-40) | Yes | ⬜ pending |
| 02-02-02 | 02 | 1 | REL-01 | E2E | RF: `05_dependencies.robot::CalDAV Related-To Syncs To TW` (S-41) | Yes | ⬜ pending |
| 02-02-03 | 02 | 1 | REL-04 | E2E | RF: `05_dependencies.robot::Blocks Verification` | No — Wave 0 | ⬜ pending |
| 02-02-04 | 02 | 1 | REL-03 | manual-only | Review `docs/compatibility/tasks-org.md` | No — new file | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Update `cyclic_entry_skipped` unit test in `writeback.rs` — must verify cyclic entries ARE written (not skipped)
- [ ] New unit test: cyclic entry produces VTODO without RELATED-TO
- [ ] New RF test: 3-node cycle in `05_dependencies.robot`
- [ ] New RF test: blocks verification in `05_dependencies.robot`
- [ ] Update S-42 assertions in `05_dependencies.robot` — remove `skip-unimplemented` tag
- [ ] New RF keyword in `TaskWarriorLibrary.py`: `TW Task Should Have Blocks` or similar for checking computed `blocks` field
- [ ] Create `docs/compatibility/` directory and `tasks-org.md`

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| tasks.org/DAVx5 compatibility doc | REL-03 | Research-based documentation, not runtime verification | Review `docs/compatibility/tasks-org.md` for evidence and citations |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
