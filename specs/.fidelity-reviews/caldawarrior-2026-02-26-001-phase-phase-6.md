# Fidelity Review: caldawarrior

**Spec ID:** caldawarrior-2026-02-26-001
**Scope:** phase (phase: phase-6)
**Verdict:** partial
**Date:** 2026-02-28T14:50:55.097617

## Summary

Phase 5 is substantially complete — all four spec files are present and cover the required scenarios, the harness correctly reuses the Phase 0 docker-compose.yml, and Docker-based isolation works cleanly. However, two tests in test_scenarios.rs have incomplete assertions that fail to fully verify stated acceptance criteria: (1) orphaned_caldavuid_causes_tw_deletion asserts only that CalDAV count stays at zero, but never checks that the TW task was actually deleted — directly missing half of its AC; (2) status_sync_caldav_completed_to_tw uses a disjunctive guard (status==completed OR written_tw>=1) that allows the test to pass without confirming the TW task reached 'completed' status. These gaps mean the tests can give false-positive results even when the sync engine misbehaves. All other requirements are well-implemented with strong assertions.

## Requirement Alignment
**Status:** partial

task-6-1 (mod.rs): harness fully aligned — reuses Phase 0 docker-compose.yml via COMPOSE_DIR constant, MKCOL with CalDAV XML body for proper collection creation, UUID-isolated calendars, DockerizedTaskRunner for TW isolation, reset() wipes both TW .data files and CalDAV VTODOs, Drop cleans up calendar. No container teardown after suite (low severity; accepted pattern). task-6-2 (test_first_sync.rs): all three ACs covered by four focused tests — push, caldavuid, dry-run, project mapping. task-6-3 (test_lww.rs): all three ACs covered — tw_wins_lww, caldav_wins_lww, loop_prevention_stable_point (four-step scenario), etag_conflict_scenario. task-6-4 (test_scenarios.rs): status sync and dependency sync (both directions) and large-dataset (100 tasks, stable-point) are well covered, but orphaned caldavuid test does not assert TW deletion and the status sync assertion is a relaxed disjunction.

## Success Criteria
**Status:** partial

AC met: Rust harness starts Radicale via Phase 0 docker-compose (not recreated). AC met: harness creates/deletes CalDAV calendars and TW tasks programmatically. AC met: state fully reset between test cases. AC met: TW tasks appear as VTODOs in CalDAV after first sync. AC met: caldavuid UDA set on TW tasks after sync. AC met: project→calendar URL mapping applied correctly. AC met: LWW TW-wins and CalDAV-wins both verified. AC met: second sync after CalDAV-wins produces written_caldav==0 && written_tw==0. AC met: ETag conflict scenario tested. AC met: TW depends synced to CalDAV RELATED-TO and back. AC met: 100-task first sync produces written_caldav==100 and stable second sync. AC NOT FULLY MET: 'Orphaned caldavuid causes TW task deletion (not CalDAV re-create)' — test verifies CalDAV count==0 but never calls get_tw_task() to assert the TW task was deleted or checks r2.written_tw. AC WEAKLY MET: 'CalDAV COMPLETED syncs back to TW as completed task' — assertion is `status == 'completed' || r2.written_tw >= 1`, which passes on any TW write regardless of resulting status.

## Deviations

- **[HIGH]** orphaned_caldavuid_causes_tw_deletion does not assert TW task deletion. The test name and spec AC both require verifying that the TW task is deleted, but the only assertion is `count_caldav_vtodos() == 0` (the 'not re-created in CalDAV' half). get_tw_task() is never called to confirm the task was removed, and r2.written_tw is not checked.
  - Justification: No justification documented. Likely an oversight; the test was designed to cover the CalDAV side but forgot the TW side.
- **[MEDIUM]** status_sync_caldav_completed_to_tw uses a relaxed disjunctive assertion: `status == 'completed' || r2.written_tw >= 1`. This means the test passes when a TW write occurs even if the resulting status is not 'completed' (e.g., 'pending' with updated description).
  - Justification: Likely added because completed tasks may not be returned by the standard TW export filter. However, the workaround weakens the AC verification rather than solving the root cause (e.g., using `export rc.status:completed` filter or checking the completed.data file).
- **[LOW]** Radicale container is never stopped after the test suite. The harness starts Radicale once via OnceLock but provides no post-suite teardown for the Docker container.
  - Justification: Accepted pattern for integration test suites; stopping per-test would be too expensive. Container cleanup is the operator's responsibility. Radicale data persists in a named Docker volume which is cleaned per-calendar via Drop.
- **[LOW]** import_tw_tasks_bulk performs 100 separate `docker run --rm` invocations for the large-dataset test, making it extremely slow (estimated 2-5 minutes). This defeats the spirit of a 'performance' test.
  - Justification: The spec AC only says '100-task first sync completes without duplication or data loss', not a timing requirement. Functional correctness is met. A `task import` JSON approach would be far faster but is not required.
