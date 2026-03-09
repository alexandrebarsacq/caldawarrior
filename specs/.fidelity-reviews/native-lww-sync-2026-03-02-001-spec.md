# Fidelity Review: native-lww-sync

**Spec ID:** native-lww-sync-2026-03-02-001
**Scope:** spec
**Verdict:** pass
**Date:** 2026-03-02T21:32:25.322485

## Summary

The native-lww-sync implementation fully satisfies all six success criteria. All four phases completed cleanly: get_last_sync() and LAST_SYNC_PROP are eliminated from lww.rs; the Layer 1 decision tree matches the spec (caldav_ts > tw_modified → CalDAV wins, otherwise TW wins, with None caldav_ts → TW wins); LAST-MODIFIED is written as tw.modified.unwrap_or(tw.entry) in build_vtodo_from_tw(); ical.rs required no changes (no dedicated LAST-SYNC handling existed); grep -r 'X-CALDAWARRIOR-LAST-SYNC' src/ returns zero results; cargo test yields 148 passed / 0 failed; all 19 previously-passing Robot Framework tests continue to pass with no regressions. The 7 RF failures and 5 skips are a confirmed pre-existing baseline, explicitly out of scope.

## Requirement Alignment
**Status:** yes

Every stated requirement maps to a completed journal entry with evidence:
- get_last_sync() / LAST_SYNC_PROP removed from lww.rs (task-1-1).
- resolve_lww() Layer 1 now compares tw.modified.unwrap_or(tw.entry) vs vtodo.last_modified.or(vtodo.dtstamp); spec rule 'TW >= LAST-MODIFIED → TW wins' is implemented as 'caldav_ts > tw_modified → CalDAV wins, otherwise TW wins' — logically equivalent (strict greater-than for CalDAV, TW wins on equal or newer).
- Layer 2 content check confirmed to fire before Layer 1 (identical_content_skips_even_when_tw_newer test).
- X-CALDAWARRIOR-LAST-SYNC write block removed from build_vtodo_from_tw().
- LAST-MODIFIED now written as tw.modified.unwrap_or(tw.entry).
- ical.rs confirmed zero dedicated LAST-SYNC handling.
- Public API of run_sync() and caller interfaces unchanged.
- caldavuid UDA mechanism untouched.
- No new external dependencies.
- Rust edition 2024 constraint honoured.

## Success Criteria
**Status:** yes

All six success criteria verified:
1. grep -r 'X-CALDAWARRIOR-LAST-SYNC' src/ → zero results (Phase 2 verify + Phase 3 verify).
2. VTODO LAST-MODIFIED = tw.modified.unwrap_or(tw.entry) on every CalDAV write (writeback.rs task-2-1).
3. LWW uses TW.modified vs CalDAV LAST-MODIFIED; TW.modified >= LAST-MODIFIED → TW wins (task-1-2, verify-1-1).
4. Content-identical pairs skipped before LWW via Layer 2 (confirmed by test and regression test verify-1-1).
5. All 19 previously-passing RF tests still pass; 7 pre-existing failures unchanged (verify-4-2).
6. cargo test: 148 passed, 0 failed (verify-4-1).

## Deviations

- **[LOW]** writeback.rs test helper make_paired_entry was updated to use an inlined string literal rather than the removed LAST_SYNC_PROP constant, to preserve legacy fixture simulation fidelity.
  - Justification: This is an internal test-scaffolding detail. The production path is clean; the literal exists only to exercise the extra_props parsing path in tests. No functional deviation from spec intent.

## Test Coverage
**Status:** sufficient

Unit test coverage is comprehensive:
- lww.rs: 10 tests covering all 7 spec-defined LWW scenarios (equal timestamps, TW newer, CalDAV newer, None caldav_ts, None tw.modified fallback to entry, identical content skips, regression resync after CalDAV win).
- writeback.rs: 14 tests updated; 7 call sites verified.
- Integration: 18 integration tests + 4 main tests pass.
- Blackbox: 19 RF tests (the full previously-passing set) confirmed green with no regressions.
- grep coverage used to verify no stray references remain in src/ and tests/robot/.

## Code Quality

No quality concerns identified. The stale doc comment on resolve_lww() was updated to reflect the new algorithm. The test function tw_wins_when_modified_after_last_sync was renamed to tw_wins_when_modified_is_newer for consistency. Dead code (LAST_SYNC_PROP constant, get_last_sync() function, X-CALDAWARRIOR-LAST-SYNC write block) was fully removed from production paths. The extra_props filter in build_vtodo_from_tw() was simplified to strip only X-TASKWARRIOR-WAIT, removing the now-unnecessary LAST-SYNC strip arm.


## Documentation
**Status:** adequate

The resolve_lww() doc comment was updated to remove the outdated description of X-CALDAWARRIOR-LAST-SYNC behaviour and reflect the native LWW algorithm. Journal entries provide a detailed implementation audit trail across all four phases. No additional inline documentation gaps identified.

## Recommendations

- Consider a brief CHANGELOG or commit message entry summarising the migration from X-CALDAWARRIOR-LAST-SYNC to native LWW, so future contributors understand the historical intent without reading the full spec.
- The 7 pre-existing RF failures (e.g. S-62 CalDAV VTODO Summary Syncs To TW Description) remain open. Tracking them in a separate spec or issue would prevent them from being conflated with regressions in future review cycles.
- Monitor Radicale's LAST-MODIFIED override behaviour (noted as a low-likelihood risk) in production; if it manifests, the Layer 2 content check is the sole safety net and any gaps in the 8-field content comparison would need to be addressed.

---
*Generated by Foundry MCP Fidelity Review*