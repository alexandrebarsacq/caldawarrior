# Fidelity Review: caldawarrior

**Spec ID:** caldawarrior-2026-02-26-001
**Scope:** phase (phase: phase-4)
**Verdict:** partial
**Date:** 2026-02-27T14:41:32.752465

## Summary

Phase 3 implementation is structurally sound and covers the majority of spec requirements across all three files. The LWW algorithm, write-back decision tree, and sync orchestrator are correctly architected with proper ETag retry ownership, warning aggregation, injected-clock testing, and the full 8-field Layer 2 identical check. However, three issues prevent a clean pass: (1) DTSTAMP fallback is explicitly required as an AC but is absent and untested due to a parser-level limitation from an earlier phase; (2) several writeback decision-tree branches lack direct unit tests (TW-completed → CalDAV COMPLETED, CalDAV-completed → TW update), violating the 'all branches' AC; and (3) no compilation or test run has been executed, so we cannot confirm tests pass or that the code is free of compiler errors. A secondary concern is an inconsistency in wait-expiry logic: `content_identical` uses the injected `now`, while `tw_to_caldav_fields` calls `Utc::now()` directly, creating a subtle non-determinism in the write path.

## Requirement Alignment
**Status:** partial

task-4-1 (lww.rs): All 8 Layer-2 fields are checked with correct normalization (status enum, second-precision timestamps, sorted RELATED-TO, expired-wait collapse). Layer 1 LWW logic is correct; epoch default for missing LAST-SYNC ensures TW wins on first sync. DTSTAMP fallback required by AC is absent — the parser discards DTSTAMP and no workaround is implemented. task-4-2 (writeback.rs): apply_writeback signature matches spec (plus justified `now` injection). ETag retry is exclusively owned here (MAX_ETAG_RETRIES=3), full decision tree re-evaluated on each retry. create() vs update() routing is correct. CalDAV-uid-to-TW-uuid reverse index is built and passed through. X-CALDAWARRIOR-LAST-SYNC is set on every CalDAV write via build_vtodo_from_tw. Two SkipReason variants (Completed, CalDavDeletedTwTerminal) are defined in types.rs but never emitted — CalDAV-only terminal entries return None from decide_op rather than PlannedOp::Skip, which still increments skipped but loses the semantic reason. task-4-3 (mod.rs): Three-step pipeline in correct order, ETag retry not re-implemented, warnings drained and merged from all three steps, dry_run passed through. Depends mapping: tw_to_caldav_fields converts TW UUIDs directly to strings, not through entry.resolved_depends as specified; this is valid only because caldavuid UDA design makes TW UUID == CalDAV UID, but deviates from the spec's stated mechanism.

## Success Criteria
**Status:** partial

Met: TW-wins/CalDAV-wins/Identical tests in lww.rs; regression test (CalDAV-wins then Identical on resync); status normalization test; all writeback branches tested except completed state transitions; integration tests for three-step flow, dry-run, warning collection, dependency ordering; ETag exhaustion error accumulation test; LAST-SYNC written on every CalDAV PUT. Not met: DTSTAMP fallback test absent from lww.rs test suite (AC explicitly requires it); writeback tests do not cover TW-completed → CalDAV COMPLETED (TwCompletedMarkCompleted) or CalDAV-completed → TW update (CalDavCompletedUpdateTw) branches; regression AC specifies 'written_caldav==0 && written_tw==0' but the lww.rs regression test only checks Skip(Identical) at the LWW level without going through apply_writeback (a companion writeback test paired_identical_skips does verify the counters but is not framed as the regression test). No test results: compilation and test passage are entirely unverified.

## Deviations

- **[MEDIUM]** DTSTAMP fallback for LWW comparison is not implemented. The spec AC requires 'LWW uses TW.modified vs CalDAV LAST-MODIFIED (DTSTAMP fallback for comparison only, never on write path)' and a dedicated unit test for the DTSTAMP fallback case.
  - Justification: The iCalendar parser from Phase 2 discards DTSTAMP; the code documents this limitation explicitly. The behavior degrades safely to TW-wins when LAST-MODIFIED is absent. However, this does not satisfy the spec AC and the required test is completely missing.
