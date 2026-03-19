---
phase: 03-field-and-sync-correctness
plan: 03
subsystem: sync
tags: [taskwarrior, task-modify, tag-diff, annotation-diff, lww-sync]

# Dependency graph
requires:
  - phase: 03-02
    provides: "Field mapping E2E tests and idempotency suite that exposed the task import regression"
provides:
  - "tw.update() using task modify with tag diff (+tag/-tag) and annotation diff (annotate/denotate)"
  - "All 174 unit tests passing (7 new tests for modify-based update)"
  - "All 18 integration tests passing (9 previously-failing tests now green)"
affects: [04-resilience-and-error-handling, 05-ux-and-cli, 06-documentation]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Tag diff via +tag/-tag modify syntax instead of atomic import"
    - "Annotation diff via separate annotate/denotate runner commands"
    - "Optional old_task parameter for diff computation (None for scalar-only updates)"

key-files:
  created: []
  modified:
    - src/tw_adapter.rs
    - src/sync/writeback.rs

key-decisions:
  - "Reverted tw.update() from task import back to task modify -- task import drops caldavuid UDA in Docker, causing perpetual re-sync"
  - "Tags diffed via +tag/-tag modify args rather than atomic replacement -- compatible with TW3 CLI semantics"
  - "Annotations diffed by description text match, using separate annotate/denotate commands -- modify cannot handle annotation arrays"
  - "PushToCalDav caldavuid writeback passes None as old_task -- only caldavuid changed, no tag/annotation diff needed"

patterns-established:
  - "update(task, old_task) pattern: callers must supply old TW task for tag/annotation diff computation"

requirements-completed: [FIELD-01, FIELD-02, FIELD-03, FIELD-04]

# Metrics
duration: 5min
completed: 2026-03-19
---

# Phase 03 Plan 03: Gap Closure Summary

**Reverted tw.update() from task import to task modify with tag diff (+tag/-tag) and annotation diff (annotate/denotate), fixing 9/18 integration test failures**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-19T09:48:15Z
- **Completed:** 2026-03-19T09:53:33Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Reverted tw.update() from task import to task modify, fixing the regression that dropped caldavuid UDA in Docker
- Implemented tag diff logic using +tag/-tag modify syntax for CalDAV-to-TW tag sync
- Implemented annotation diff logic using separate annotate/denotate commands
- All 174 unit tests pass (167 baseline + 7 new tests for modify-based update)
- All 18 integration tests pass (9 previously-failing tests now green)
- All 4 FIELD requirements (FIELD-01 through FIELD-04) fully satisfied

## Task Commits

Each task was committed atomically:

1. **Task 1: Revert tw.update() to task modify with tag and annotation diff support** - `2d956a4` (feat)
2. **Task 2: Verify integration tests pass** - verification only, no code changes

## Files Created/Modified
- `src/tw_adapter.rs` - Replaced import-based update() with modify-based implementation including tag diff and annotation diff
- `src/sync/writeback.rs` - Updated all 3 tw.update() call sites to pass old_task parameter; fixed 4 mock expectations from push_import_response to push_run_response

## Decisions Made
- Reverted tw.update() from task import back to task modify because task import drops caldavuid UDA in Docker TW3, causing perpetual re-sync (9/18 integration test failures)
- Tags are diffed between old and new tasks using +tag/-tag modify args (compatible with TW3 CLI)
- Annotations are diffed by description text match using separate annotate/denotate runner commands
- PushToCalDav caldavuid writeback passes None as old_task since only caldavuid changes (no tag/annotation diff needed)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 03 (Field and Sync Correctness) is fully complete
- All field mapping, sync logic, and idempotency requirements verified at unit, integration, and E2E levels
- Ready for Phase 04 (Resilience and Error Handling)

## Self-Check: PASSED

- FOUND: src/tw_adapter.rs
- FOUND: src/sync/writeback.rs
- FOUND: 03-03-SUMMARY.md
- FOUND: commit 2d956a4

---
*Phase: 03-field-and-sync-correctness*
*Completed: 2026-03-19*
