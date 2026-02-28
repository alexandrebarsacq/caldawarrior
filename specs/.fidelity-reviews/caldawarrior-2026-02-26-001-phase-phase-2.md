# Fidelity Review: caldawarrior

**Spec ID:** caldawarrior-2026-02-26-001
**Scope:** phase (phase: phase-2)
**Verdict:** partial
**Date:** 2026-02-26T16:01:17.806553

## Summary

Phase 1 is substantially complete and well-structured. All six modules exist, types and error variants match the spec, serde attributes are correctly applied to TWTask, the config resolution/validation logic is correct, and the iCalendar layer (ical.rs) is a solid RFC 5545 implementation with all 9 required unit tests passing. However, one high-severity and several medium-severity deviations prevent a full pass. The critical issue is that caldav_adapter.rs contains its own internal `parse_vtodo_from_ical()` that silently drops RELATED-TO (depends) and extra_props for every VTODO fetched from the CalDAV server — while ical.rs has the full-featured parser, the adapter never delegates to it. This will break dependency sync in Phase 2+. Additional medium issues include: (1) the TaskRunner trait methods are named `run/import` instead of `export/import/modify` as specified; (2) `delete()` runs only one command where the spec prescribes two; (3) `apply_writeback()` and `run_sync()` are absent, though these belong in Phase 3; and (4) the `icalendar = 0.16` crate in Cargo.toml is unused, since both parsing modules were written from scratch.

## Requirement Alignment
**Status:** partial

task-2-1 (types.rs): All 11 domain types present. UpdateReason has exactly 5 variants, SkipReason exactly 8 with the 8-field Identical comment. SyncResult fields match spec. Option<T> serde attributes are correct on TWTask but VTODO's Option fields (summary, description, status, last_modified, dtstart, due, completed, rrule) lack #[serde(default)] and #[serde(skip_serializing_if='Option::is_none')]; Vec<Uuid> depends carries correct attributes. Serde round-trip test present. RelType and IcalProp added as required by task-2-6 downstream. | task-2-2 (error.rs): All 7 variants correct. Auth message includes 'check your credentials'. EtagConflict carries FetchedVTODO. 4 unit tests all passing. | task-2-3 (config.rs): Path resolution order correct (arg → env → default). CALDAWARRIOR_PASSWORD override works. Defaults 90/false/30 confirmed. Duplicate-URL check excludes 'default'. Permission check emits [WARN] non-fatally. Tests cover happy path, defaults, missing fields, duplicate URL, and default-exemption. Missing: no dedicated test exercises the permission-warning branch. | task-2-4 (tw_adapter.rs): UDA registration runs in new() before any export/import. list_all() two-call merge with max-modified deduplication works correctly. create() is the sole task import caller. update() uses task modify exclusively with depends as comma-separated UUIDs. However, the TaskRunner trait exposes run()/import() instead of spec-prescribed export()/import()/modify(), and delete() runs only one command (spec says two sequential commands). | task-2-5 (caldav_adapter.rs): CalDavClient trait defined with list_vtodos/put_vtodo/delete_vtodo. RealCalDavClient and MockCalDavClient present. If-Match/If-None-Match conditional headers correct. 401→Auth, TLS→CalDav-with-hint, 412→EtagConflict-with-refetch all implemented. reqwest timeout/TLS config correct. However: (a) apply_writeback() and run_sync() are absent (these are Phase 3 items but the AC lists them under task-2-5); (b) the adapter's internal parse_vtodo_from_ical() always returns depends:[] and extra_props:[], losing all RELATED-TO relationships from fetched VTODOs. | task-2-6 (ical.rs): All ACs satisfied. DTSTAMP injected, VCALENDAR wrapper, RFC 5545 TEXT escaping (SUMMARY+DESCRIPTION), 75-octet byte-boundary folding, TZID via chrono-tz, RelType::DependsOn/Other, extra_props round-trip, 9 unit tests.

## Success Criteria
**Status:** partial

Passing: TWTask serde round-trip (task-2-1). Error variant construction (task-2-2). Config resolution, defaults, duplicate URL, permission warn (task-2-3). UDA ordering, list dedup, create-via-import, update-via-modify, delete tolerance (task-2-4). MockCalDavClient tests, VTODO text parsing, datetime parsing, unescape (task-2-5). ical.rs round-trip, escaping, folding, TZID, RELATED-TO, extra_props, DTSTAMP, missing-UID error (task-2-6). | Not fully met: VTODO Option fields missing serde attrs (task-2-1). Permission-warning test missing (task-2-3). delete() one-command-only (task-2-4). depends/extra_props silently dropped in CalDAV list_vtodos path (task-2-5). apply_writeback/run_sync absent (task-2-5).

