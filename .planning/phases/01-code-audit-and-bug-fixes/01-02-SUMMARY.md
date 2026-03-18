---
phase: 01-code-audit-and-bug-fixes
plan: 02
subsystem: caldav
tags: [quick-xml, xml-parser, etag, namespace, caldav, webdav]

# Dependency graph
requires: []
provides:
  - "Namespace-aware XML parser using quick-xml NsReader"
  - "ETag normalization function (weak-to-strong, bare-to-quoted)"
  - "Correct If-Match header construction without double-quoting"
affects: [02-expanded-test-coverage, 03-cross-server-compat]

# Tech tracking
tech-stack:
  added: [quick-xml 0.39]
  patterns: [NsReader state machine for XML parsing, ETag normalization at extraction boundary]

key-files:
  created: []
  modified:
    - Cargo.toml
    - src/caldav_adapter.rs

key-decisions:
  - "Used quick-xml NsReader with resolve_event for namespace matching instead of manual prefix handling"
  - "Removed 7 legacy dead-code functions and their tests (parse_vtodo_from_ical, unfold_ical, unescape_ical, parse_ical_datetime, extract_tag_content, find_tag_start, extract_calendar_data)"
  - "ETag normalization applied at extraction boundary (3 sites) rather than at usage boundary, simplifying If-Match construction"

patterns-established:
  - "XML namespace matching: use quick_xml NsReader with DAV_NS/CALDAV_NS constants and local_name().as_ref() byte matching"
  - "ETag normalization: always normalize at extraction point, never at usage point"

requirements-completed: [AUDIT-02, AUDIT-04]

# Metrics
duration: 14min
completed: 2026-03-18
---

# Phase 1 Plan 2: XML Parser and ETag Normalization Summary

**Namespace-aware XML parser using quick-xml NsReader replacing hand-rolled string splitting, with ETag weak-to-strong normalization at all extraction points**

## Performance

- **Duration:** 14 min
- **Started:** 2026-03-18T15:49:26Z
- **Completed:** 2026-03-18T16:03:37Z
- **Tasks:** 1 (TDD: RED + GREEN)
- **Files modified:** 2

## Accomplishments
- Replaced hand-rolled XML parser (hardcoded D:/C:/"" prefixes) with quick-xml NsReader that handles arbitrary namespace prefixes
- Added normalize_etag function that strips W/ weak prefix and ensures double-quote wrapping
- Applied ETag normalization at all 3 extraction points (GET, PUT response, XML parser)
- Simplified If-Match header construction by removing redundant double-quoting
- Removed 7 legacy dead-code functions and 7 associated tests
- Added 11 new tests covering ETag normalization (5) and XML namespace variants (6)

## Task Commits

Each task was committed atomically:

1. **Task 1 (RED): Failing tests** - `ba1d317` (test)
2. **Task 1 (GREEN): Implementation** - `9a4d0e6` (feat)

_TDD task: RED committed tests that don't compile, GREEN committed implementation making all pass_

## Files Created/Modified
- `Cargo.toml` - Added quick-xml = "0.39" dependency
- `src/caldav_adapter.rs` - Replaced XML parser with NsReader, added normalize_etag, removed dead code

## Decisions Made
- Used quick-xml NsReader with `resolve_event()` for namespace-aware parsing rather than manual prefix detection. This correctly handles any namespace prefix (D:, d:, ns0:, bare default xmlns).
- Removed all legacy iCal parsing dead code from caldav_adapter.rs. These functions (parse_vtodo_from_ical, unfold_ical, etc.) were duplicates of the canonical implementation in ical.rs and annotated with #[allow(dead_code)].
- Applied normalize_etag at extraction boundary (3 sites) rather than at If-Match construction. This ensures ETags are always in canonical form throughout the application.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] quick-xml 0.39 API change: BytesText::unescape() removed**
- **Found during:** Task 1 (GREEN phase)
- **Issue:** quick-xml 0.39 renamed `BytesText::unescape()` to `BytesText::decode()`. Plan's code snippets referenced the old API.
- **Fix:** Used `e.decode()` for both BytesText and BytesCData events instead of `e.unescape()`.
- **Files modified:** src/caldav_adapter.rs
- **Verification:** All 16 caldav_adapter tests pass
- **Committed in:** 9a4d0e6 (Task 1 GREEN commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** API name change only, same semantics. No scope creep.

## Issues Encountered
- Git stash/pop conflict during pre-existing test verification caused loss of in-progress work. Resolved by re-applying changes from scratch using the complete file rewrite approach.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- XML parser now correctly handles responses from any CalDAV server (Radicale, Nextcloud, Baikal, etc.)
- ETag normalization prevents 412 Precondition Failed loops on servers returning weak ETags
- Ready for Plan 03 (remaining audit fixes) and Phase 2 (expanded test coverage)

---
*Phase: 01-code-audit-and-bug-fixes*
*Completed: 2026-03-18*
