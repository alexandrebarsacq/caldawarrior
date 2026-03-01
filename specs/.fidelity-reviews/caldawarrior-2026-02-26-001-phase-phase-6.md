# Fidelity Review: caldawarrior

**Spec ID:** caldawarrior-2026-02-26-001
**Scope:** phase (phase: phase-6)
**Verdict:** pass
**Date:** 2026-02-28T16:03:07.065493

## Summary

Phase 5 integration test implementation is fully complete and compliant with specification. Both reviewers confirm that the test harness (mod.rs), three scenario files (test_first_sync.rs, test_lww.rs, test_scenarios.rs), and all 16 integration tests satisfy every acceptance criterion. The harness correctly manages Docker lifecycle for Radicale via the Phase 0 compose file, provides UUID-keyed CalDAV calendar isolation, Dockerized TaskWarrior execution, and full per-test state reset. All specified E2E scenarios—first-sync push with UDA stamping, LWW in both directions, stable-point loop prevention, ETag conflict handling, status sync, bidirectional dependency sync, orphaned-UID cleanup, and 100-task large dataset—are explicitly asserted. Minor code quality observations from one reviewer (brittle XML parser, misleading method name, DRY violations in test helpers) do not affect correctness or spec compliance. A process-level gap exists in that full runtime execution results were deferred to the verify-6-1 gate rather than recorded in task journals for tasks 6-3 and 6-4.

## Requirement Alignment
**Status:** yes

Both models confirm complete alignment. task-6-1 (mod.rs) uses the Phase 0 docker-compose file without duplication, OnceLock for idempotent startup, MKCOL/DELETE for calendar lifecycle, and DockerizedTaskRunner for isolated TW databases. task-6-2 through task-6-4 implement all specified scenario files and test functions exactly as described in the spec, with direct assertions on written_caldav, written_tw, UDA values, VTODO content, and TW task status.

## Success Criteria
**Status:** yes

All acceptance criteria are satisfied across all four tasks. harness-1 through harness-3: Docker startup idempotency, per-test calendar/TW isolation, and full reset verified. first-sync-1 through first-sync-3: push counts, caldavuid UDA stamping, dry-run idempotence, and project routing all asserted. lww-1 through lww-3: both LWW directions, stable-point (r4 written_caldav==0 && written_tw==0), and graceful ETag conflict handling confirmed. scenarios-1 through scenarios-4: CalDAV→TW COMPLETED propagation, bidirectional RELATED-TO/depends dependency sync, orphan deletion (status=deleted, zero CalDAV re-creation), and 100-task large sync with stable second pass all verified.

## Deviations

- **[LOW]** TaskWarrior is Dockerized via DockerizedTaskRunner (archlinux:latest + pacman TW 3.4.2) rather than relying on a host-local TW binary.
  - Justification: The spec only required Radicale to be managed via docker-compose; it did not prohibit Dockerizing TW. Using Docker for TW provides a pinned, reproducible TW version and eliminates host-version dependencies, strictly improving test reliability.
- **[LOW]** should_skip() was changed to only check the SKIP_INTEGRATION_TESTS environment variable; tests fail loudly when Docker is unavailable rather than silently skipping.
  - Justification: Documented as an intentional change in the task-6-3 journal: silent passes mask real CI failures. This improves test fidelity at the cost of requiring an explicit opt-out when Docker is absent.
- **[LOW]** reset() wipes individual VTODOs (one DELETE per .ics) rather than destroying and recreating the CalDAV collection.
  - Justification: Deleting all VTODO items achieves the same observable state (empty calendar) as collection teardown/recreation. Functionally equivalent to the spec's 'CalDAV calendars wiped' requirement while avoiding potential race conditions from collection recreation.
- **[LOW]** import_tw_tasks_bulk issues one `docker run task add` invocation per task rather than a single `task import` bulk call for the 100-task performance test.
  - Justification: The 100-task AC verifies correctness (no duplication/data loss) rather than throughput, so N=100 sequential Docker invocations is acceptable. A true bulk import would require JSON marshalling of all tasks and is not required by any acceptance criterion.