- **[LOW]** Status transitions spec description says 'both ways' but only CalDAV-completed→TW is tested. TW-completed→CalDAV COMPLETED is not explicitly covered in test_scenarios.rs.
  - Justification: The AC for task-6-4 only requires 'CalDAV COMPLETED syncs back to TW as completed task'. The TW→CalDAV direction is implicitly covered by the general sync path, and the spec AC does not mandate an explicit test for it.

## Test Coverage
**Status:** sufficient

16 integration tests across 3 test files plus 3 pure-unit tests in mod.rs (19 total integration suite entries). All required scenarios from the spec are represented: first-sync push, caldavuid UDA, dry-run, project routing, TW-wins LWW, CalDAV-wins LWW, loop-prevention stable-point, ETag conflict, CalDAV completion→TW, dependency sync (both directions), orphaned UID cleanup, and 100-task large dataset. The main gap is incomplete assertion coverage in two tests (orphaned + status), not missing test cases. Test isolation is strong: UUID-keyed calendars, Docker-isolated TW, per-test state reset. All tests compile cleanly (cargo build --tests zero warnings per journal). Full Docker test execution was deferred to verify-6-1 and results are not recorded in the journal; successful execution is assumed but not confirmed in this review.

## Code Quality

Overall the harness is well-structured: OnceLock-based idempotent container startup, strong isolation via UUID calendars and TempDir, clean Drop-based teardown, and a rich set of helper methods. The DockerizedTaskRunner correctly implements the TaskRunner trait so the main run_sync() path is exercised unchanged. Module registration (mod test_first_sync; mod test_lww; mod test_scenarios;) is correct. The Cargo.toml [[test]] block properly exposes the integration suite. No unsafe code. No credential leakage risks (test credentials are test-only).

- The disjunctive assertion `status == 'completed' || r2.written_tw >= 1` in status_sync_caldav_completed_to_tw is a maintenance trap: future regressions that write to TW without completing the task will silently pass.
- orphaned_caldavuid_causes_tw_deletion is misleadingly named; the body only verifies the CalDAV side. Readers will assume TW deletion is verified.
- import_tw_tasks_bulk spawns one Docker container per task (O(n) Docker overhead). For 100 tasks this is correct but very slow; a `task import` JSON batch would be idiomatic and orders of magnitude faster.
- ensure_radicale_running() uses a bare HTTP GET to port 5233 as the liveness check. A CalDAV OPTIONS or PROPFIND would be more semantically correct and would catch cases where the port is open but the CalDAV service is not yet ready.
- parse_hrefs_from_multistatus uses manual string scanning instead of an XML parser. It handles both <D:href> and <href> namespace variants and is tested, but edge cases (CDATA, attribute-qualified href elements, multi-byte characters) are not handled.

## Documentation
**Status:** adequate

Each file has a top-level doc comment. TestHarness and all public helpers have /// doc comments. The skip guard change (from checking Docker binary to SKIP_INTEGRATION_TESTS env var) is documented in the module-level comment. Key design decisions (htpasswd auth, MKCOL XML body, DockerizedTaskRunner) are explained inline. The LWW setup preconditions in test_lww.rs are commented clearly. No external README or test-running guide was added, but that is addressed in Phase 6 (Hardening & Docs).

## Issues

- orphaned_caldavuid_causes_tw_deletion: missing TW task deletion assertion — only CalDAV count is verified, leaving the primary AC half unvalidated (HIGH).
- status_sync_caldav_completed_to_tw: relaxed disjunctive assertion masks actual TW status after sync (MEDIUM).
- Full integration test execution results not recorded; suite correctness confirmed only by journal notes, not by observable test output in this review.

## Recommendations

- In orphaned_caldavuid_causes_tw_deletion: after r2, call get_tw_task(&uuid) and assert the JSON is null/empty OR use TW export with rc.status:deleted filter to confirm deletion. Also assert r2.written_tw >= 1 or r2.deleted_tw >= 1 if SyncResult tracks deletions.
- In status_sync_caldav_completed_to_tw: replace the disjunctive assert with two sequential assertions — first assert r2.written_tw >= 1, then re-export the task using `task {uuid} export rc.status:completed` (or check completed.data) to assert status=='completed'.
- Consider adding a StopRadicale / docker compose down step via a test main or a once_at_exit hook so CI environments do not leave containers running after the suite.
- Replace import_tw_tasks_bulk's per-task Docker loop with a single `task import` call fed a JSON array to dramatically reduce the large-dataset test runtime.
- Record or link actual test run output (cargo test --test integration) in the journal before closing the verify-6-1 task to confirm all 16 integration tests pass in a Docker-enabled environment.

---
*Generated by Foundry MCP Fidelity Review*