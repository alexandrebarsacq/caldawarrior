# native-lww-sync

## Mission

Refactor CaldaWarrior's sync engine to use native Last-Write-Wins (TW.modified vs CalDAV LAST-MODIFIED) for conflict resolution and content-based deduplication for loop prevention, eliminating the X-CALDAWARRIOR-LAST-SYNC custom property so CalDAV VTODOs remain 100% standard-compliant.

## Objectives

- Remove the X-CALDAWARRIOR-LAST-SYNC iCalendar property from all VTODO writes
- Replace the custom-property-based LWW with native timestamp comparison (TW.modified vs LAST-MODIFIED)
- Change LAST-MODIFIED written to CalDAV from wall-clock `now` to `TW.modified` so LWW comparisons are self-consistent
- Rely on the existing content-based deduplication (Layer 2) as the primary loop prevention mechanism
- Ensure correct initial-sync fallbacks: TW.entry when TW.modified is absent, CalDAV DTSTAMP when LAST-MODIFIED is absent

## Success Criteria

- [ ] No VTODO written by caldawarrior contains X-CALDAWARRIOR-LAST-SYNC
- [ ] VTODO LAST-MODIFIED is set to `TW.modified` (falling back to `TW.entry`) on every CalDAV write
- [ ] LWW decision uses TW.modified vs CalDAV LAST-MODIFIED directly (no LAST-SYNC read)
- [ ] TW.modified == LAST-MODIFIED (with differing content) resolves as TW wins; self-stabilises via Layer 2 on the next sync
- [ ] Content-identical pairs are still skipped before LWW timestamp comparison
- [ ] All existing unit tests pass (updated as needed)
- [ ] All Robot Framework blackbox tests pass (updated as needed)

## Assumptions

- The existing content-based deduplication (Layer 2 in lww.rs) is sufficient for loop prevention without LAST-SYNC; no new mechanism is needed. Empirical validation is added as an explicit task.
- All timestamps (TWTask.modified, VTODO.last_modified, VTODO.dtstamp) are typed as `Option<DateTime<Utc>>` in Rust. TWTask.entry is typed as `DateTime<Utc>` (non-Optional — confirmed in types.rs). Comparison via `>` / `==` operators on `DateTime<Utc>` is inherently UTC-normalized — no explicit timezone conversion is required.
- `build_vtodo_from_tw()` writes `dtstamp: None` (confirmed: writeback.rs line 101). DTSTAMP is never written by caldawarrior. The `vtodo.last_modified.or(vtodo.dtstamp)` fallback in LWW therefore only applies to CalDAV-native VTODOs created by third-party clients that supply DTSTAMP but not LAST-MODIFIED.
- DTSTAMP represents the CalDAV object generation time and is a weaker signal than LAST-MODIFIED. It is used only as a fallback when LAST-MODIFIED is absent. If a CalDAV server regenerates DTSTAMP on every export, this can occasionally affect the LWW outcome; the LAST-MODIFIED-first priority mitigates this in practice.
- Setting LAST-MODIFIED = TW.modified in the VTODO body is supported by Radicale (which uses the value from the client PUT body). The subsequent native LWW comparison (TW.modified vs LAST-MODIFIED) then resolves to equality, and TW wins (see tie-breaker policy below) — the next sync finds identical content and skips, achieving self-stabilization.
- Backward compatibility: existing VTODOs on the server may still have X-CALDAWARRIOR-LAST-SYNC; we simply ignore it going forward. The property falls through to generic `extra_props` and is never read.
- The caldavuid UDA already exists and handles task-to-VTODO pairing; no change required there.
- This spec targets TaskWarrior 3.x. TW.modified is updated automatically by TW on every import/modify.

## Constraints

- Must not change the public API of run_sync() or any caller interfaces
- Must not alter the caldavuid UDA mechanism (already standard-compliant)
- Must not remove content-based deduplication (Layer 2); it is now the sole loop-prevention layer
- Rust edition 2024; no new external dependencies

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Layer 2 lossy-mapping loop: if TW normalizes a field on import in a way not captured by the 8-field content check (description, status, due, dtstart, completed, depends, wait), content_identical() returns false on re-sync and the normalized value ping-pongs | medium | medium | Investigation task in Phase 1 validates round-trip determinism for representative task types (time-boxed to 2 hours); if more than 2 gap types found, scope gaps as a follow-on spec and treat them as accepted risk. Risk row here. |
| Phase 1 investigation reveals widespread content check gaps, expanding scope beyond estimate | medium | medium | Time-box to 2 hours. If gaps exceed 2 types, extract to a follow-on spec. Proceed with native LWW treating known gaps as accepted risk. |
| Clock skew between TW host and CalDAV server causes wrong LWW decision when a third-party CalDAV client edits the VTODO | medium | low | Accepted operational risk; correctness requires NTP synchronization on TW host. Code has no mechanism to enforce this. Document in deployment notes. |
| Radicale overrides LAST-MODIFIED with its own timestamp on PUT, ignoring the client-supplied value | low | medium | Layer 2 (content check) catches the subsequent steady-state case. Investigation task includes verifying Radicale's behavior. |

