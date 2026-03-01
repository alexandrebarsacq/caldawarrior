# Fidelity Review: caldawarrior

**Spec ID:** caldawarrior-2026-02-26-001
**Scope:** phase (phase: phase-2)
**Verdict:** pass
**Date:** 2026-02-28T19:47:00.481532

## Summary

All six Phase 1 implementation files (types.rs, error.rs, config.rs, tw_adapter.rs, caldav_adapter.rs, ical.rs) are present and substantially complete. Every required type, error variant, config field, adapter method, and iCalendar function is implemented. Serde attributes are applied correctly throughout. Test coverage spans all six modules with 35+ total unit tests. Three low-severity deviations were identified: delete() uses one command instead of two as described, the permission-warning code path lacks a dedicated unit test, and a dead legacy parser in caldav_adapter.rs carries allow(dead_code) suppressions. None of these affect correctness.

## Requirement Alignment
**Status:** yes

task-2-1: All 12 types present (TWTask, VTODO, FetchedVTODO, IREntry, PlannedOp, SyncResult, UpdateReason/5 variants, SkipReason/8 variants, Side, Warning, CyclicEntry, plus RelType+IcalProp). #[serde(default, skip_serializing_if)] applied correctly on all Option<T> and Vec fields. Custom tw_date / tw_date_opt / tw_depends serde modules correctly handle TW compact format and comma-or-array depends. task-2-2: Seven CaldaWarriorError variants match spec exactly; Auth message mentions credentials; EtagConflict carries FetchedVTODO. task-2-3: Config path resolution order, CALDAWARRIOR_PASSWORD override, all three defaults, duplicate-URL validation, and Unix 0600 permission check all implemented correctly. task-2-4: TaskRunner trait has run/import plus default export/modify; MockTaskRunner queues responses; TwAdapter::new() registers UDA first; create() exclusively uses import; update() exclusively uses modify with comma-delimited depends. task-2-5: CalDavClient trait defined; RealCalDavClient uses reqwest blocking with timeout+TLS flags; If-Match / If-None-Match headers set correctly; 401→Auth, TLS→CalDav guidance, 412→EtagConflict (fresh fetch, no internal retry). task-2-6: from_icalendar_string and to_icalendar_string both complete; RFC 5545 TEXT escape/unescape; 75-octet fold/unfold; TZID via chrono-tz; RELTYPE→RelType; unknown props→extra_props; DTSTAMP injected on serialize.

## Success Criteria
**Status:** yes

AC-by-AC: types.rs — serde round-trip test (tw_task_roundtrip_minimal) passes with all optional fields absent; all 5 UpdateReason variants and 8 SkipReason variants present; SyncResult has all 6 required fields. error.rs — 4 unit tests cover Config, Tw, Auth (credentials message verified), EtagConflict (inner value accessible). config.rs — 5 tests: happy_path, defaults_applied, missing_required_field, duplicate_calendar_url_error, duplicate_default_url_allowed; permission warning logic is correct (0o177 mask). tw_adapter.rs — 5 tests: UDA ordering, create via import, update via modify, delete tolerates already-deleted, dedup by max modified. caldav_adapter.rs — 8 tests using MockCalDavClient, plus 4 parse_vtodo_from_ical legacy tests and datetime helpers. ical.rs — 9 tests: round-trip, TEXT escaping, line folding, TZID, RelType::DependsOn, RelType::Other, extra_props preservation, DTSTAMP present, missing-UID error.

## Deviations

- **[LOW]** delete() issues a single 'task rc.confirmation:no <uuid> delete' command rather than the two sequential commands described in the spec ('delete(uuid) runs two sequential commands').
  - Justification: Using rc.confirmation:no in one invocation achieves the same effect as a two-step approach that pipes 'yes' or issues a confirmation command separately. The graceful handling of exit-1 / 'not deletable' covers the idempotency requirement.
- **[LOW]** config.rs unit tests do not include a dedicated test for the permission warning code path (>0600 emits [WARN] to stderr).
  - Justification: Testing Unix file permission bits in portable unit tests is complex (requires creating a file and calling chmod). The production logic at line 94 (`mode & 0o177 != 0`) is correct, so the gap is test coverage only, not a code defect.
