# fix-rf-test-failures

## Mission

Fix the 7 failing Robot Framework blackbox tests by correcting test infrastructure bugs and a missing feature in the sync engine, so the full suite passes at 26/26 (excluding 5 pre-existing skips).

## Objectives

- Fix CalDAV teardown contamination causing S-03/S-04/S-05 to fail due to leftover VTODOs
- Fix `modify_vtodo_summary` not setting LAST-MODIFIED, causing S-11/S-62 to fail (TW always wins LWW)
- Fix `completed_cutoff_days` not being enforced in the sync engine, causing S-33 to fail
- Fix keyword name typo causing S-41 to fail with "No keyword found"

## Success Criteria

- [ ] RF suite passes 26 tests, 0 failures, 5 skips (up from 19/7/5)
- [ ] `cargo test` continues to pass (148+ tests, 0 failures)
- [ ] S-03, S-04, S-05 pass (CalDAV cleared between tests)
- [ ] S-11, S-62 pass (CalDAV wins LWW when LAST-MODIFIED is newer than TW.modified)
- [ ] S-33 passes (completed tasks beyond cutoff not synced)
- [ ] S-41 passes (keyword name resolves correctly)

## Assumptions

- The 5 skipped tests (S-42, S-50, S-51, S-52, S-53) are intentionally skipped and out of scope
- The 7 pre-existing failures are fully explained by the 4 root causes identified above
- `TW.modified` is bumped by caldawarrior when writing the `caldavuid` UDA back to TW, making it newer than the VTODO's DTSTAMP — this is why `modify_vtodo_summary` must set LAST-MODIFIED explicitly
- Radicale preserves LAST-MODIFIED when a VTODO is PUT (does not override it); this is standard iCal server behaviour for user-supplied properties — will be verified empirically as part of the modify_vtodo_summary task
- The `completed_cutoff_days` fix applies only to completed/deleted TW tasks that have no `caldavuid` (never synced) — already-synced tasks must remain visible to avoid orphan deletion
- RF resources (`CalDAVLibrary.py`, `common.robot`, suite `.robot` files) are volume-mounted from `./tests/robot/` into the container at `/tests` — Phase 1 fixes are visible to the test runner without rebuilding the Docker image; only the Rust binary change in Phase 2 requires a rebuild

## Constraints

- Must not break any currently-passing RF tests (19 passing must remain passing)
- `cargo test` must remain green after any Rust changes
- The blackbox tests must remain blackbox — test assertions use only the CLI output and CalDAV/TW state, not internal fields

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Radicale overrides LAST-MODIFIED on PUT | low | high | Verified empirically: task acceptance criteria require GET after PUT to confirm LAST-MODIFIED is preserved |
| `clear_vtodos` teardown fails on missing collection (PROPFIND to 404 endpoint) | low | medium | Handle 404 from PROPFIND as empty collection — return without error |
| Cutoff filter removes already-synced completed tasks | medium | high | Filter only tasks where `caldavuid.is_none()` |
| S-11/S-62 still fail if TW.modified is set at the same second as LAST-MODIFIED | low | low | LWW uses strict `>` so equal timestamps → TW wins (expected by spec) |
| `completed_cutoff_days = 0` silently drops all recently-completed unsynced tasks | low | medium | Treat `0` as no-op (filter disabled) — documented in acceptance criteria |

## Open Questions

- None — all root causes identified from test output and code review

## Dependencies

- `blackbox-integration-tests-2026-03-01-001` spec must remain active (this plan adds tasks to it)
- Docker image must be rebuilt after any Rust source changes (S-33 fix), but not for Phase 1

## Phases

### Phase 1: Fix Test Infrastructure

**Goal:** Fix the 6 test failures caused by three bugs in the test helper library and suites.

**Description:** Three bugs in `CalDAVLibrary.py`, `common.robot`, and `05_dependencies.robot` cause 6 of the 7 failures. First, test teardown only clears TaskWarrior data but leaves VTODOs in CalDAV, so later tests in the same suite find stale data (S-03, S-04, S-05). Second, `modify_vtodo_summary` doesn't update `LAST-MODIFIED`, so when caldawarrior runs LWW resolution, the CalDAV timestamp is the original DTSTAMP (from initial PUT), which is older than TW.modified (bumped when caldavuid is written) — TW always wins (S-11, S-62). Third, a keyword name typo prevents S-41 from running. Note: RF resources are volume-mounted — these fixes take effect immediately without rebuilding the Docker image.

