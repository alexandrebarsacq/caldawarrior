---
phase: 7
slug: readme-accuracy-fixes
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-19
---

# Phase 7 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Manual review (documentation-only phase) |
| **Config file** | N/A |
| **Quick run command** | `grep -n "annotations\|DESCRIPTION" README.md` |
| **Full suite command** | Visual inspection of all four edit locations |
| **Estimated runtime** | ~5 seconds |

---

## Sampling Rate

- **After every task commit:** Run `grep -n "annotations\|DESCRIPTION" README.md`
- **After every plan wave:** Visual inspection of all four edit locations
- **Before `/gsd:verify-work`:** All four grep checks pass
- **Max feedback latency:** 5 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 07-01-01 | 01 | 1 | DOC-01 (Edit 1) | manual-only | `sed -n '390,396p' README.md` | N/A | ⬜ pending |
| 07-01-02 | 01 | 1 | DOC-01 (Edit 2) | manual-only | `grep 'annotations.*DESCRIPTION' README.md` | N/A | ⬜ pending |
| 07-01-03 | 01 | 1 | DOC-01 (Edit 3) | manual-only | `grep -c 'Annotation.*DESCRIPTION sync' README.md` (expect 0) | N/A | ⬜ pending |
| 07-01-04 | 01 | 1 | DOC-01 (Edit 4) | manual-only | `grep 'tasks-org.md' README.md` | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements. No test infrastructure needed for documentation-only edits.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Limitation 12 reflects annotation sync reality | DOC-01 | Documentation accuracy is textual/visual | Read Limitation 12, confirm it states annotation ↔ DESCRIPTION sync is implemented (first annotation only) |
| Field Mapping table includes annotations row | DOC-01 | Documentation accuracy is textual/visual | Verify `annotations → DESCRIPTION` row exists in Field Mapping table |
| v2 Roadmap does not list annotation sync | DOC-01 | Documentation accuracy is textual/visual | Confirm "Annotation / DESCRIPTION sync" is absent from v2 Roadmap |
| Compatibility links to tasks-org.md | DOC-01 | Documentation accuracy is textual/visual | Confirm `docs/compatibility/tasks-org.md` link exists in Compatibility section |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 5s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