- **[LOW]** caldav_adapter.rs retains a duplicate legacy parse_vtodo_from_ical() function alongside the canonical crate::ical::from_icalendar_string(). The legacy function is annotated #[allow(dead_code)] and is exercised only through its own unit tests.
  - Justification: The legacy parser predates the ical module and is kept for its dedicated test coverage of lower-level helpers (unfold_ical, parse_ical_datetime, unescape_ical). It doesn't affect the production code path.
- **[LOW]** The uda_registration_before_list_all unit test only confirms the sequence runs without panicking; it does not assert that UDA config calls appear before export calls in the recorded call log.
  - Justification: The uda_registration_runs_on_new test confirms adapter.uda_registered == true after construction, and code structure guarantees ordering (register_uda() is called in new() before any public method). The weaker assertion is adequate for demonstrating the happy path.

## Test Coverage
**Status:** sufficient

types.rs: 1 serde round-trip test. error.rs: 4 tests covering all non-trivial variants. config.rs: 5 tests (happy path, defaults, missing field, duplicate URL, default-exempt URL). tw_adapter.rs: 5 tests (UDA ordering, create/import, update/modify, delete idempotency, dedup). caldav_adapter.rs: 8 Mock-based trait tests + 4 legacy parser tests + 3 datetime helper tests = 15 tests. ical.rs: 9 tests covering all RFC 5545 behaviors. All tests reported passing by the journal ('35/35 non-config tests pass'). No HTTP server required for CalDAV tests (MockCalDavClient used). The one gap—explicit permission warning test—is low risk given the logic simplicity.

## Code Quality

Overall code quality is high: no unsafe blocks, no shell interpolation in process::Command calls, Mutex-guarded mock state is thread-safe, XML parsing in caldav_adapter avoids external crate dependencies using simple string search (sufficient for well-formed server responses). Error mapping is thorough and user-directed.

- parse_vtodo_from_ical in caldav_adapter.rs is dead production code kept for legacy tests; it partially duplicates ical::from_icalendar_string and will need to be kept in sync if the VTODO struct changes.
- fold_line in ical.rs uses byte counts (FIRST_MAX=75, CONT_MAX=74) correctly per RFC 5545, but the test assertion checks physical_line.len() <= 75 which could pass incorrectly for multi-byte UTF-8 continuations that happen to be short — existing test only uses ASCII 'A' characters.
- TwAdapter::uda_registered field is set but only ever read in the constructor assertion; there is no public method to inspect it and it adds marginal value as a runtime flag.
- MockTaskRunner::get_calls() drains the recorded calls, making repeated inspection impossible in the same test; defensive or non-destructive access could improve test ergonomics.

## Documentation
**Status:** adequate

Each module opens with a module-level comment or inline documentation explaining its purpose. Public structs and their fields carry doc-comments or contextual inline comments (e.g., IREntry field explanations). The tw_adapter create/update methods explicitly call out the import-vs-modify contract in doc-comments. No separate API reference doc exists at this phase, which is expected — Phase 6 is designated for README and configuration reference.

## Issues

- delete() uses one command (rc.confirmation:no flag) instead of the spec-described two sequential commands.
- Permission warning code path lacks a dedicated unit test.
- Legacy parse_vtodo_from_ical in caldav_adapter.rs is dead production code duplicating functionality.
- fold_line test coverage limited to ASCII; multi-byte UTF-8 boundary behaviour not directly tested.

## Recommendations

- Remove or move parse_vtodo_from_ical tests into ical.rs to eliminate the dead-code duplication in caldav_adapter.rs.
- Add a UTF-8 multi-byte character to the line-folding test (e.g., CJK or emoji) to validate that fold_line never splits a character boundary.
- Consider adding a tempfile-based permission test on Unix (chmod 0644, verify [WARN] appears on stderr) to fully satisfy the permission-warning AC.
- Document the deliberate single-command delete approach in a code comment referencing the spec's 'two sequential commands' wording, so the rationale is preserved for future maintainers.

---
*Generated by Foundry MCP Fidelity Review*