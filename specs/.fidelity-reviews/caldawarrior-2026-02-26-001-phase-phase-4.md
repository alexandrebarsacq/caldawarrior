# Fidelity Review: caldawarrior

**Spec ID:** caldawarrior-2026-02-26-001
**Scope:** phase (phase: phase-4)
**Verdict:** pass
**Date:** 2026-02-28T15:58:15.713907

## Summary

Phase 3 (Sync Algorithm) implementation is substantively complete and architecturally sound. All three modules (lww.rs, writeback.rs, mod.rs) are present and correctly implement the two-layer LWW resolver, the write-back decision tree with ETag retry ownership, and the three-step sync orchestrator. All SkipReason (8) and UpdateReason (5) variants are produced correctly. The single cross-model deviation — unavailability of DTSTAMP fallback due to upstream parser limitations — is handled gracefully via TW-wins tiebreaker and is explicitly documented and tested. No HIGH or CRITICAL severity deviations were identified by either reviewer. Claude additionally flagged dead-code paths, a stale doc comment, and the absence of compiled test-run output; none of these rise above medium severity.

## Requirement Alignment
**Status:** partial

Gemini assessed alignment as fully 'yes'; claude assessed it as 'partial' due to the DTSTAMP fallback AC being unsatisfiable at the Phase 3 layer without an upstream parser fix. All other core ACs are met: resolve_lww() uses correct timestamp ordering, the 8-field content-identical check covers all required fields with correct normalization, X-CALDAWARRIOR-LAST-SYNC is set on every CalDAV write, ETag retry is owned exclusively in apply_writeback, CalDAV-only NEEDS-ACTION routes through tw.create(), depends are reverse-mapped via the IR HashMap index, and run_sync executes the three steps in order.

## Success Criteria
**Status:** partial

Gemini rated all ACs as met; claude rated them 'partial' specifically because (1) the DTSTAMP fallback AC cannot be satisfied at the Phase 3 layer without a parser change, and (2) no compiled cargo test output is available to confirm all 24 tests compile and pass. All other verifiable ACs — TW-wins tiebreaker, regression test coverage, ETag retry, dry-run passthrough, mock adapter integration tests, three-step pipeline order — are demonstrably satisfied by code inspection.

## Deviations

- **[MEDIUM]** DTSTAMP fallback for LAST-MODIFIED comparison is unavailable. The spec requires using DTSTAMP as a fallback when LAST-MODIFIED is absent ('for comparison only, never on write path'), but the upstream iCalendar parser discards DTSTAMP at parse time, making it inaccessible at the Phase 3 layer. When LAST-MODIFIED is absent, the implementation degrades to the TW-wins tiebreaker instead.
  - Justification: Inherited constraint from the Phase 1/2 iCalendar parser; cannot be resolved within Phase 3 alone. The fallback (TW-wins) is safe and conservative — it pushes TW state rather than silently missing a CalDAV update. The limitation is explicitly documented in inline comments and covered by a dedicated test ('no_last_modified_dtstamp_fallback_unavailable_tw_wins'). The write-path portion of the AC is trivially satisfied since DTSTAMP is not available at all.
- **[LOW]** PlannedOp::DeleteFromCalDav and PlannedOp::DeleteFromTw variants have full execute_op handlers but are never produced by decide_op in writeback.rs, making them unreachable dead code paths in Phase 3.
  - Justification: The sync design uses soft-delete (CANCELLED status) rather than hard delete, which is consistent with the spec. The handlers may serve future phases or hardening. No functional impact on Phase 3 sync behavior.
- **[LOW]** VTODO DESCRIPTION is always written equal to SUMMARY (both set to tw.description in build_vtodo_from_tw). Repeated sync cycles will overwrite a CalDAV-sourced DESCRIPTION with the SUMMARY content.
  - Justification: Consistent with the field-mapping convention used throughout the codebase and reflected symmetrically in the content_identical check. This is an intentional architectural simplification documented in code comments rather than a bug.
- **[MEDIUM]** No compiled test-run results are available. All 24 tests were deferred to verify-4-1 because cargo was unavailable in the implementation environment. Compilation and runtime behavior are unconfirmed.
  - Justification: Explicitly documented in the implementation journal. Code inspection by claude strongly suggests tests are syntactically correct. Confirmation is expected in the Phase 3 verification task. Gemini did not flag this as a concern.

## Test Coverage
**Status:** sufficient