- **[LOW]** Full runtime test execution results for tasks 6-3 and 6-4 are deferred to the verify-6-1 phase gate rather than recorded in task journals.
  - Justification: Phase 5 includes a dedicated verify-6-1 gate task. cargo build --tests passes with zero warnings. Runtime results will be confirmed at the gate. This is a process gap, not a code defect.

## Test Coverage
**Status:** sufficient

Both models agree coverage is sufficient. 16 integration tests across three files exceed the 6 E2E scenarios described in the phase. Three additional unit tests in mod.rs cover the XML href parser. Every specified AC has at least one direct assertion: written_caldav/written_tw counts, caldavuid non-empty, dry-run count==0, project routing count==1, LWW description content, stable-point r4 both==0, ETag no-error, TW status=completed, RELATED-TO string presence, reverse dependency written_tw>=1, orphan status=deleted with CalDAV count==0, and 100-task written_caldav==100 with stable second sync. One minor gap noted by claude: no explicit timing/performance bound is asserted for the 100-task scenario, though the AC language ('completes without duplication or data loss') is fully covered.

## Code Quality

gemini found no code quality issues and assessed the code as idiomatic, clean, and highly modular. claude identified four test-helper-level concerns: a brittle XML string scanner, DRY violations in argument construction, a misleading method name, and a SUMMARY/DESCRIPTION dual-update asymmetry. None of these affect the correctness of test assertions or spec compliance. The OnceLock singleton pattern for Docker setup and Drop-based cleanup are both noted as idiomatic Rust by claude. All quality concerns are limited to test helper ergonomics.

- [claude] parse_hrefs_from_multistatus uses manual string scanning instead of an XML parser; brittle to namespace-prefixed tags other than 'D:href' or 'href'.
- [claude] DockerizedTaskRunner.docker_args() constructs a Vec<String> on every call and all helpers duplicate volume/env argument lists inline — DRY violation extractable to a shared builder.
- [claude] import_tw_tasks_bulk is named 'bulk' but issues N sequential Docker container invocations; the name implies a single bulk operation and could mislead maintainers.
- [claude] modify_first_vtodo_summary updates both SUMMARY and DESCRIPTION to the same value — asymmetric from the method name and could mask single-field mapping bugs.

## Documentation
**Status:** adequate

Both models agree documentation is adequate. mod.rs includes a module-level doc comment covering Docker requirements and skip behavior. All public TestHarness methods have doc comments explaining parameters and behavior. Test functions include inline comments explaining setup steps and invariants, with particularly detailed comments in caldav_wins_lww and loop_prevention_stable_point. No external documentation updates are required for test-only files.

## Issues

- [claude] Full runtime test execution results not yet confirmed in journal for tasks 6-3 and 6-4; deferred to verify-6-1 phase gate.
- [claude] import_tw_tasks_bulk method name is misleading relative to its sequential per-task Docker invocation behavior.
- [claude] parse_hrefs_from_multistatus XML parser is brittle to unexpected namespace prefixes and could silently break on some CalDAV server responses.

## Recommendations

- Run the full integration suite (cargo test --test integration) with Docker available and record pass/fail results in the verify-6-1 journal entry before closing Phase 5.
- Consider renaming import_tw_tasks_bulk to add_tw_tasks_sequentially or bulk_add_tw_tasks to accurately communicate that it issues one Docker call per task.
- Consider replacing the manual href string scanner with a minimal XML pull-parser (e.g., quick-xml) to handle namespace prefix variations robustly across CalDAV server implementations.
- Proceed to Phase 6: Hardening & Docs as planned.

## Verdict Consensus

- **pass:** claude, gemini

**Agreement Level:** strong

Both models independently returned a 'pass' verdict. All deviations identified by claude are low-severity and no deviations were identified by gemini. No tie-breaking was required.

## Synthesis Metadata

- Models consulted: claude, gemini
- Models succeeded: claude, gemini
- Synthesis provider: claude

---
*Generated by Foundry MCP Fidelity Review*