## Open Questions

- None blocking implementation.

## Dependencies

- Phase 1 and Phase 2 must complete before Phase 4
- Phase 3 (cleanup) depends on Phase 1 (dead code removal is gated on LWW rewrite compiling)
- Phase 4 (verify) depends on Phases 1, 2, and 3

## Phases

### Phase 1: Update LWW Conflict Resolution

**Goal:** Replace X-CALDAWARRIOR-LAST-SYNC-based timestamp comparison with native TW.modified vs LAST-MODIFIED comparison in lww.rs.

**Description:** The current Layer 1 check compares TW.modified against X-CALDAWARRIOR-LAST-SYNC (a TW.modified value stored on the CalDAV server at last sync time). The new approach directly compares TW.modified vs CalDAV LAST-MODIFIED (which will be set to TW.modified on every write — see Phase 2). Content-identical deduplication (Layer 2) remains unchanged and is now the sole loop-prevention mechanism. `resolve_lww()` is only called for paired entries (both TW and CalDAV sides exist); creation of new items on either side is handled upstream in decide_op() and is not in scope here.

#### Tasks

- **Validate Layer 2 round-trip determinism** `investigation` `low`
  - Description: Trace a full round-trip sync of representative task types (plain text description, task with DUE, task with X-TASKWARRIOR-WAIT, task with DEPENDS-ON) and confirm that `content_identical()` returns true after a CalDAV→TW pull followed by a TW→CalDAV push. Verify that no field normalization in TW import or iCalendar serialization produces a spurious content difference. Document findings; if gaps found, the content check must be expanded (as an additional task) before Phase 1 is closed.
  - File: N/A
  - Acceptance criteria:
    - Round-trip trace documented in a test or manual checklist
    - No unhandled lossy mapping identified, OR content_identical() is extended to handle any identified cases
    - Radicale LAST-MODIFIED behavior confirmed (does it use client-supplied value or override?)
  - Depends on: none

- **Remove get_last_sync() helper** `refactoring` `low`
  - Description: Delete the `get_last_sync()` function (lww.rs:53-65) that extracts X-CALDAWARRIOR-LAST-SYNC from vtodo.extra_props. This is the sole reader of the custom property.
  - File: src/sync/lww.rs
  - Acceptance criteria:
    - `get_last_sync` function no longer exists in the file
    - `LAST_SYNC_PROP` constant is removed (or confirmed unused)
    - No compile errors from removing the function
  - Depends on: none

- **Rewrite Layer 1 LWW comparison** `implementation` `medium`
  - Description: In `resolve_lww()`, after the Layer 2 content check, replace `if tw_modified > last_sync` with a direct `tw_timestamp vs caldav_timestamp` comparison. Timestamps: `tw_timestamp = tw.modified.unwrap_or(tw.entry)` (TWTask.entry is `DateTime<Utc>`, non-Optional; no timezone conversion needed); `caldav_timestamp = vtodo.last_modified.or(vtodo.dtstamp)`. Decision tree:
    1. If `caldav_timestamp` is None → TW wins (conservative initial-sync default)
    2. If `tw_timestamp >= caldav_timestamp` → TW wins (equality: TW.modified was set to same second as LAST-MODIFIED, meaning a sub-second TW edit at the same second as the last write; prefer local edit — self-stabilizes on next sync via content check)
    3. Otherwise (`tw_timestamp < caldav_timestamp`) → CalDAV wins
  - File: src/sync/lww.rs
  - Acceptance criteria:
    - Layer 1 comparison no longer references X-CALDAWARRIOR-LAST-SYNC or get_last_sync()
    - TW.modified is compared directly against vtodo.last_modified (with fallback to vtodo.dtstamp)
    - TW.modified=None falls back to TW.entry
    - LAST-MODIFIED=None and DTSTAMP=None results in TW winning
    - TW.modified >= LAST-MODIFIED results in TW winning (>=: equality case prefers local edit; self-stabilizes next sync via content check)
    - Layer 2 (content check) is evaluated before Layer 1, and returns early without reaching timestamp comparison when content is identical
  - Depends on: Remove get_last_sync() helper