#### Tasks

- **Add `clear_vtodos` method to CalDAVLibrary.py** `implementation` `low`
  - Description: Add a `clear_vtodos(collection_url)` method that does PROPFIND Depth:1 to list all .ics hrefs and DELETE each one. Use the same PROPFIND body pattern as `count_vtodos`. A 404 response from PROPFIND (collection not yet created, e.g. if test setup failed) must be treated as empty and return without error — this prevents teardown from masking the real test failure.
  - File: `tests/robot/resources/CalDAVLibrary.py`
  - Acceptance criteria:
    - `clear_vtodos(collection_url)` method exists and uses PROPFIND Depth:1 + DELETE loop
    - A 404 PROPFIND response is handled gracefully (returns without error, no exception raised)
    - Method is idempotent (empty collection returns without error)
    - Follows same `_check_response` pattern as other methods
  - Depends on: none

- **Update Test Teardown in common.robot to clear CalDAV** `implementation` `low`
  - Description: Update the `Test Teardown` in `common.robot` to call `CalDAV.Clear Vtodos ${COLLECTION_URL}` after `TW.Clear TW Data`. This ensures each test starts with an empty CalDAV collection.
  - File: `tests/robot/resources/common.robot`
  - Acceptance criteria:
    - `Test Teardown` calls both `TW.Clear TW Data` and `CalDAV.Clear Vtodos ${COLLECTION_URL}`
    - `Clear Vtodos` is a valid RF keyword (matches Python `clear_vtodos` method name via RF normalisation)
  - Depends on: Add `clear_vtodos` method to CalDAVLibrary.py

- **Fix `modify_vtodo_summary` to update LAST-MODIFIED** `implementation` `low`
  - Description: Add `component['LAST-MODIFIED'] = vDatetime(datetime.now(tz=timezone.utc))` inside the VTODO loop in `modify_vtodo_summary`, exactly as `add_vtodo_related_to` already does (line 309 of CalDAVLibrary.py). This ensures that when the test edits a VTODO summary, the CalDAV LAST-MODIFIED timestamp becomes newer than TW.modified, so the native LWW resolver correctly picks CalDAV as the winner. After the PUT, perform a GET on the same UID and assert that the returned VTODO contains a LAST-MODIFIED property — this verifies the Radicale assumption that PUT-supplied LAST-MODIFIED is preserved and not overridden by the server.
  - File: `tests/robot/resources/CalDAVLibrary.py`
  - Acceptance criteria:
    - `modify_vtodo_summary` sets `LAST-MODIFIED` to `datetime.now(tz=timezone.utc)` on the VTODO component before PUT
    - Uses same `vDatetime` import pattern as `add_vtodo_related_to`
    - After the PUT, a GET on the same resource confirms `LAST-MODIFIED` is present (verifies Radicale preserves it)
  - Depends on: none

- **Fix S-41 keyword name typo in 05_dependencies.robot** `implementation` `low`
  - Description: Change `CalDAV.Add VTODO Related-To` to `CalDAV.Add Vtodo Related To` on the failing line in `05_dependencies.robot`. Robot Framework does not strip hyphens when normalising keyword names, so `Related-To` does not match the Python method `add_vtodo_related_to` (which RF normalises to `Add Vtodo Related To`).
  - File: `tests/robot/suites/05_dependencies.robot`
  - Acceptance criteria:
    - The keyword call uses `CalDAV.Add Vtodo Related To` (no hyphen, title-case)
    - S-41 passes after this change
  - Depends on: none

#### Verification

- **Run tests:** `CURRENT_UID=$(id -u) CURRENT_GID=$(id -g) docker compose -f tests/robot/docker-compose.yml run --rm robot` (no rebuild needed — resources are volume-mounted)
- **Fidelity review:** Compare implementation to spec
- **Manual checks:** Verify S-03, S-04, S-05, S-11, S-41, S-62 pass

### Phase 2: Fix Sync Engine — completed_cutoff_days

**Goal:** Implement the `completed_cutoff_days` filter in the sync engine so completed tasks older than the cutoff are excluded from sync.