## Deviations

- **[HIGH]** caldav_adapter.rs internal parse_vtodo_from_ical() always returns depends:[] and extra_props:[] — the `ical::from_icalendar_string()` full-featured parser is never invoked by list_vtodos() or fetch_single_vtodo(). All RELATED-TO dependency relationships and unknown properties are silently dropped for every VTODO fetched from the CalDAV server.
  - Justification: None evident. The internal parser in caldav_adapter.rs was written before ical.rs existed; it was not updated to delegate to the complete parser once ical.rs was implemented. The fix is to replace the internal call with ical::from_icalendar_string(), handling the Result-vs-Option difference.
- **[MEDIUM]** TaskRunner trait exposes run(&[&str]) and import(&[u8]) instead of the spec-prescribed export(), import(), and modify() methods. export() and modify() are merged into a single generic run() method.
  - Justification: Functionally equivalent since export and modify both reduce to 'run task with args'. MockTaskRunner still satisfies testability requirements. The merged API is arguably cleaner, but it diverges from the spec interface.
- **[MEDIUM]** TwAdapter::delete() runs only one command ('task <uuid> delete'). The spec prescribes 'two sequential commands'. Without rc.confirmation:no or a piped 'yes', TaskWarrior may prompt for interactive confirmation in non-test environments.
  - Justification: The mock test only queues one run_response, confirming the single-call design. The second command (likely 'task rc.confirmation:no' prefix or a separate confirm step) is absent.
- **[MEDIUM]** apply_writeback() and run_sync() functions are not implemented anywhere in Phase 1. The task-2-5 AC explicitly lists 'apply_writeback() and run_sync() accept dyn CalDavClient' as a verification step.
  - Justification: These functions belong to the sync orchestrator (Phase 3). Their absence in Phase 1 is architecturally correct, but the AC for task-2-5 references them, making it impossible to fully verify task-2-5 at this phase.
- **[MEDIUM]** Cargo.toml declares icalendar = '0.16' as a dependency, but no source file imports or uses the crate. All iCalendar parsing and serialization was written from scratch in ical.rs and caldav_adapter.rs.
  - Justification: The crate was presumably included as scaffolding and then replaced by the custom implementation. It should be removed to avoid unnecessary compile time and binary bloat.
- **[LOW]** VTODO struct Option<T> fields (summary, description, status, last_modified, dtstart, due, completed, rrule) lack #[serde(default)] and #[serde(skip_serializing_if='Option::is_none')] attributes. The spec AC applies this requirement to 'All Option<T> fields' across all shared types.
  - Justification: VTODO is primarily serialized via iCalendar text, not JSON, so the absence of serde attributes has limited practical impact today. However, SyncResult.planned_ops and other types serialize VTODOs as JSON for logging/output, making this a correctness gap.
- **[LOW]** No unit test exercises the config file permission-warning branch (check_permissions). The AC requires 'Unit tests cover ... permission warning'.
  - Justification: Testing stderr output and OS-level file permission manipulation is non-trivial in unit tests. The implementation is correct; only the test coverage is missing.
- **[LOW]** uda_registration_before_list_all test does not actually inspect call ordering via MockCall drain — it relies on the test not panicking as an implicit ordering proof.
  - Justification: The test still validates the happy-path flow and would fail if UDA registration were missing entirely. A stronger test would drain calls and assert ['config uda.caldavuid.type ...', 'config uda.caldavuid.label ...'] appear before export calls.

## Test Coverage
**Status:** sufficient

35 non-config tests passing per journal. Per-module coverage is solid: types.rs (1 round-trip), error.rs (4 variant tests), config.rs (5 tests covering all key paths except permission warn), tw_adapter.rs (5 tests: UDA order, dedup, create-import, update-modify, delete-tolerance), caldav_adapter.rs (10 tests: mock queuing, full-field parsing, datetime formats, unescape, error propagation), ical.rs (9 tests: round-trip, text escaping, line folding, TZID, RELATED-TO DependsOn, RELATED-TO Other, extra_props, DTSTAMP, missing-UID error). The only AC-required test not present is a dedicated permission-warning test in config.rs. All listed tests use mocks appropriately and require no external processes or HTTP servers.