- **[HIGH]** No compilation or test run has been executed. The journal explicitly defers 'Full test run to verify-4-1 (cargo unavailable in env)'. No test results are available for any of the three files.
  - Justification: Environment limitation (cargo unavailable). The code is structurally consistent and compiles-looking, but Rust's borrow checker and type system could reveal issues that only surface at compile time.
- **[MEDIUM]** Unit tests for writeback.rs do not cover TwCompletedMarkCompleted (TW completed → CalDAV COMPLETED PUT) and CalDavCompletedUpdateTw (CalDAV COMPLETED → tw.update()) branches. The AC requires coverage of all branches with MockCalDavClient injection.
  - Justification: The decision-tree logic for both branches exists in decide_op and execute_op, but no test exercises them. A basic test verifying written_caldav=1 for TW-completed and written_tw=1 for CalDAV-completed is missing.
- **[LOW]** Two SkipReason variants (Completed, CalDavDeletedTwTerminal) are defined in types.rs but are never emitted by the Phase 3 decision tree. CalDAV-only COMPLETED/CANCELLED entries return None from decide_op rather than PlannedOp::Skip. The AC states 'All SkipReason variants used correctly per decision tree'.
  - Justification: The counting behavior (result.skipped += 1) is still correct. Returning None is semantically valid for terminal CalDAV-only entries but loses the per-variant diagnostic reason. Potentially intentional, but diverges from the AC.
- **[LOW]** tw_to_caldav_fields uses Utc::now() directly for the wait-expiry collapse, while content_identical uses the injected `now: DateTime<Utc>` parameter for the same check. This creates a timing inconsistency between the Layer-2 comparison and the actual write.
  - Justification: For practical sync runs the two calls are nanoseconds apart, so the risk of mismatching behavior is negligible. However, it makes the write path non-deterministic in tests and creates a subtle asymmetry between what content_identical checks and what build_vtodo_from_tw writes.
- **[LOW]** Depends mapping in build_vtodo_from_tw uses tw_to_caldav_fields(tw).depends (TW UUIDs as strings) rather than entry.resolved_depends (CalDAV UIDs from IR resolution) as specified by the AC 'TW depends reverse-mapped from resolved_depends through IR HashMap index'.
  - Justification: Functionally equivalent because the caldavuid UDA design ensures TW UUID == CalDAV UID for all synced tasks. The comment in tw_to_caldav_fields acknowledges this design invariant. However, the implementation deviates from the spec's stated mechanism.

## Test Coverage
**Status:** insufficient

lww.rs: 6 tests covering TW-wins, CalDAV-wins, Identical, first-sync (no LAST-SYNC), regression (CalDAV-wins → Identical), status normalization. Missing: DTSTAMP fallback test (required by AC). writeback.rs: 11 tests covering TW-only pending push, TW-only deleted skip, CalDAV-only NEEDS-ACTION create, CalDAV-only COMPLETED skip, paired TW-wins, paired CalDAV-wins, paired Identical, ETag exhaustion, dry-run, TW-deleted → CalDAV CANCELLED, cyclic skip. Missing: TW-completed → CalDAV COMPLETED, CalDAV-completed → TW update. mod.rs: 4 integration tests covering TW-only push, dry-run, warning collection, dependency resolution ordering. Coverage is missing for paired scenarios and CalDAV-only at the orchestration level. Critically: no test run results exist to confirm any of these tests actually pass.

## Code Quality

Overall the code is well-structured Rust with good separation of concerns, proper error propagation, injected clocks for testability, and clear documentation. The primary quality concern is the unverified compilation state and the non-deterministic wait-check in the mapper.

