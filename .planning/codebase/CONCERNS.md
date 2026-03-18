# Codebase Concerns

**Analysis Date:** 2026-03-18

## Tech Debt

**Panic-based error handling in test assertions:**
- Issue: Status mapper and field mapper tests use `panic!()` directly instead of proper test assertions, making failures less informative.
- Files: `src/mapper/status.rs` (4 panic calls), `src/mapper/fields.rs` (1 expect call)
- Locations: Lines 117, 126, 140, 152 in status.rs; line 281 in fields.rs
- Impact: Test failures crash with generic panic messages instead of clear assertion descriptions. Makes debugging harder during test runs.
- Fix approach: Replace panic assertions with `match` statements that use `assert!()` or `unreachable!()` with descriptive messages. Example from status.rs: convert `panic!("expected NeedsActionWithWait, got {:?}", other)` to proper assertion.

**Unwrap/expect calls in fallback paths:**
- Issue: 190+ unwrap/expect calls across codebase, though most are in tests. Production code has several in critical paths.
- Files: `src/caldav_adapter.rs` (6 unwrap_or_default), `src/sync/writeback.rs` (2 unwrap_or_default), `src/sync/deps.rs` (2 unwrap_or_default), `src/mapper/status.rs` (2 unwrap_or for fallback values), `src/mapper/fields.rs` (1 expect)
- Locations: caldav_adapter.rs lines 98, 114, 148, 155, 200, 229
- Impact: While unwrap_or_default() is safe, it silently swallows errors. If CalDAV response parsing fails catastrophically, body becomes empty string rather than propagating the error. Reduces observability.
- Fix approach: Add explicit logging before unwrap_or_default() calls to record when we fall back to defaults. Consider structured error logging middleware.

**Rust edition mismatch:**
- Issue: Cargo.toml specifies `edition = "2024"` which does not exist. Rust editions are 2015, 2018, 2021.
- Files: `Cargo.toml` line 4
- Impact: Build may fail or default to 2021 edition silently. Creates ambiguity in code interpretation and possible incompatibility with tooling.
- Fix approach: Change to `edition = "2021"` (current stable). Verify no code relies on 2024 features (there are none — this appears to be a typo from a template).

**Cyclic dependency handling is incomplete:**
- Issue: Cyclic tasks are marked and skipped with warnings, but no user documentation explains what to do when cycles are detected.
- Files: `src/sync/deps.rs` (lines 87-127), `src/sync/writeback.rs` (line 231)
- Impact: Users see "cyclic dependency" warnings but don't know how to resolve them. Sync quietly skips affected tasks. No guidance on fixing dependencies in TW.
- Fix approach: Document cycle resolution in README. Add example of breaking cycles. Enhance warning message with actionable steps.

## Known Bugs

**iCalendar line unfolding may lose precision:**
- Issue: `unfold_lines()` in `src/ical.rs` uses simple string operations to handle RFC 5545 line folding. Edge case: CRLF sequences in escaped text could be mishandled.
- Files: `src/ical.rs` (lines 13-23 implicit via unfold)
- Symptoms: Rare parsing failures on malformed iCalendar input with embedded carriage returns
- Trigger: iCalendar with CRLF in quoted string values, especially in DESCRIPTION fields
- Workaround: Normalize input with dos2unix before sync
- Verification: No unit tests for edge cases in line unfolding

**ETag conflict retry loop exhausts after 3 attempts:**
- Issue: MAX_ETAG_RETRIES = 3 hardcoded. If CalDAV server consistently modifies during concurrent updates, sync fails with SyncConflict after exactly 3 retries.
- Files: `src/sync/writeback.rs` line 18 (constant), lines 350-400 (retry loop)
- Symptoms: Sync errors on high-contention calendars with concurrent clients
- Trigger: Multiple caldawarrior instances syncing same calendar collection, or external client modifying during writeback
- Workaround: Increase delay between manual retries, stagger multiple sync instances
- Test coverage: Tested (writeback test: etag_conflict_retries_and_exhausts) but only 3 hardcoded attempts verified

**Default fallback for TW `wait` and `end` fields may mask missing data:**
- Issue: When TW task has status "waiting" but no `wait` timestamp, code falls back to `task.entry`. Similarly for "completed" status falling back to `entry` when `end` is None.
- Files: `src/mapper/status.rs` lines 41, 55
- Symptoms: Completed tasks synced with wrong completion time (task creation time instead of completion time)
- Trigger: TaskWarrior version incompatibility or data corruption where `end` field is missing from completed tasks
- Current behavior: No warning emitted. CalDAV receives task.entry (creation time) as completion time.
- Fix approach: Emit explicit warnings when falling back, suggesting user audit the TW task. Consider adding validation phase.

## Security Considerations