- **Update lww.rs unit tests** `implementation` `medium`
  - Description: Rewrite or remove unit tests that set up X-CALDAWARRIOR-LAST-SYNC in extra_props or assert on LAST-SYNC-based decisions. The `make_vtodo` helper signature must drop the `last_sync: Option<DateTime<Utc>>` parameter. Add new tests for all decision paths: (a) TW.modified > LAST-MODIFIED → TW wins; (b) TW.modified == LAST-MODIFIED → TW wins (equality, prefer local edit); (c) LAST-MODIFIED > TW.modified → CalDAV wins; (d) LAST-MODIFIED absent, DTSTAMP absent → TW wins; (e) LAST-MODIFIED absent, DTSTAMP present and newer → CalDAV wins; (f) TW.modified absent → falls back to TW.entry for comparison; (g) regression: content-identical pair → Skip(Identical) even when TW.modified > LAST-MODIFIED (Layer 2 fires before Layer 1).
  - File: src/sync/lww.rs
  - Acceptance criteria:
    - All unit tests in lww.rs pass with `cargo test`
    - Tests cover all seven scenarios (a)-(g) listed in description
    - No test references X-CALDAWARRIOR-LAST-SYNC for LWW decisions
    - Test (b) asserts TW wins on equal timestamps (not CalDAV wins)
    - Test (g) explicitly asserts Layer 2 fires before Layer 1 (i.e., returns Skip, not ResolveConflict)
  - Depends on: Rewrite Layer 1 LWW comparison

#### Verification

- **Run tests:** `cargo test sync::lww`
- **Fidelity review:** Compare implementation to spec
- **Manual checks:** `grep -r 'LAST.SYNC\|last_sync\|get_last_sync' src/` returns zero results in lww.rs

### Phase 2: Remove X-CALDAWARRIOR-LAST-SYNC from VTODO Writes and Fix LAST-MODIFIED Value

**Goal:** Stop writing the custom property into VTODO extra_props during CalDAV PUTs, and change LAST-MODIFIED from wall-clock `now` to `TW.modified` so the native LWW comparison is self-consistent.

**Description:** `build_vtodo_from_tw()` (writeback.rs:46-116) currently: (1) appends X-CALDAWARRIOR-LAST-SYNC to extra_props, and (2) sets `last_modified: Some(now)` using the wall-clock sync time. With the native LWW approach, LAST-MODIFIED must equal TW.modified so that after a TW-wins write, `TW.modified == LAST-MODIFIED` resolves to CalDAV wins (equality) on the next sync — meaning no redundant write. Other extra_props (X-TASKWARRIOR-WAIT passthrough) must remain unchanged.

#### Tasks

- **Remove LAST-SYNC write and fix LAST-MODIFIED value in build_vtodo_from_tw()** `implementation` `medium`
  - Description: In writeback.rs `build_vtodo_from_tw()`: (1) Delete the block that constructs the X-CALDAWARRIOR-LAST-SYNC IcalProp and appends it to extra_props. (2) Change `last_modified: Some(now)` to `last_modified: Some(tw.modified.unwrap_or(tw.entry))` — using TW.modified as the LAST-MODIFIED value written to CalDAV. The value must be serialized in `%Y%m%dT%H%M%SZ` format (RFC 5545 UTC, no fractional seconds, Z suffix) — the existing iCalendar serializer already uses this format. The `now` parameter is still needed for `completed: Some(now)` for completed tasks, so do not remove it from the function signature. The `LAST_SYNC_PROP` constant is removed in Phase 1 (owned there); remove only the writeback.rs-local constant and any remaining reference.
  - File: src/sync/writeback.rs
  - Acceptance criteria:
    - build_vtodo_from_tw() does not add X-CALDAWARRIOR-LAST-SYNC to extra_props
    - VTODO.last_modified is set to `tw.modified.unwrap_or(tw.entry)` (not wall-clock `now`)
    - Written LAST-MODIFIED is formatted as `YYYYMMDDThhmmssZ` (RFC 5545 UTC, no fractional seconds, Z suffix)
    - The string "X-CALDAWARRIOR-LAST-SYNC" no longer appears in writeback.rs
    - X-TASKWARRIOR-WAIT passthrough in extra_props is untouched
    - Compiles without errors
  - Depends on: none

- **Update writeback.rs tests** `implementation` `low`
  - Description: Fix unit tests in writeback.rs that: (a) pass `last_sync: Option<DateTime<Utc>>` to make_paired_entry or make_vtodo helpers — remove this parameter; (b) assert X-CALDAWARRIOR-LAST-SYNC is present in VTODO output — replace with assertions that the property is absent. The LWW winner assertions in tests (paired_tw_wins_pushes_to_caldav, paired_caldav_wins_updates_tw) should be updated to not rely on LAST-SYNC timestamps for setup.
  - File: src/sync/writeback.rs
  - Acceptance criteria:
    - `cargo test sync::writeback` passes with no reference to X-CALDAWARRIOR-LAST-SYNC in test assertions or helpers
    - `make_paired_entry` and `make_vtodo` helpers no longer accept a `last_sync` parameter
  - Depends on: Remove LAST-SYNC write and fix LAST-MODIFIED value in build_vtodo_from_tw()

