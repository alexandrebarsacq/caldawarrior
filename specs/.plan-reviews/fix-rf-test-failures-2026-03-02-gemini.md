# Review Summary

## Critical Blockers
None identified. The plan is exceptionally well-structured, thoroughly researched, and addresses the root causes with precise, logical fixes.

## Major Suggestions
- **[Completeness]** Incomplete File List for Phase 2 Task
  - **Description:** The task "Add cutoff filter inside build_ir" dictates updating `run_sync` and existing `build_ir` call sites, but it only lists `src/ir.rs` under the `File:` property. 
  - **Impact:** An autonomous agent implementing this task might strictly scope its modifications to `src/ir.rs` and fail to update the files where `run_sync` or other call sites live (e.g., `src/sync/mod.rs`, `src/main.rs`, `tests/...`), resulting in compilation failures.
  - **Fix:** Update the `File:` property to be a comma-separated list of all affected files, or split the task into two: one for modifying the `build_ir` internals and another for updating the upstream call sites.

- **[Risk]** Test Flakiness due to Timestamp Resolution
  - **Description:** The risk section notes that if `TW.modified` and `LAST-MODIFIED` are set in the same second, TW will win due to the strict `>` operator, accepting this as "expected by spec." However, if a test relies on `modify_vtodo_summary` to update CalDAV, the *test's explicit intent* is for CalDAV to win.
  - **Impact:** If fast execution in CI causes the updates to happen within the same second, CalDAV will lose the LWW resolution, leading to intermittent blackbox test failures for S-11/S-62.
  - **Fix:** In `modify_vtodo_summary`, explicitly guarantee that CalDAV's timestamp is strictly greater than TW's. You can do this by either introducing a 1-2 second `sleep` before the PUT, or artificially bumping the timestamp slightly into the future (e.g., `datetime.now(tz=timezone.utc) + timedelta(seconds=2)`).

## Minor Suggestions
- **[Architecture]** Helper Keyword Side-Effects
  - **Description:** The plan includes adding a `GET` request and an assertion inside the `modify_vtodo_summary` helper method to verify Radicale's behavior. Putting assertions inside generic test helpers can make them less reusable and clutters the test library with side-effects.
  - **Fix:** Consider removing the assertion from the helper keyword and placing it exclusively inside the specific `.robot` test cases that need to verify this behavior, or create a distinct keyword specifically for verifying the PUT/GET lifecycle.

- **[Clarity]** Handling of Teardown DELETE Failures
  - **Description:** The plan specifies handling 404s gracefully in the `clear_vtodos` teardown, but it does not specify what to do if the `DELETE` requests themselves fail (e.g., due to a 500 error or permission issue).
  - **Fix:** Explicitly state whether `DELETE` errors should be swallowed (for a best-effort teardown) or allowed to raise an exception to surface underlying infrastructure issues.

## Questions
- **[Sequencing]** Docker Rebuild Lifecycle
  - **Context:** Phase 2 implies a Docker rebuild is needed for the Rust changes to take effect in the blackbox tests, and it is mentioned in the Verification sections. 
  - **Needed:** Should the Docker rebuild be formalized as a discrete task at the beginning of Phase 3? This would ensure the agent executing the plan has an explicit instruction to rebuild the environment before attempting to run the final verification suite.

## Praise
- **[Architecture]** Prevention of Orphan Deletion
  - **Why:** Explicitly exempting already-synced tasks (`caldavuid.is_some()`) from the cutoff filter is a highly insightful architectural decision. It prevents the sync engine from interpreting filtered tasks as "deleted in TW," thereby avoiding catastrophic and unintended orphan deletions in CalDAV.
- **[Architecture]** Deterministic Time in Tests
  - **Why:** Passing `now: DateTime<Utc>` into `build_ir` as a parameter—rather than fetching the current time inline—is an excellent dependency injection choice. It ensures the time-based filtering remains 100% deterministic and unit-testable.
- **[Clarity]** Exceptional Traceability
  - **Why:** The objectives section perfectly maps specific test scenario codes (S-03, S-11, etc.) directly to their root causes. This makes the plan's intent unambiguous and ensures the success criteria are easily measurable.