- apply_entry re-evaluates decide_op on each ETag retry attempt — correct behavior, but only the first planned_op is recorded (attempt==0 guard); subsequent retry decisions (which may differ after refetch) are not recorded. This is acceptable but could complicate audit logging.
- build_vtodo_from_tw ignores the VTODO categories field from the existing entry in one path (when fetched_vtodo is None, categories defaults to empty). This is documented via `base.map(...).unwrap_or_default()` but silently drops categories for new push entries.
- The `_` fallback arm in execute_op's ResolveConflict match (`_ => Ok(true)`) silently succeeds for any unhandled (Side, UpdateReason) combination, which could mask future bugs when new variants are added.
- tw_to_caldav_fields is called inside a retry loop (via execute_op → build_vtodo_from_tw) and internally calls Utc::now() on each call; in a tight retry loop this is inconsequential but is architecturally impure.
- PullFromCalDav entry's tw_task.is_none() guard in execute_op is dead code in practice: the CalDAV-only arm of decide_op only produces PullFromCalDav for (None, Some(vtodo)) entries, so tw_task is always None here. The update() fallback branch is unreachable via this op variant.

## Documentation
**Status:** adequate

All three files carry module-level and function-level doc comments that accurately describe behavior, spec references, and known limitations (DTSTAMP parser limitation is explicitly called out in both the docstring and inline comments). The SkipReason::Identical doc comment in types.rs contains a stale field list ('uuid, description, status, due, scheduled, priority, project, tags') that does not match the actual 8-field contract (SUMMARY, DESCRIPTION, STATUS, DUE, DTSTART, COMPLETED, RELATED-TO[DEPENDS-ON], X-TASKWARRIOR-WAIT). This doc drift is a minor but concrete inaccuracy.

## Issues

- No compilation or test run executed — correctness unverified at the compiler level (HIGH)
- DTSTAMP fallback not implemented and no DTSTAMP unit test present, violating explicit spec ACs (MEDIUM)
- Writeback tests missing for TwCompletedMarkCompleted and CalDavCompletedUpdateTw branches (MEDIUM)
- SkipReason::Completed and SkipReason::CalDavDeletedTwTerminal unused in Phase 3 decision tree (LOW)
- Wait-expiry inconsistency: content_identical uses injected `now`, build_vtodo_from_tw uses wall-clock Utc::now() (LOW)
- SkipReason::Identical docstring in types.rs lists stale field names inconsistent with actual 8-field contract (LOW)

## Recommendations

- Run `cargo test` as the immediate next step (verify-4-1 gate); resolve any compilation errors before proceeding to Phase 4.
- Add a DTSTAMP fallback unit test — even though the parser discards DTSTAMP, add a test that documents the degraded behavior (TW wins when LAST-MODIFIED is absent) and note the parser limitation explicitly so that if DTSTAMP parsing is added in a future phase, the test can be updated.
- Add writeback unit tests for TwCompletedMarkCompleted (TW status='completed', CalDAV status='NEEDS-ACTION' → PUT with COMPLETED, assert written_caldav=1) and CalDavCompletedUpdateTw (CalDAV status='COMPLETED', TW status='pending' → tw.update, assert written_tw=1).
- Fix the SkipReason::Identical docstring in types.rs to list the actual 8 fields: SUMMARY, DESCRIPTION, STATUS, DUE, DTSTART, COMPLETED, RELATED-TO[DEPENDS-ON], X-TASKWARRIOR-WAIT.
- Consider threading `now` into tw_to_caldav_fields (or into build_vtodo_from_tw as an argument) so the wait-expiry check is consistent with content_identical and fully deterministic in tests.
- Decide whether SkipReason::Completed and CalDavDeletedTwTerminal should be emitted for CalDAV-only terminal entries or removed from the enum to eliminate dead variants.
- Add a guard or comment for the `_ => Ok(true)` fallback arm in execute_op's ResolveConflict match to prevent silent success for future unhandled (Side, UpdateReason) combinations.

---
*Generated by Foundry MCP Fidelity Review*