**Password transmission risk in config file:**
- Risk: Plaintext passwords stored in `~/.config/caldawarrior/config.toml`. File permissions warned at runtime (0600 check) but not enforced.
- Files: `src/config.rs` (password parsing), `src/main.rs` (0600 check)
- Current mitigation: Runtime warning if permissions > 0600. Optional override via environment variable `CALDAWARRIOR_PASSWORD`.
- Recommendations:
  - Enforce (not just warn) config file permissions on Unix — fail startup if > 0600
  - Document environment variable password override in README prominently
  - Consider keychain/secret manager integration for future versions
  - Add test to verify warning is emitted for permissive configs

**CalDAV credentials transmitted over HTTP if URL misconfigured:**
- Risk: If server_url uses `http://` instead of `https://`, credentials sent in plaintext. No validation prevents this.
- Files: `src/config.rs` (parsing), `src/caldav_adapter.rs` (client setup)
- Current mitigation: None. Documentation recommends HTTPS but not enforced.
- Recommendations: Add config validation to reject `http://` URLs unless explicitly enabled via `allow_insecure_http` flag. Warn prominently in README about http risk.

**TLS insecure mode disables certificate validation:**
- Risk: `allow_insecure_tls: true` disables all certificate validation via `danger_accept_invalid_certs()`. Enables MITM attacks.
- Files: `src/caldav_adapter.rs` line 54
- Current mitigation: Off by default. Documented in README as "optional insecure mode for self-signed certificates"
- Recommendations:
  - Prefer certificate pinning or custom CA bundle over blanket disable
  - Document self-signed certificate setup alternatives
  - Add warning when insecure_tls is enabled

## Performance Bottlenecks

**No connection pooling for CalDAV requests:**
- Problem: `RealCalDavClient` creates new `reqwest::blocking::Client` per instantiation. No connection reuse across multiple API calls.
- Files: `src/caldav_adapter.rs` lines 44-67
- Cause: Single client instance created once per sync run, but verbose instantiation. No pool for multiple concurrent syncs.
- Current impact: Acceptable for single sync runs but would become bottleneck if CLI is daemonized or called frequently.
- Improvement path: Singleton client per process (move to main.rs or lazy_static). Add keep-alive headers. Benchmark sync time with large calendar collections (>1000 tasks).

**Large file parsing without streaming:**
- Problem: `from_icalendar_string()` loads entire VTODO components into memory. No streaming parser.
- Files: `src/ical.rs` (entire module)
- Cause: RFC 5545 standard favors in-memory parsing; streaming is complex due to property folding.
- Current impact: Acceptable for individual VTODO items (rarely >100KB) but problematic if calendar has 10K+ tasks and client fetches all at once.
- Improvement path: Profile memory usage with real Radicale calendar. Consider chunked fetching if CalDAV supports limit parameter.

**Dependency resolution O(n²) worst case:**
- Problem: Cycle detection in `src/sync/deps.rs` uses iterative DFS but rebuilds adjacency list for each task. No memoization of resolved paths.
- Files: `src/sync/deps.rs` lines 66-127
- Cause: Three-colour DFS is correct but adjacency list construction is repeated.
- Current impact: Negligible for typical task counts (<10K). Would be noticeable at 100K+ tasks.
- Improvement path: Pre-compute adjacency list once, reuse across cycle detection iterations.

## Fragile Areas

**Status mapping has hidden assumptions about TW field presence:**
- Files: `src/mapper/status.rs` (entire module)
- Why fragile: Function `tw_to_caldav_status()` assumes specific TW field presence:
  - "waiting" status must have `wait` field or defaults to `entry`
  - "completed" status must have `end` field or defaults to `entry`
  - "recurring" status may have `recur` field but not required
  - Unknown status silently becomes NEEDS-ACTION (no error)
- Safe modification: Add comprehensive docstring explaining all assumptions. Add tests for each field presence combination. Consider returning Result<> instead of panicking on bad combos.
- Test coverage: Unit tests cover happy paths (wait/end present) but lack coverage for field-absent cases. Gaps in status="recurring" with missing recur field.

**iCalendar property parsing is permissive but incomplete:**
- Files: `src/ical.rs` lines 40-100 (property line parsing)
- Why fragile: Parser extracts known fields (SUMMARY, DESCRIPTION, STATUS, PRIORITY) but silently discards unknown properties. If CalDAV server sends new RFC 5545 properties (e.g., RDATE, ATTACH), they are dropped without warning.
- Safe modification: Preserve unknown properties in `extra_props` (already done). Before modifying parser, audit for RFC 5545 compliance. Add RFC version note to spec.
- Test coverage: Tests cover known fields only. No tests for preservation of unknown properties round-tripping.

**Annotation slot invariant (slot 0 owned by CalDAV sync):**
- Files: `src/sync/writeback.rs` lines 37-76
- Why fragile: Comment documents invariant "slot 0 is owned by CalDAV sync; slots 1+ are user-created" but this is enforced by merge logic, not type system. User could manually edit TW and insert annotation at slot 0, breaking invariant.
- Safe modification: Document invariant prominently in TW UDA setup instructions. Add validation in TwAdapter that warns if slot 0 was manually modified. Consider using separate annotation category to avoid conflicts.
- Test coverage: Tests verify merge behavior but not invariant violations from external TW edits.

