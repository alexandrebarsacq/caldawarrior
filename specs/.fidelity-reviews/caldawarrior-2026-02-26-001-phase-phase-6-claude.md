# Fidelity Review: caldawarrior

**Spec ID:** caldawarrior-2026-02-26-001
**Scope:** phase (phase: phase-6)
**Verdict:** pass
**Date:** 2026-02-28T16:01:59.969815
**Provider:** claude

## Summary

All four Phase 5 tasks are fully implemented and all specification acceptance criteria are satisfied. The test harness (mod.rs) provides proper Docker lifecycle management for Radicale via Phase 0 docker-compose, isolated per-test CalDAV calendars and TW databases, and full state reset capability. The 16 integration tests across three scenario files cover every specified AC: first-sync push with UDA stamping, project mapping, LWW both directions, stable-point loop prevention, ETag conflict resolution, status sync, bidirectional dependency sync, orphaned-UID cleanup, and 100-task large dataset. Notable positive deviations improve the design without violating spec intent. The implementation builds cleanly (zero warnings per journal) and the code quality is high.

## Requirement Alignment
**Status:** yes

task-6-1 (mod.rs): harness starts Radicale via `docker compose up -d` from tests/integration/ (Phase 0 compose file, not duplicated), uses OnceLock for idempotency, creates/deletes UUID-keyed CalDAV calendars via authenticated MKCOL/DELETE, manages isolated TW databases in TempDir mounted into Docker, and provides reset() that wipes both .data files and all VTODOs. task-6-2 (test_first_sync.rs): four tests cover push (2 tasks→2 VTODOs), caldavuid UDA stamping, dry-run no-write, and project→calendar routing — all three ACs met. task-6-3 (test_lww.rs): tw_wins_lww and caldav_wins_lww cover both LWW directions; loop_prevention_stable_point asserts written_caldav==0 && written_tw==0 on sync 4; etag_conflict_scenario verifies graceful ETag conflict handling — all three ACs met. task-6-4 (test_scenarios.rs): status_sync_caldav_completed_to_tw confirms TW status=completed; dependency_sync_tw_to_caldav covers both TW→CalDAV RELATED-TO and CalDAV→TW depends reverse; orphaned_caldavuid_causes_tw_deletion confirms status=deleted and zero CalDAV re-creation; large_dataset_first_sync verifies written_caldav=100 + stable second sync — all four ACs met.

## Success Criteria
**Status:** yes

AC harness-1 (starts/stops Radicale via Phase 0 docker-compose): met via ensure_radicale_running() using COMPOSE_DIR pointing to tests/integration/. AC harness-2 (creates/deletes calendars and tasks programmatically): met via TestHarness::new() MKCOL + Drop DELETE + add_tw_task() Docker invocations. AC harness-3 (full reset between test cases): met via reset() → wipe_caldav() (per-VTODO DELETE) + wipe_tw() (.data file removal). AC first-sync-1/2/3: all met in test_first_sync.rs. AC lww-1 (both TW-wins and CalDAV-wins): met with written_* assertions and TW description verification. AC lww-2 (stable-point written_caldav==0 && written_tw==0): explicitly asserted on r4 in loop_prevention_stable_point. AC lww-3 (ETag conflict): met in etag_conflict_scenario with no-error assertion. AC scenarios-1 through -4: all met with explicit status, RELATED-TO content, deletion, and count assertions.

## Deviations

- **[LOW]** TaskWarrior is also Dockerized (DockerizedTaskRunner), not run as a host-local binary.
  - Justification: Journal entry for task-6-2 explains the switch to archlinux:latest + pacman task (TW 3.4.2) to get a pinned, reproducible TW version. The spec only mentioned starting/stopping Radicale via docker-compose but did not prohibit Dockerizing TW; using Docker for TW is strictly more reproducible and avoids host TW version dependencies.
- **[LOW]** should_skip() was changed to only check SKIP_INTEGRATION_TESTS env var; tests fail loudly when Docker is unavailable rather than silently skipping.
  - Justification: Journal entry for task-6-3 documents this intentional change: silent passes mask real failures in CI. This improves test fidelity at the cost of requiring explicit opt-out when Docker is absent.
