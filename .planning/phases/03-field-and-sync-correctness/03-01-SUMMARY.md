---
phase: 03-field-and-sync-correctness
plan: 01
subsystem: sync
tags: [writeback, status-mapping, deletion, cancelled, reopen, lww, robot-framework, e2e]

# Dependency graph
requires:
  - phase: 01-core-parsing-and-field-bugs
    provides: "XML parser, field mapping, ETag normalization"
  - phase: 02-relation-verification
    provides: "Dependency sync, cyclic handling"
provides:
  - "Fixed CANCELLED propagation (CalDAV CANCELLED -> TW deletion)"
  - "Fixed completed status reopen via LWW timestamp checks"
  - "CalDAVLibrary.py keywords: modify_vtodo_field, remove_vtodo_property, put_vtodo_with_fields, vtodo_should_not_have_property"
  - "8 new E2E scenarios (S-23, S-24, S-25, S-34, S-35, S-36, S-37, S-38)"
affects: [03-field-and-sync-correctness, 04-multi-calendar-and-edge-cases]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "LWW timestamp guard before status short-circuit in decide_op"
    - "CalDAVLibrary LAST-MODIFIED +2s bump pattern for all status/field mutations"

key-files:
  created: []
  modified:
    - src/sync/writeback.rs
    - tests/robot/resources/CalDAVLibrary.py
    - tests/robot/suites/04_status_mapping.robot
    - tests/robot/suites/03_orphan.robot
    - tests/robot/docs/CATALOG.md

key-decisions:
  - "Completed status special-casing now checks LWW timestamps before short-circuiting -- enables reopen propagation"
  - "SkipReason::Cancelled retained for CalDAV-only branch (no TW pair) -- still needed by orphan logic"

patterns-established:
  - "LWW timestamp guard: all status-transition blocks in decide_op check whether the 'other side' is newer before short-circuiting"
  - "CalDAVLibrary LAST-MODIFIED bump: +2 seconds on all property mutations to ensure CalDAV wins LWW"

requirements-completed: [FIELD-02, FIELD-03]

# Metrics
duration: 9min
completed: 2026-03-19
---

# Phase 3 Plan 1: Status Transitions and Deletion Propagation Summary

**Fixed CANCELLED-to-TW-deletion asymmetry, completed-reopen LWW bypass, and added 8 E2E tests covering all bidirectional status transitions and deletion paths**

## Performance

- **Duration:** 9 min
- **Started:** 2026-03-19T07:34:09Z
- **Completed:** 2026-03-19T07:43:09Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments
- Fixed CalDAV CANCELLED propagation: paired active TW tasks now get deleted instead of skipped
- Fixed completed status short-circuit to respect LWW timestamps, enabling reopen propagation in both directions
- Added CalDAVLibrary.py keywords (modify_vtodo_field, remove_vtodo_property, put_vtodo_with_fields, vtodo_should_not_have_property) for Phase 3 Plan 2
- 8 new E2E scenarios pass: S-23, S-24, S-25, S-34, S-35, S-36, S-37, S-38
- All 167 unit tests pass, 47/48 E2E tests pass (1 pre-existing failure in field-mapping)

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix CalDAVLibrary.py modify_vtodo_status and add assertion keywords** - `cb70e29` (feat)
2. **Task 2: Fix CANCELLED propagation bug in writeback.rs** - `57a926c` (test: RED), `397af45` (fix: GREEN)
3. **Task 3: E2E tests for status transitions and deletion propagation** - `ee33a5d` (feat)

_Note: Task 2 followed TDD with separate RED/GREEN commits_

## Files Created/Modified
- `src/sync/writeback.rs` - Fixed CANCELLED propagation, added LWW timestamp guards for completed reopen, 4 new unit tests
- `tests/robot/resources/CalDAVLibrary.py` - Fixed modify_vtodo_status (LAST-MODIFIED bump, COMPLETED clearing), added 5 new keywords
- `tests/robot/suites/04_status_mapping.robot` - Added S-34 through S-38 (reopen, delete, cancelled, both-terminal)
- `tests/robot/suites/03_orphan.robot` - Added S-23, S-24, S-25 (ghost prevention, both-terminal-completed)
- `tests/robot/docs/CATALOG.md` - Updated ranges, added 8 new scenario entries

## Decisions Made
- Completed status special-casing in decide_op now checks LWW timestamps before short-circuiting. This ensures that if the "other side" has a newer timestamp (e.g., CalDAV was reopened from COMPLETED to NEEDS-ACTION after TW completed), LWW determines the winner instead of always favoring the completed state.
- SkipReason::Cancelled variant retained in src/types.rs -- still used by the CalDAV-only branch in writeback.rs for orphan CANCELLED VTODOs with no TW pair.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Completed status short-circuit bypassed LWW for reopened tasks**
- **Found during:** Task 3 (E2E tests S-34, S-35 failing)
- **Issue:** When TW completed + CalDAV NEEDS-ACTION (or vice versa), the status special-casing in decide_op always favored the completed side, ignoring that the other side may have been modified more recently (reopen scenario).
- **Fix:** Added LWW timestamp checks in both completed status blocks: if the non-completed side is newer, fall through to resolve_lww() instead of short-circuiting.
- **Files modified:** src/sync/writeback.rs (decide_op function, lines 262-291)
- **Verification:** 2 new unit tests (caldav_reopen_completed_falls_through_to_lww, tw_reopen_completed_falls_through_to_lww) + E2E S-34, S-35 pass
- **Committed in:** ee33a5d (Task 3 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Essential for correctness of reopen bidirectional propagation. No scope creep -- the plan's must_haves explicitly required "Completed task reopened on either side propagates back to pending/NEEDS-ACTION".

## Issues Encountered
- Pre-existing failure in `TW Tags Sync To CalDAV CATEGORIES` (AUDIT-01 in 07_field_mapping.robot) -- confirmed pre-existing by testing against the baseline. Not caused by our changes, not in scope for this plan.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- CalDAVLibrary.py now has all keywords needed for field clear/modify tests in Plan 2
- All status transition and deletion paths verified bidirectionally
- CANCELLED propagation fix unblocks any future tests that depend on CalDAV CANCELLED behavior

## Self-Check: PASSED

All 5 created/modified files verified present. All 4 commits (cb70e29, 57a926c, 397af45, ee33a5d) verified in git log.

---
*Phase: 03-field-and-sync-correctness*
*Completed: 2026-03-19*
