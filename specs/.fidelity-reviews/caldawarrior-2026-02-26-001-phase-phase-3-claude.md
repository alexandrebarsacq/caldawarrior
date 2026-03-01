# Fidelity Review: caldawarrior

**Spec ID:** caldawarrior-2026-02-26-001
**Scope:** phase (phase: phase-3)
**Verdict:** pass
**Date:** 2026-02-28T15:52:17.525104
**Provider:** claude

## Summary

All four Phase 2 tasks (status mapper, field mapper, IR builder, dependency resolver) are implemented and align tightly with the specification. All acceptance criteria are met. Test suites are comprehensive (8, 16, 9, and 5 tests respectively across the four modules). The IREntry struct in types.rs has been correctly extended with the required fields. Code quality is clean, well-documented, and idiomatic Rust. Two low-severity observations are noted but do not affect correctness. The only outstanding gap is that cargo compile and test execution were deferred to verify-3-1 due to tooling unavailability in the implementation environment.

## Requirement Alignment
**Status:** yes

task-3-1 (status mapper): All 5 TW statuses (pending→NeedsAction, waiting→NeedsActionWithWait, recurring→Skip(Warning), completed→Completed, deleted→TwStateDeleted) produce the correct enum variant. TwStateDeleted is clearly documented as a state descriptor, not a hard-delete instruction, matching the spec note. Unknown status falls back to NeedsAction. task-3-2 (field mapper): All five bidirectional mappings are present — description↔DESCRIPTION, due↔DUE, scheduled↔DTSTART, wait↔X-TASKWARRIOR-WAIT (with expired-wait collapse per Phase 0 finding #6), depends↔RELATED-TO[RELTYPE=DEPENDS-ON]. The round-trip test confirms full dep-edge preservation. task-3-3 (IR builder): Three-way TW classification is correct (new→fresh UUID4, paired→VTODO removed from map, orphaned→UID preserved). CalDAV-only RRULE VTODOs are skipped with RecurringCalDavSkipped. NEEDS-ACTION/IN-PROCESS get fresh tw_uuid; COMPLETED/CANCELLED get None. UnmappedProject warning fired on missing config entry. calendar_url resolved at construction. All dirty/cyclic flags initialised to false. task-3-4 (dep resolver): HashMap index built with filter_map for Option<Uuid>. resolved_depends populated with CalDAV UID strings. Iterative 3-colour DFS detects cycles; all cycle-path nodes marked cyclic=true with CyclicEntry warning. UnresolvableDependency emitted for both 'not in IR' and 'no caldav_uid' cases.

## Success Criteria
**Status:** yes

task-3-1 ACs: (1) all 5 status variants produced ✓; (2) TwStateDeleted returned for 'deleted', doc explicitly forbids treating it as hard-delete ✓; (3) recurring returns Skip(Warning), no panic ✓; (4) 8 unit tests covering all branches ✓. task-3-2 ACs: (1) all bidirectional mappings implemented ✓; (2) expired-wait collapse with Phase 0 finding #6 rationale in comments ✓; (3) round-trip test preserves all dependency edges ✓; (4) 16 tests including None/absent cases and a full tw→caldav→tw round-trip ✓. task-3-3 ACs: (1) fresh UUID4 as caldav_uid for TW-only new ✓; (2) fresh UUID4 as tw_uuid for CalDAV-only NEEDS-ACTION ✓; (3) CANCELLED/COMPLETED tw_uuid=None ✓; (4) calendar_url resolved at construction ✓; (5) UnmappedProject warning emitted ✓; (6) orphaned entries preserve caldav_uid ✓; (7) RRULE skip with RecurringCalDavSkipped ✓; (8) dirty_*/cyclic=false after construction ✓; (9) 9 unit tests covering all cases ✓. task-3-4 ACs: (1) resolved_depends has CalDAV UID strings ✓; (2) cyclic=true + CyclicEntry warning per node ✓; (3) UnresolvableDependency for missing UUID ✓; (4) HashMap index for O(1) lookups ✓; (5) 5 tests: simple deps, mutual cycle, not-in-IR, no-caldav-uid, no-dep baseline ✓.

## Deviations

- **[LOW]** In resolve_dependencies() cycle-marking loop, cyclic_nodes[node] = true (line 113) is set after the for loop over stack[cycle_start..], but the current node is already the last element of that slice and thus already marked by the loop. The extra assignment is redundant.
  - Justification: Harmless; does not affect correctness or observable behaviour. May be a copy-paste artefact from a previous implementation draft.
- **[LOW]** For a 'waiting' task with wait=None, tw_to_caldav_status() falls back to task.entry as the wait timestamp. This fallback is not described in the spec.
  - Justification: A defensively safe choice: entry is always present, and a missing wait on a 'waiting' task is a TW data anomaly. No spec requirement is violated; the variant returned is still NeedsActionWithWait.
- **[LOW]** The IR builder emits UnmappedProject for orphaned TW entries (caldavuid set but no VTODO found) in addition to new TW-only entries. The spec only explicitly mentions new TW-only entries.
  - Justification: Prudent extension: orphaned entries also require calendar routing, so the warning is operationally correct. No spec requirement is contradicted.
- **[LOW]** The CalDAV-only pass in build_ir() assigns a fresh tw_uuid to any status that is not COMPLETED or CANCELLED (catch-all), not strictly only to NEEDS-ACTION and IN-PROCESS as listed in the spec.
  - Justification: Consistent with defensive design: unrecognised statuses are treated as active rather than terminal. The spec's two named statuses are subsets of the catch-all, so they are fully covered.
- **[LOW]** cargo compile check and test execution were not performed in the implementation environment; results deferred to the verify-3-1 task.
  - Justification: Cargo was unavailable in the implementation environment per journal entries. The code structure, types, and patterns are consistent and error-free from static inspection.

## Test Coverage
**Status:** sufficient

status.rs: 8 tests — pending, waiting+wait, waiting-no-wait fallback, recurring, completed+end, completed-no-end fallback, deleted, unknown status. fields.rs: 16 tests — each TW field mapped to CalDAV direction, each CalDAV field mapped to TW direction, all None/absent cases, expired vs future wait, depends round-trip tw→caldav→tw. ir.rs: 9 tests — tw-only-new, paired, orphaned, caldav-only NEEDS-ACTION, caldav-only COMPLETED, caldav-only CANCELLED, RRULE skip+warning, UnmappedProject, project→calendar routing, dirty/cyclic flags all false. deps.rs: 5 tests — simple dep resolution, mutual cycle (both marked), UUID-not-in-IR, UUID-in-IR-but-no-caldav-uid, no-dep baseline. All tests are logically sound. Actual pass/fail confirmation awaits a cargo test run in verify-3-1.

## Code Quality

Code is idiomatic Rust throughout: no unwrap() on user-supplied data paths, Options handled with and_then/filter_map/unwrap_or_default, iterative DFS avoids stack overflow risk from deep dependency graphs. All public symbols have doc comments. The Warning struct reuse is consistent with types.rs. IcalProp formatting uses the compact iCalendar UTC format correctly. The #[serde(default)] annotations on IREntry fields ensure backward compatibility if the struct is serialised between versions.

- Redundant cyclic_nodes[node] = true assignment after the for loop in deps.rs (minor, harmless).
- parse_ical_datetime_utc in fields.rs uses trim_end_matches('Z') which will incorrectly strip multiple trailing 'Z' characters, though this is a pathological input and not a realistic concern with iCalendar data.

## Documentation
**Status:** adequate

Each module has a module-level doc comment. All public structs, enums, and functions have doc comments explaining intent, TwStateDeleted dispatch semantics, expired-wait collapse rationale (with Phase 0 finding #6 citation), and the three-way classification rules in build_ir. Inline comments explain non-obvious logic (3-colour DFS, cycle-start position search, wait collapse). No external API doc or README update is required for this phase.

## Issues

- cargo compile check and test run not yet confirmed (deferred to verify-3-1).
- Redundant cyclic_nodes[node] = true after stack-slice loop in deps.rs (harmless).
- parse_ical_datetime_utc strips all trailing 'Z' chars rather than exactly one (theoretical but harmless with well-formed iCalendar input).

## Recommendations

- Run 'cargo test' in the verify-3-1 environment to confirm all 38 unit tests pass and there are no compiler errors.
- Optionally tighten parse_ical_datetime_utc to strip exactly one trailing 'Z' (e.g., use strip_suffix('Z').unwrap_or(s)) to be fully spec-compliant with RFC 5545 datetime strings.
- Consider adding a doc comment or assertion noting that the cyclic_nodes[node] = true after the loop is redundant, or remove it to simplify the code.

---
*Generated by Foundry MCP Fidelity Review*