- **[LOW]** reset() wipes individual VTODOs (one DELETE per .ics) rather than destroying and recreating the CalDAV collection.
  - Justification: The spec states 'CalDAV calendars wiped' — deleting all VTODO items achieves the same observable state (empty calendar) without the overhead and potential race conditions of recreating the collection. Functionally equivalent.
- **[LOW]** import_tw_tasks_bulk issues one `docker run task add` per task rather than a single `task import` bulk call for the 100-task performance test.
  - Justification: The test verifies correctness (no duplication/loss) rather than speed, so N=100 Docker invocations is acceptable. A true bulk import would require JSON marshalling of all tasks and is not required by the AC.
- **[LOW]** Full test suite execution results are not recorded in the journal for tasks 6-3 and 6-4; journal entries say 'Full test execution deferred to verify-6-1'.
  - Justification: The Phase 5 spec includes a verify-6-1 gate task. cargo build --tests passes with zero warnings per journal. Runtime results are expected to be confirmed in the verification phase. This is a process gap, not a code defect.

## Test Coverage
**Status:** sufficient

16 integration tests implemented across three files, exceeding the 6 E2E scenarios mentioned in the phase description. 3 pure-unit tests in mod.rs cover the XML href parser. All specified ACs have direct test assertions: push counts (written_caldav), UDA presence (caldavuid non-empty), dry-run idempotence (count==0), project routing (count==1), LWW direction (written_* values + TW description content), stable-point (r4 both==0), ETag no-error, CalDAV→TW COMPLETED propagation (status=completed), RELATED-TO presence (string match in iCal body), reverse dependency creation (written_tw>=1), orphan deletion (status=deleted + caldav count==0), large dataset (written_caldav==100 + stable second sync). Minor gap: no explicit timing/performance bound is asserted for the 100-task scenario (spec AC says 'completes without duplication or data loss', which is what is asserted).

## Code Quality

Overall code quality is high. OnceLock for singleton Docker setup is correct for parallel test runners. Drop-based cleanup is idiomatic Rust. Helper methods are well-documented with doc comments. The DockerizedTaskRunner correctly implements the TaskRunner trait interface. TestHarness encapsulates all state cleanly. The main quality concerns are limited to test helper ergonomics and the brittle XML parser, none of which affect correctness of the test assertions.

- parse_hrefs_from_multistatus uses manual string scanning instead of an XML parser; while acceptable in test code, it would silently break on namespace-prefixed tags other than 'D:href' or 'href'.
- DockerizedTaskRunner.docker_args() constructs a Vec<String> on every call and all helpers duplicate the volume/env argument list inline — DRY violation that could be extracted to a shared builder.
- import_tw_tasks_bulk is named 'bulk' but issues N=100 sequential Docker container invocations; the name implies a single bulk operation (like `task import`) which could confuse maintainers.
- modify_first_vtodo_summary updates both SUMMARY and DESCRIPTION to the same value — asymmetric from the method name and could mask bugs where only one field is mapped.

## Documentation
**Status:** adequate

mod.rs has a module-level doc comment explaining Docker requirements and skip behavior. All public methods on TestHarness have doc comments explaining parameters and behavior. Test functions have inline comments explaining setup steps and the invariant being asserted. The caldav_wins_lww and loop_prevention_stable_point tests include particularly detailed comments explaining the LWW precondition setup. No external documentation updates (README/CHANGELOG) are required for test-only files.

## Issues

- Full runtime test execution results not yet confirmed in journal (deferred to verify-6-1 phase gate).
- import_tw_tasks_bulk name is misleading relative to its sequential Docker invocation behavior.
- parse_hrefs_from_multistatus XML parser is brittle to unexpected namespace prefixes.

## Recommendations

- Run the full integration suite (cargo test --test integration) with Docker available and record results in the verify-6-1 journal entry before closing Phase 5.
- Consider renaming import_tw_tasks_bulk to add_tw_tasks_sequentially or bulk_add_tw_tasks to accurately reflect that it issues one Docker call per task.
- Consider replacing the manual href string scanner with a minimal XML pull-parser (e.g., quick-xml) to handle namespace prefix variations robustly in future CalDAV server compatibility.

---
*Generated by Foundry MCP Fidelity Review*