#### Verification

- **Run tests:** `cargo test sync::writeback`
- **Fidelity review:** Compare implementation to spec
- **Manual checks:** `grep -r 'X-CALDAWARRIOR-LAST-SYNC\|LAST_SYNC_PROP' src/` returns zero results after Phase 1+2

### Phase 3: Clean Up iCalendar Parsing

**Goal:** Remove any dead code related to X-CALDAWARRIOR-LAST-SYNC parsing from ical.rs.

**Description:** The VTODO struct stores arbitrary properties in `extra_props: Vec<IcalProp>` (types.rs:229). There is no dedicated `last_sync` field — the property is only handled in extra_props passthrough. However, ical.rs may have a special match arm or constant for X-CALDAWARRIOR-LAST-SYNC that should be removed. Verify and clean up.

#### Tasks

- **Confirm and remove dedicated LAST-SYNC handling in ical.rs** `refactoring` `low`
  - Description: Inspect ical.rs for any special-case parsing of X-CALDAWARRIOR-LAST-SYNC (e.g., a named constant, a match arm extracting it into a struct field, or any call site that reads it). The VTODO struct has no `last_sync` field (confirmed: types.rs line 211-230). If special handling exists in ical.rs, remove it; the property will fall through to generic `extra_props` passthrough and be silently ignored. The `LAST_SYNC_PROP` constant is already removed by Phase 1 (owned by the "Remove get_last_sync() helper" task); verify zero occurrences remain.
  - File: src/ical.rs
  - Acceptance criteria:
    - No dedicated X-CALDAWARRIOR-LAST-SYNC handling in ical.rs beyond generic extra_props passthrough
    - No `last_sync` field on VTODO or any related struct
    - `grep -r 'LAST.SYNC\|last_sync\|LAST_SYNC' src/` returns zero results
    - `cargo test` passes
  - Depends on: Rewrite Layer 1 LWW comparison

#### Verification

- **Run tests:** `cargo test`
- **Fidelity review:** Compare implementation to spec
- **Manual checks:** `grep -r 'LAST.SYNC\|last_sync\|LAST_SYNC' src/` returns zero results

### Phase 4: Verify — Run All Tests and Fix Failures

**Goal:** Confirm the refactored sync engine passes all tests including Robot Framework blackbox tests.

**Description:** Run the full test suite (unit, integration, blackbox) and fix any remaining failures. The Robot Framework tests exercise the full sync pipeline against a live Radicale instance and are the ultimate acceptance gate. Tests that previously asserted X-CALDAWARRIOR-LAST-SYNC presence in VTODO output or relied on LAST-SYNC-based LWW outcomes must be fixed or removed.

#### Tasks

- **Run unit and integration tests** `implementation` `low`
  - Description: Run `cargo test` (all targets) and fix any remaining failures not already addressed in Phases 1-3.
  - File: N/A
  - Acceptance criteria:
    - `cargo test` exits 0
    - No test failures or panics
  - Depends on: none

- **Run Robot Framework blackbox tests** `implementation` `medium`
  - Description: Run `CURRENT_UID=$(id -u) CURRENT_GID=$(id -g) docker compose -f tests/robot/docker-compose.yml run --rm robot`. Identify failures introduced by the sync behaviour change — look for: (a) RF tests asserting X-CALDAWARRIOR-LAST-SYNC presence in VTODO content, (b) RF tests relying on LAST-SYNC-based LWW outcomes (e.g., TW winning because LAST-SYNC was absent). Fix failing tests. Pre-existing failures (7 tests) are out of scope unless directly caused by the LWW change.
  - File: tests/robot/
  - Acceptance criteria:
    - All 19 currently-passing Robot Framework tests continue to pass after the refactoring
    - Pre-existing 7 failures are out of scope (not introduced by this change)
    - No RF test checks for X-CALDAWARRIOR-LAST-SYNC presence in VTODO output
  - Depends on: Run unit and integration tests

#### Verification

- **Run tests:** `cargo test && CURRENT_UID=$(id -u) CURRENT_GID=$(id -g) docker compose -f tests/robot/docker-compose.yml run --rm robot`
- **Fidelity review:** Compare full implementation to this spec
- **Manual checks:** none
