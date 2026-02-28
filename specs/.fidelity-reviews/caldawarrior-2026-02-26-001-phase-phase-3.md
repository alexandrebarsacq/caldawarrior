# Fidelity Review: caldawarrior

**Spec ID:** caldawarrior-2026-02-26-001
**Scope:** phase (phase: phase-3)
**Verdict:** pass
**Date:** 2026-02-27T10:04:23.185251

## Summary

All four Phase 2 tasks are implemented and meet their acceptance criteria. The status mapper correctly models all 5 TW states including the TwStateDeleted descriptor semantics. The field mapper implements all bidirectional mappings including expired-wait collapse. The IR builder correctly performs three-way TW classification, pre-assigns UUIDs for CalDAV-only NEEDS-ACTION entries, resolves calendar URLs at construction time, and emits all required warnings. The dependency resolver builds an O(1) HashMap index, populates resolved_depends with CalDAV UIDs, and runs an iterative 3-colour DFS for cycle detection. Test coverage is thorough (8, 16, 10, and 5 tests respectively). The sole open risk is that cargo was unavailable in the implementation environment, so compilation and test execution could not be confirmed at task-completion time — this is flagged as a medium concern but is not a code defect.

## Requirement Alignment
**Status:** yes

task-3-1: All 5 TW status branches (pending→NeedsAction, waiting→NeedsActionWithWait, recurring→Skip(Warning), completed→Completed(DateTime), deleted→TwStateDeleted) are covered. The TwStateDeleted doc comment explicitly states it is NOT a hard-delete instruction. task-3-2: All required bidirectional field mappings are present — description↔DESCRIPTION, due↔DUE, scheduled↔DTSTART, wait↔X-TASKWARRIOR-WAIT (with Phase 0 finding #6 expired-wait collapse), depends↔RELATED-TO[RELTYPE=DEPENDS-ON]. task-3-3: Three-way TW classification is correct; fresh UUID4 is assigned for TW-only-new and for CalDAV-only NEEDS-ACTION/IN-PROCESS entries; CalDAV-only COMPLETED/CANCELLED keep tw_uuid=None; orphaned uid is preserved; calendar_url is resolved from config at construction; UnmappedProject and RecurringCalDavSkipped warnings are emitted at the correct sites; all dirty_* and cyclic fields are false after construction. task-3-4: HashMap<Uuid,usize> index for O(1) lookups; resolved_depends populated with CalDAV UID strings; UnresolvableDependency emitted for missing IR entries and for entries with no caldav_uid; iterative 3-colour DFS correctly marks all cyclic nodes; CyclicEntry warning emitted per affected node.

## Success Criteria
**Status:** yes

task-3-1 ACs: (1) All 5 statuses ✓ (2) TwStateDeleted is a descriptor, not a delete signal ✓ (3) recurring → Skip(Warning containing 'recurring') without panic ✓ (4) 8 unit tests covering all 5 branches plus fallback ✓. task-3-2 ACs: (1) All bidirectional mappings ✓ (2) Expired-wait collapse implemented with `w > Utc::now()` guard ✓ (3) depends round-trip test confirms all edges preserved ✓ (4) 16 tests including None/absent cases ✓. task-3-3 ACs: (1) TW-only-new: Uuid::new_v4().to_string() assigned as caldav_uid ✓ (2) CalDAV-only NEEDS-ACTION: Uuid::new_v4() pre-assigned as tw_uuid ✓ (3) CANCELLED/COMPLETED: tw_uuid=None ✓ (4) calendar_url resolved via resolve_calendar_url() in both TW-only branches at construction ✓ (5) UnmappedProject warning when calendar_url is None ✓ (6) Orphaned: uid.clone() preserved ✓ (7) RRULE: RecurringCalDavSkipped warning + skipped from caldav_map ✓ (8) All dirty_*/cyclic false at construction ✓ (9) 10 tests covering all classification paths ✓. task-3-4 ACs: (1) resolved_depends contains CalDAV UID strings ✓ (2) cyclic=true + CyclicEntry warning per cyclic node ✓ (3) UnresolvableDependency for not-in-IR and no-caldav-uid cases ✓ (4) HashMap index ✓ (5) 5 tests cover simple deps, mutual cycle, not-in-IR, no-caldav-uid, no-deps ✓.

## Deviations

- **[MEDIUM]** Compilation and test execution not verified: cargo was unavailable in the implementation environment. Code was reviewed manually but not compiled or run.
  - Justification: Environment constraint acknowledged in the journal. Code is type-consistent and uses correct import paths based on static review.
- **[LOW]** CalDAV-only entries with unrecognised status (not COMPLETED/CANCELLED/NEEDS-ACTION) also receive a fresh tw_uuid. The spec AC mentions only NEEDS-ACTION explicitly for fresh UUID assignment.
  - Justification: The implementation extends fresh-UUID assignment to IN-PROCESS and any unrecognised status, which is documented in the doc comment. This is a safe superset behaviour — treating unknown active statuses like NEEDS-ACTION is more conservative than leaving them without a tw_uuid.
- **[LOW]** In deps.rs cycle detection, `cyclic_nodes[node] = true` is set redundantly after the slice marking already includes node (node is the last stack element).
  - Justification: The redundancy is idempotent and does not affect correctness. Minor code smell only.
- **[LOW]** `TwCalDavFields.description` is typed as `Option<String>` but is always Some in `tw_to_caldav_fields()`, making None unreachable in practice.
  - Justification: No functional impact; the Option wrapper may serve as a future extension point. The AC does not mandate a non-Option type.

## Test Coverage
**Status:** sufficient

task-3-1: 8 tests covering all 5 TW status branches plus fallback and the wait-absent fallback. task-3-2: 16 tests (TW→CalDAV: description, due, dtstart, future wait, expired wait, no wait, depends, none-fields; CalDAV→TW: description, absent description, due, dtstart, wait parse, depends round-trip, none-fields; plus a full TW→CalDAV→TW round-trip for depends). task-3-3: 10 tests covering TW-only-new, paired, orphaned, CalDAV-only NEEDS-ACTION, CalDAV-only COMPLETED, CalDAV-only CANCELLED, RRULE skip, unmapped project, project-to-calendar mapping, and dirty/cyclic init. task-3-4: 5 tests covering simple resolution, mutual cycle, not-in-IR, no-caldav-uid, and no-dependency baseline. All tests are well-structured and verify both positive and negative/edge cases. Confirmation of pass requires `cargo test` in a working environment.

## Code Quality

Overall code quality is high: idiomatic Rust, clear doc comments, proper use of serde attributes with #[serde(default)] for backward-compatible IREntry extension, iterative DFS avoids stack overflow on large dependency graphs, and all public APIs are documented. The RRULE skip is handled at the earliest possible point (caldav_map construction) which is efficient.

- parse_ical_datetime_utc in fields.rs handles only UTC (Z-suffix) iCal datetime strings; timezone-aware DTSTART/DUE with TZID parameters would silently return None.
- redundant `cyclic_nodes[node] = true` assignment at line 113 of deps.rs (see deviations).
- tw_to_caldav_fields uses Utc::now() inline for the expired-wait check, making the function non-deterministic and harder to unit-test with fixed timestamps at boundary conditions.

## Documentation
**Status:** adequate

All four files have module-level doc comments. TwToCalDavStatus variants are individually documented, especially TwStateDeleted's dispatch semantics. build_ir() has a thorough doc comment covering all three-way classification rules, CalDAV-only rules, and the calendar_url contract. resolve_dependencies() is clearly documented with numbered steps. The expired-wait collapse logic references Phase 0 finding #6 inline, providing good traceability.

## Issues

- Compilation and test execution unverified due to missing cargo in implementation environment — must be confirmed in verify step.
- parse_ical_datetime_utc does not handle TZID-parameterised iCal datetimes (out-of-scope for this phase but worth noting as a future gap).
- Expired-wait check uses live Utc::now() making the function non-deterministic; consider injecting a reference timestamp for deterministic testing.

## Recommendations

- Run `cargo build` and `cargo test` in a proper Rust environment to confirm compilation and all 39+ tests pass before closing Phase 2.
- Add a test for the IN-PROCESS CalDAV-only case to explicitly document the deliberate extension of fresh-UUID assignment beyond NEEDS-ACTION.
- Consider injecting a `now: DateTime<Utc>` parameter into `tw_to_caldav_fields` or wrapping the expired-wait check in a testable helper to enable deterministic boundary testing.
- Track the TZID-aware datetime parsing gap as a known limitation in the project README or a follow-up issue, since CalDAV servers may emit local-timezone datetimes.

---
*Generated by Foundry MCP Fidelity Review*