Both models rated test coverage as sufficient. lww.rs contains 7 unit tests covering all LWW decision paths (TW-wins, CalDAV-wins, identical, no-LAST-SYNC, regression, status normalization, DTSTAMP fallback unavailability). writeback.rs contains 13 unit tests covering all PlannedOp decision paths including ETag retry exhaustion, dry-run, and dependency mapping. mod.rs contains 4 integration tests using mock adapters (MockCalDavClient, MockTaskRunner) covering the full pipeline, dry-run, warning collection, and step ordering. The regression AC (CalDAV-wins then identical-on-resync) is covered at the unit level. Gap noted by claude: no cargo test run output confirms actual compilation and passage.

## Code Quality

Gemini found no code quality issues, characterizing the implementation as clean, idiomatic Rust with excellent use of enums for modeling outcomes and a well-structured decision tree minimizing cyclomatic complexity. Claude concurred on overall quality but identified four minor issues (dead code paths, stale comment, minor asymmetry, loose test assertion). All issues are low severity. Architecture-positive notes shared by both: injected now: DateTime<Utc> enables deterministic testing, generics (TaskRunner) keep adapters testable, helper functions are well-factored, and build_caldav_index is correctly built once per apply_writeback invocation.

- [claude] Dead code: PlannedOp::DeleteFromCalDav and DeleteFromTw are handled in execute_op but never produced by decide_op. Should be removed or annotated with a TODO explaining intended future use.
- [claude] Stale doc comment: SkipReason::Identical in types.rs references the old 8-field list ('uuid, description, status, due, scheduled, priority, project, tags') rather than the final spec fields (SUMMARY, STATUS, DUE, DTSTART, COMPLETED, RELATED-TO[DEPENDS-ON], X-TASKWARRIOR-WAIT, DESCRIPTION). Implementation is correct; only the comment is outdated.
- [claude] Minor asymmetry in content_identical: SUMMARY check uses Some(tw.description.as_str()) (returns false if CalDAV SUMMARY is None) while DESCRIPTION check uses unwrap_or(''), creating inconsistent behavior when fields are absent.
- [claude] ETag retry exhaustion error message assertion in tests uses loose .contains('SyncConflict') OR .contains('ETag') matching, which could mask future message format regressions.

## Documentation
**Status:** adequate

Both models rated documentation as adequate. All three public functions (resolve_lww, apply_writeback, run_sync) have thorough doc comments explaining algorithms, panic conditions, and design decisions. The DTSTAMP limitation is clearly noted inline. The Layer 1/Layer 2 loop-prevention contract is documented on both resolve_lww and build_vtodo_from_tw. The one identified gap (stale SkipReason::Identical doc comment in types.rs) is minor and does not affect runtime correctness.

## Issues

- DTSTAMP fallback for LAST-MODIFIED comparison is unavailable due to upstream parser discarding the property — falls back to TW-wins tiebreaker rather than DTSTAMP comparison as specced. [claude, gemini]
- No compiled test-run evidence confirming all 24 tests compile and pass; deferred to verify-4-1. [claude]
- Dead code: DeleteFromCalDav and DeleteFromTw PlannedOp variants are handled in execute_op but never produced by decide_op. [claude]
- Stale SkipReason::Identical doc comment in types.rs references wrong 8-field set from an earlier draft. [claude]

## Recommendations

- Run 'cargo test' in verify-4-1 to confirm all 24 tests compile and pass; specifically validate the regression test and DTSTAMP fallback test.
- Fix the iCalendar parser (Phase 1/2) to expose DTSTAMP as a fallback for LAST-MODIFIED, or formally accept the limitation and update the spec to reflect the tiebreaker behavior.
- Update the SkipReason::Identical doc comment in types.rs to reference the correct 8 fields: SUMMARY, STATUS, DUE, DTSTART, COMPLETED, RELATED-TO[DEPENDS-ON], X-TASKWARRIOR-WAIT, DESCRIPTION.
- Either remove DeleteFromCalDav/DeleteFromTw handlers from execute_op or add a TODO comment documenting the intended future use case.
- Harden the ETag retry exhaustion test assertion to match the exact expected message format rather than using loose substring checks.
- Proceed with Phase 4: CLI & Output once verify-4-1 confirms test passage.

## Verdict Consensus

- **pass:** claude, gemini

**Agreement Level:** strong

Both models independently voted 'pass'. Verdict is unambiguous. The models diverge on the granularity of alignment and criteria assessments (claude rates both 'partial' due to the DTSTAMP gap and missing test-run output; gemini rates both 'yes'), but neither escalates any deviation to high or critical severity, so the overall verdict remains 'pass' under the tiebreaker rule as well.

## Synthesis Metadata

- Models consulted: claude, gemini
- Models succeeded: claude, gemini
- Synthesis provider: claude

---
*Generated by Foundry MCP Fidelity Review*