## Scaling Limits

**CalDAV collection fetch in single request:**
- Current capacity: Works well for calendars with <1000 VTODO items (typical personal calendars)
- Limit: CalDAV PROPFIND with depth=1 returns all items. Memory grows O(n). HTTP request timeout (30s default) may be exceeded for very large collections.
- Scaling path: Implement pagination via CalDAV query filters (RFC 3744 DASL). Chunk by date range or use LIMIT if server supports it.

**TaskWarrior CLI invocations for each operation:**
- Current capacity: Batching is limited. Each task create/update/delete requires separate `task` command invocation.
- Limit: If syncing thousands of changed tasks, TW becomes bottleneck. No built-in task batch operations.
- Scaling path: Consider TaskWarrior hook system or library bindings instead of CLI. Implement transaction-like batching within a single TW run.

## Dependencies at Risk

**Rust edition 2024 (non-existent):**
- Risk: Cargo.toml specifies non-existent edition. Build tooling may default or fail.
- Impact: Builds with warnings. Possible incompatibility with future rustc versions.
- Migration plan: Change to edition 2021 immediately (one-line fix).

**reqwest 0.12 is relatively new (released late 2024):**
- Risk: Newer version of reqwest means less field-tested, more likely breaking changes in minor versions.
- Impact: Cargo.lock pins version but updates may introduce incompatibilities.
- Mitigation: Monitor changelog. Pin to minor version (0.12.x).

**chrono-tz 0.10 has known ambiguous time handling:**
- Risk: Timezone conversions around DST transitions may be ambiguous. chrono-tz relies on zoneinfo database (external dependency).
- Impact: Daylight saving time transitions could cause sync errors.
- Mitigation: All comparisons use UTC internally (DateTime<Utc>). Test explicitly with DST transition dates.

## Missing Critical Features

**No offline support:**
- Problem: Sync immediately fails if CalDAV server is unreachable. No local queue for changes while offline.
- Blocks: Laptop users, intermittent connectivity scenarios
- Priority: Medium — reasonable for server-centric tool but note for future

**No selective sync by date range:**
- Problem: All sync operations fetch entire calendar. Cannot sync only recent changes.
- Blocks: Performance optimization for large calendars
- Priority: Low — covered by scheduling frequent syncs

**No custom field mapping configuration:**
- Problem: Field mapping is hardcoded (SUMMARY ↔ description, PRIORITY ↔ priority). User cannot map additional UDAs to iCalendar properties.
- Blocks: Advanced users wanting richer metadata sync
- Priority: Low — acceptable for initial release

## Test Coverage Gaps

**iCalendar parsing edge cases:**
- What's not tested: Malformed VTODO (missing UID, missing BEGIN:VTODO), timezone handling in datetime parsing, multi-line property folding edge cases, escape sequence handling (DESCRIPTION with \n vs actual newlines)
- Files: `src/ical.rs`
- Risk: Undetected parsing failures on edge-case CalDAV servers (Exchange, Apple Calendar) that produce non-standard iCalendar output
- Priority: High — parsing is critical path for all syncs

**CalDAV HTTP error responses:**
- What's not tested: Server returning 401 Unauthorized, 500 Internal Server Error, timeout scenarios, partial list responses, missing ETag header fallback
- Files: `src/caldav_adapter.rs`
- Risk: Network failures escalate to SyncConflict or unrecoverable errors. No graceful degradation.
- Priority: High — critical for reliability

**Dependency cycle detection with cross-dependencies:**
- What's not tested: Cycle involving both TW-only and CalDAV tasks, cycles larger than 2 nodes, dependency on non-existent tasks, missing caldav_uid during cycle detection
- Files: `src/sync/deps.rs`
- Risk: Incorrect cycle detection could skip valid tasks or fail to detect actual cycles
- Priority: Medium — covered by unit tests but integration scenarios untested

**LWW conflict resolution with extreme timestamps:**
- What's not tested: Year 2038 problem (32-bit timestamp overflow), timestamps in far future, microsecond precision loss, out-of-order timestamp updates
- Files: `src/sync/lww.rs`
- Risk: Incorrect winner selection due to timestamp truncation or overflow
- Priority: Low — unlikely in practice but conceptually important

**Concurrent sync runs (race conditions):**
- What's not tested: Multiple caldawarrior instances running simultaneously against same calendar, TW database file locks during multi-instance sync, ETag conflict under true concurrency
- Files: All sync logic
- Risk: Data corruption, lost updates, orphaned VTODOs if multiple syncs run concurrently
- Priority: Medium — currently single-threaded design, but worth documenting constraint

---

*Concerns audit: 2026-03-18*
