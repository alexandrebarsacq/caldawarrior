---
phase: 03-field-and-sync-correctness
plan: 02
subsystem: sync
tags: [field-mapping, idempotency, e2e, robot-framework, lww, content-identical, categories, priority, annotations]

# Dependency graph
requires:
  - phase: 03-field-and-sync-correctness
    plan: 01
    provides: "CalDAVLibrary keywords (modify_vtodo_field, remove_vtodo_property, put_vtodo_with_fields), status transition fixes"
  - phase: 01-core-parsing-and-field-bugs
    provides: "XML parser, field mapping, ETag normalization"
provides:
  - "21 new field mapping E2E tests (S-69 to S-89) covering all mapped fields"
  - "6 new idempotency E2E tests (S-90 to S-95) covering create/update/complete/delete"
  - "content_identical now checks PRIORITY and CATEGORIES (10-field coverage)"
  - "CalDAV CATEGORIES mapped to TW tags on CalDAV-to-TW sync"
  - "tw.update() uses task import for full-field upsert (tags + annotations)"
  - "TaskWarriorLibrary.modify_tw_task supports +tag/-tag positional args"
affects: [04-multi-calendar-and-edge-cases]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "content_identical 10-field check: SUMMARY, DESCRIPTION, STATUS, DUE, DTSTART, COMPLETED, RELATED-TO, WAIT, PRIORITY, CATEGORIES"
    - "tw.update() via task import upsert for atomic field updates including tags and annotations"

key-files:
  created:
    - tests/robot/suites/08_idempotency.robot
  modified:
    - tests/robot/suites/07_field_mapping.robot
    - tests/robot/resources/TaskWarriorLibrary.py
    - tests/robot/docs/CATALOG.md
    - src/sync/lww.rs
    - src/sync/writeback.rs
    - src/tw_adapter.rs

key-decisions:
  - "content_identical expanded from 8 to 10 fields: PRIORITY and CATEGORIES added to prevent LWW short-circuit when only those fields change"
  - "tw.update() switched from task modify to task import for full-field upsert -- enables tags and annotations propagation from CalDAV to TW"
  - "CalDAV CATEGORIES mapped to TW tags in build_tw_task_from_caldav -- previously only TW-to-CalDAV direction worked"

patterns-established:
  - "10-field content_identical check covers all bidirectionally synced fields"
  - "Task import upsert pattern for TW updates ensures all fields including tags and annotations are atomically set"

requirements-completed: [FIELD-01, FIELD-04]

# Metrics
duration: 16min
completed: 2026-03-19
---

# Phase 3 Plan 2: Field Mapping E2E Tests and Idempotency Suite Summary

**27 new E2E tests covering all 10 mapped field lifecycles (create/update/clear) plus dedicated idempotency suite proving sync-twice-zero-writes invariant**

## Performance

- **Duration:** 16 min
- **Started:** 2026-03-19T07:46:25Z
- **Completed:** 2026-03-19T08:02:31Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- 21 new field mapping E2E tests (S-69 to S-89) covering SUMMARY, DUE, DTSTART, PRIORITY, CATEGORIES, DESCRIPTION, X-TASKWARRIOR-WAIT, and COMPLETED timestamp fields
- 6 new idempotency tests (S-90 to S-95) proving sync-twice-zero-writes for all operation types
- Fixed content_identical to check PRIORITY and CATEGORIES (was silently skipping LWW for those fields)
- Fixed CalDAV CATEGORIES to TW tags mapping on CalDAV-to-TW sync path
- Switched tw.update() to task import for full-field upsert (enables tags and annotations sync)
- Fixed TaskWarriorLibrary.modify_tw_task to support +tag/-tag positional arguments
- Full RF suite: 75 tests, 70 passed, 0 failed, 5 skipped (4 skip-unimplemented + 1 multi-calendar)

## Task Commits

Each task was committed atomically:

1. **Task 1: E2E tests for field create/update/clear operations** - `23f04d8` (feat)
2. **Task 2: Create dedicated idempotency test suite and update CATALOG.md** - `13c0e11` (feat)

## Files Created/Modified
- `tests/robot/suites/07_field_mapping.robot` - 21 new test cases (S-69 to S-89) for all mapped field operations
- `tests/robot/suites/08_idempotency.robot` - New suite with 6 tests (S-90 to S-95) for sync idempotency
- `tests/robot/resources/TaskWarriorLibrary.py` - modify_tw_task now accepts *args for +tag/-tag syntax
- `tests/robot/docs/CATALOG.md` - 27 new scenario entries, updated ranges, replaced Bulk Ops/Multi-Sync sections
- `src/sync/lww.rs` - content_identical expanded from 8 to 10 fields (added PRIORITY and CATEGORIES)
- `src/sync/writeback.rs` - build_tw_task_from_caldav maps CalDAV categories to TW tags; updated mock expectations
- `src/tw_adapter.rs` - update() now uses task import for full-field upsert instead of task modify