## Code Quality

Overall code quality is high: clean Rust idioms, good use of generics for testability (TwAdapter<R: TaskRunner>), proper error mapping, RFC 5545 compliance in ical.rs. The main structural concern is the duplicate parser between caldav_adapter.rs and ical.rs, which defeats the modularity goal and introduces the high-severity data-loss bug.

- caldav_adapter.rs has a duplicate VTODO parser (parse_vtodo_from_ical, ~75 lines) that is a strict subset of ical.rs::from_icalendar_string(). Maintaining two parsers creates divergence risk; the adapter should call ical::from_icalendar_string() and map the Result to Option.
- Cargo.toml includes icalendar = '0.16' which is not imported in any source file; it adds dead weight to the dependency graph.
- tw_adapter.rs update() clears optional fields by emitting 'field:' (empty value) even when the spec does not mention clearing semantics — this is a behavior assumption not validated by tests.
- config.rs check_permissions uses mode & 0o177 != 0 to detect >0600; this correctly catches any world/group bits but silently passes if the file is 0700 (executable owner). A stricter check would be mode & 0o777 != 0o600.
- caldav_adapter.rs string-based XML parsing (no DOM library) is fragile against namespace prefix variations not covered by the prefix list ['D:', 'C:', '']; real-world servers may use different prefixes or attribute ordering in tags.
- tw_adapter.rs MockTaskRunner::get_calls() drains the call list on each invocation, making it impossible to inspect calls without consuming them — this is an unusual API that could surprise test authors.
- No #[derive(Default)] on MockCalDavClient or MockTaskRunner, requiring explicit ::new() calls instead of Default::default().

## Documentation
**Status:** adequate

Module-level and function-level doc comments are present and accurate throughout. Key design decisions are documented inline (e.g., 'This is the ONLY method that calls task import', the two-call merge strategy, RFC 5545 fold boundary logic). The CLAUDE.md workflow instructions are not source documentation but are relevant context. No rustdoc HTML-generation gaps observed. A minor gap: the duplicate parse_vtodo_from_ical in caldav_adapter.rs has no comment explaining why it exists alongside ical::from_icalendar_string().

## Issues

- HIGH: caldav_adapter list_vtodos() uses an internal incomplete VTODO parser that always returns depends=[] and extra_props=[], silently losing all RELATED-TO dependency data for every CalDAV-fetched task.
- MEDIUM: TaskRunner trait API (run/import) diverges from spec's prescribed export/import/modify interface.
- MEDIUM: TwAdapter::delete() runs only one 'task <uuid> delete' command; spec requires two sequential commands (likely needs rc.confirmation:no handling).
- MEDIUM: apply_writeback() and run_sync() accepting dyn CalDavClient are absent from Phase 1 (Phase 3 scope but listed as task-2-5 AC).
- MEDIUM: icalendar = '0.16' crate is declared in Cargo.toml but never used.
- LOW: VTODO Option<T> fields missing #[serde(default, skip_serializing_if)] attributes.
- LOW: No unit test for permission-warning branch in config.rs.
- LOW: uda_registration_before_list_all test does not assert call ordering via inspection.

## Recommendations

- Replace caldav_adapter.rs::parse_vtodo_from_ical() with a call to ical::from_icalendar_string() (map Err→None or propagate). Remove the duplicate internal parser to eliminate data loss and maintenance divergence.
- Resolve the delete() two-command requirement: determine whether the second command is 'task rc.confirmation:no <uuid> delete', a piped 'yes', or another step, and implement accordingly with a matching mock test.
- Remove the icalendar = '0.16' dependency from Cargo.toml since it is unused.
- Add #[serde(default, skip_serializing_if='Option::is_none')] to all Option fields in the VTODO struct to match the spec AC.
- Strengthen uda_registration_before_list_all: drain MockCall list after list_all() and assert the first two calls contain 'uda.caldavuid.type' and 'uda.caldavuid.label' before the export calls.
- Clarify (in a code comment or spec update) whether apply_writeback()/run_sync() are intentionally deferred to Phase 3 or should have stub definitions accepting &dyn CalDavClient in Phase 1.
- Consider adding a test for the permission-warning path using a helper that sets file permissions to 0o644 and captures stderr (or mocks the check).
- Consider renaming the TaskRunner methods to export()/modify() (keeping import()) to match the spec interface, reducing cognitive mismatch between spec and implementation.

---
*Generated by Foundry MCP Fidelity Review*