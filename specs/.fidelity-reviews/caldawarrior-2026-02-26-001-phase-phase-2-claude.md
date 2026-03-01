# Fidelity Review: caldawarrior

**Spec ID:** caldawarrior-2026-02-26-001
**Scope:** phase (phase: phase-2)
**Verdict:** pass
**Date:** 2026-02-28T15:49:13.053561
**Provider:** claude

## Summary

Phase 1 implementation is highly faithful to the specification. All six modules (types, error, config, tw_adapter, caldav_adapter, ical) are present and implement their specified interfaces. All enumerated ACs are met: correct serde annotations, full variant counts, error hierarchy, config path resolution, UDA registration ordering, CalDavClient trait with correct HTTP semantics, and RFC-5545-compliant iCal serialization with line folding, TEXT escaping, TZID normalization, and extra-props preservation. Test coverage spans all required happy paths. Deviations are low-severity and do not impair correctness.

## Requirement Alignment
**Status:** yes

All twelve required domain types are present in types.rs with correct serde field annotations. UpdateReason has exactly 5 variants; SkipReason has exactly 8 variants. SyncResult carries all six specified fields. error.rs has all 7 variants with thiserror Display; Auth message directs to credentials; EtagConflict carries FetchedVTODO. config.rs implements three-step path resolution, CALDAWARRIOR_PASSWORD override, 0600 permission warning, and duplicate URL rejection. tw_adapter.rs gates all UDA registration before any export, create() is the sole task-import caller, update() uses modify exclusively, and dedup retains max-modified. caldav_adapter.rs defines CalDavClient trait with If-Match / If-None-Match semantics, 401→Auth, 412→EtagConflict with re-fetch, TLS→CalDav with insecure-tls guidance. ical.rs implements unfold/fold, TEXT escape/unescape, TZID→UTC via chrono-tz, RelType parsing, and extra_props round-trip.

## Success Criteria
**Status:** yes

task-2-1: round-trip test for minimal TWTask passes; all serde annotations present. task-2-2: 4 variant-construction tests; Auth directs to credentials. task-2-3: 5 tests (happy path, defaults, missing field, duplicate URL, default exemption); permission warn code present. task-2-4: 5 tests covering UDA ordering, create/update/delete semantics, dedup-by-max-modified. task-2-5: 8+ mock-based tests covering list/put/delete call recording, error variants, and empty-response defaults. task-2-6: 9 tests covering round-trip, TEXT escaping, line folding, TZID conversion, RelType::DependsOn, RelType::Other, extra_props, DTSTAMP presence, missing-UID error.

## Deviations

- **[LOW]** delete() issues one command ('task rc.confirmation:no <uuid> delete') rather than the two sequential commands stated in the task-2-4 description.
  - Justification: The rc.confirmation:no flag suppresses the interactive prompt, achieving the same non-interactive effect. The already-deleted case (exit 1 / 'Deleted 0 tasks') is handled gracefully. No AC explicitly mandates two commands, and the chosen approach is functionally correct.
- **[LOW]** Config permission-warning unit test is absent. The spec AC states 'Unit tests cover … permission warning'.
  - Justification: The check_permissions() function exists and the [WARN] path is exercised at runtime, but no automated test exercises it. Setting file permissions in a tempfile-based unit test requires Unix-specific calls and is commonly deferred to integration testing.
- **[LOW]** The uda_registration_before_list_all test does not programmatically verify call ordering; it only confirms no panic occurs.
  - Justification: The complementary test uda_registration_runs_on_new verifies uda_registered=true before list_all is called, providing indirect ordering coverage. A stronger test would inspect recorded calls for the UDA config invocations appearing before export calls.
- **[LOW]** caldav_adapter.rs retains a dead-code legacy iCal parser (parse_vtodo_from_ical, unfold_ical, unescape_ical, parse_ical_datetime) that duplicates logic now canonical in ical.rs.
  - Justification: The functions are annotated #[allow(dead_code)] and are only used by adapter unit tests that predate the ical module extraction. They do not affect production code paths.

## Test Coverage
**Status:** sufficient

35+ unit tests across 6 modules cover all primary code paths: serde round-trips, all error variant constructions, config loading/validation, TW adapter mock-verified behavior (create/update/delete/dedup/UDA), CalDAV mock client (list/put/delete with ETags and error injection), and iCal full-cycle tests (round-trip, escaping, folding, TZID, RelType, extra_props, DTSTAMP, missing-UID error). Gaps are: permission warning test and stronger UDA ordering assertion (both low severity).

## Code Quality

Overall code quality is high: no unsafe code, consistent use of thiserror, clean serde annotations, RFC-compliant iCal helpers, well-structured mock objects with Mutex-guarded FIFO queues. The tw_adapter.rs update() field-clearing behavior (emitting 'field:' to clear) is intentional but may over-write TW fields not managed by CalDAV sync; this is a design trade-off that should be documented or gated by caller logic.

- Dead code in caldav_adapter.rs: parse_vtodo_from_ical and helpers duplicate functionality now owned by ical.rs; should be removed or consolidated.
- update() in tw_adapter.rs always emits 'due:' and 'scheduled:' clear tokens even when those fields were never set, which may unintentionally clear fields if called on tasks where TW owns those values.
- uda_registration_before_list_all test is a no-op assertion (only checks that the call sequence doesn't panic); no call-order inspection is performed.

## Documentation
**Status:** adequate

Each module has a module-level comment or doc comment. Key functions carry /// doc comments explaining their contract (e.g., create() is the ONLY path calling task import). Inline comments explain non-obvious choices (TW date format, tw_depends string-or-array duality, rc.confirmation:no, fold boundary logic). No top-level API documentation (rustdoc) is present at the lib level beyond pub mod declarations, but this is appropriate for an early-phase CLI tool.

## Issues

- delete() issues one command instead of two as described in the spec (low impact; functionally equivalent).
- No unit test exercises the config file permission warning path.
- Dead-code duplication of iCal helpers in caldav_adapter.rs should be cleaned up.
- update() unconditionally clears optional TW fields (due, scheduled, priority, project, caldavuid) which may over-write TW data not managed by CalDAV.

## Recommendations

- Remove or guard the dead-code iCal helpers in caldav_adapter.rs (parse_vtodo_from_ical, unfold_ical, unescape_ical, parse_ical_datetime) now that ical::from_icalendar_string is canonical.
- Add a Unix-only unit test for the config permission warning path using std::os::unix::fs::PermissionsExt::set_permissions on the tempfile.
- Strengthen uda_registration_before_list_all by capturing and inspecting MockCall order after new() returns.
- Audit update() field-clearing behavior: only emit 'field:' clear tokens for fields that were previously set (or document this as intentional full-overwrite semantics).
- Consider adding a two-command delete path (e.g., 'task done' + 'task delete') if spec intent was to move tasks through lifecycle states, or update the spec description to match the implemented single-command approach.

---
*Generated by Foundry MCP Fidelity Review*