## Decisions Made
- **content_identical expanded to 10 fields**: PRIORITY (H/M/L to 1/5/9 mapping) and CATEGORIES (sorted comparison) added. Without this, changes to only priority or tags were invisible to the sync engine because content_identical returned true and LWW never fired.
- **tw.update() switched to task import**: task modify cannot set tags (+tag/-tag syntax) or annotations atomically. Task import with an existing UUID performs an upsert, setting all fields including tags and annotations in one operation.
- **CalDAV CATEGORIES mapped to TW tags on creation**: build_tw_task_from_caldav previously set tags to None for CalDAV-only entries. Now uses vtodo.categories when non-empty.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] content_identical didn't check PRIORITY and CATEGORIES**
- **Found during:** Task 1 (S-79, S-80, S-82, S-83 failing)
- **Issue:** content_identical only checked 8 fields. When only PRIORITY or CATEGORIES changed, it returned true, causing LWW to short-circuit as "identical" and skip the update.
- **Fix:** Added checks 9 (PRIORITY: TW H/M/L mapped to iCal 1/5/9) and 10 (CATEGORIES: sorted comparison against TW tags) to content_identical.
- **Files modified:** src/sync/lww.rs
- **Verification:** S-79, S-80, S-82, S-83 now pass; all 167 unit tests pass
- **Committed in:** 23f04d8 (Task 1 commit)

**2. [Rule 2 - Missing Critical] CalDAV CATEGORIES not mapped to TW tags**
- **Found during:** Task 1 (S-81 failing -- empty tags after CalDAV CATEGORIES sync)
- **Issue:** build_tw_task_from_caldav set tags to base.and_then(|t| t.tags.clone()) which is None for CalDAV-only entries. CalDAV CATEGORIES were silently discarded.
- **Fix:** Use vtodo.categories when non-empty, fall back to existing TW tags otherwise.
- **Files modified:** src/sync/writeback.rs
- **Verification:** S-81 now passes
- **Committed in:** 23f04d8 (Task 1 commit)

**3. [Rule 1 - Bug] tw.update() couldn't set tags or annotations**
- **Found during:** Task 1 (S-82 tag update and S-85 annotation update failing)
- **Issue:** tw.update() used task modify which cannot set tags (+tag syntax) or annotations. Only description, status, due, scheduled, priority, project, caldavuid, and depends were written.
- **Fix:** Switched tw.update() to use task import (upsert by UUID), matching the tw.create() pattern. Updated unit test expectations from push_run_response to push_import_response.
- **Files modified:** src/tw_adapter.rs, src/sync/writeback.rs (test mock expectations)
- **Verification:** S-82, S-85 now pass; all 167 unit tests pass
- **Committed in:** 23f04d8 (Task 1 commit)

**4. [Rule 3 - Blocking] TaskWarriorLibrary.modify_tw_task couldn't handle +tag syntax**
- **Found during:** Task 1 (AUDIT-01 test previously failing)
- **Issue:** modify_tw_task only accepted **kwargs (key=value), but Robot Framework passes +tag as positional arg which doesn't match keyword argument syntax.
- **Fix:** Added *args parameter to accept raw modification tokens (positional args like +tag, -tag).
- **Files modified:** tests/robot/resources/TaskWarriorLibrary.py
- **Verification:** AUDIT-01 test now passes (was previously failing)
- **Committed in:** 23f04d8 (Task 1 commit)

---

**Total deviations:** 4 auto-fixed (2 bugs, 1 missing critical, 1 blocking)
**Impact on plan:** All auto-fixes were essential for correctness of bidirectional field sync. The content_identical gap and tw.update() limitation were pre-existing bugs exposed by the new E2E tests. No scope creep.

## Issues Encountered
None -- after fixing the 4 deviations above, all tests passed on first re-run.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All 10 mapped fields now have complete E2E coverage for create, update, and clear operations
- Idempotency proven for all operation types (FIELD-04 satisfied)
- Phase 3 is now complete -- both plans executed successfully
- Ready for Phase 4 (multi-calendar and edge cases)

## Self-Check: PASSED

All 7 created/modified files verified present. Both commits (23f04d8, 13c0e11) verified in git log.

---
*Phase: 03-field-and-sync-correctness*
*Completed: 2026-03-19*