**Description:** The `completed_cutoff_days` config field (`u32`, default 90) is parsed but never used. Rather than filtering in `run_sync` (which would require allocating a new `Vec` and cloning tasks), the filter should be applied inline inside `build_ir`'s existing task loop, where each TW task is already being iterated. Pass `now` as an additional parameter to `build_ir` so it can compute `cutoff_dt = now - Duration::days(config.completed_cutoff_days as i64)` and skip tasks that are out-of-scope. Already-synced completed tasks (with `caldavuid.is_some()`) must NOT be filtered — removing them from TW's view would trigger orphan deletion of their CalDAV entries. When `completed_cutoff_days == 0`, the filter must be a no-op (all tasks pass through), as `0` is a legitimate user-supplied value meaning "disable cutoff."

#### Tasks

- **Add cutoff filter inside build_ir** `implementation` `medium`
  - Description: Add a `now: DateTime<Utc>` parameter to `build_ir` in `src/ir.rs`. Inside the loop over TW tasks, skip any task where all three conditions hold: `task.status == "completed" || task.status == "deleted"`, `task.caldavuid.is_none()`, and `task.end.map(|e| e < cutoff_dt).unwrap_or(false)`. Compute `cutoff_dt` as `now - Duration::days(config.completed_cutoff_days as i64)` but only when `config.completed_cutoff_days > 0` — if the value is `0`, skip the filter entirely (no-op). Update `run_sync` to pass `now` to `build_ir`. Update existing `build_ir` call sites and unit tests. Add a unit test verifying: (a) task filtered when `end < cutoff`; (b) task included when `end == cutoff` (boundary inclusive); (c) task included when `end == None`; (d) task included when `caldavuid.is_some()` regardless of end date; (e) no filtering when `completed_cutoff_days == 0`.
  - File: `src/ir.rs`
  - Acceptance criteria:
    - `build_ir` accepts `now: DateTime<Utc>` parameter
    - Completed/deleted tasks with no `caldavuid` and `end < cutoff` are excluded from IR
    - Completed tasks that already have a `caldavuid` are NOT filtered
    - Tasks with `end == None` are treated as within cutoff (included)
    - When `completed_cutoff_days == 0`, no filtering is applied (all tasks pass through)
    - Uses `as i64` cast for `Duration::days(config.completed_cutoff_days as i64)`
    - All 5 unit test boundary cases listed above are present and passing
    - `cargo test` passes after change
  - Depends on: none

#### Verification

- **Run tests:** `cargo test` first; then `docker compose -f tests/robot/docker-compose.yml build && CURRENT_UID=$(id -u) CURRENT_GID=$(id -g) docker compose -f tests/robot/docker-compose.yml run --rm robot` (Docker rebuild required for Rust binary change)
- **Fidelity review:** Compare implementation to spec
- **Manual checks:** S-33 passes after Docker image rebuild

### Phase 3: Full Suite Verification

**Goal:** Confirm all 7 previously-failing tests now pass and no regressions were introduced.

**Description:** Run the full Robot Framework suite. Phase 1 fixes (Python/RF resources) are volume-mounted and already live. Phase 2 (Rust binary) requires `cargo build --release` and a Docker image rebuild before RF execution. Expected result: 26 pass, 0 fail, 5 skip. Also confirm `cargo test` is green.

#### Tasks

- **Run full RF suite and confirm 26/0/5** `verification` `low`
  - Description: Confirm `cargo test` is green (no new failures). Then run the full RF suite via docker compose. All 7 previously-failing tests (S-03, S-04, S-05, S-11, S-33, S-41, S-62) must pass. The 5 skips (S-42, S-50, S-51, S-52, S-53) must remain skipped. Note: Phase 1 resource changes are volume-mounted (no rebuild needed). Phase 2 Rust binary change requires Docker image rebuild before this run.
  - File: `tests/robot/`
  - Acceptance criteria:
    - `cargo test`: 148+ passed, 0 failed
    - RF suite: 26 passed, 0 failed, 5 skipped
    - All 7 previously-failing scenarios pass individually
  - Depends on: Add cutoff filter inside build_ir, Fix `modify_vtodo_summary` to update LAST-MODIFIED, Add `clear_vtodos` method to CalDAVLibrary.py, Update Test Teardown in common.robot to clear CalDAV, Fix S-41 keyword name typo in 05_dependencies.robot

#### Verification

- **Run tests:** `cargo test && docker compose -f tests/robot/docker-compose.yml build && CURRENT_UID=$(id -u) CURRENT_GID=$(id -g) docker compose -f tests/robot/docker-compose.yml run --rm robot`
- **Fidelity review:** Compare full implementation to spec
- **Manual checks:** none
