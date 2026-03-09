# Gap Analysis: caldawarrior CLI Output Format & Behavior Coverage

## 1. Verified Output Formats

All format strings are traced directly to `src/output.rs` and `src/main.rs`.

### 1.1 Success Summary

Printed to **stdout** by `print_result()` in `src/output.rs:31-35` when not in dry-run mode:

```
Synced: {} created, {} updated in CalDAV; {} created, {} updated in TW
```

Where the four counters are:
- `caldav_creates` — `PlannedOp::PushToCalDav` ops (TW task pushed to CalDAV for the first time)
- `caldav_updates` — `PlannedOp::ResolveConflict { winner: Side::Tw }` ops (TW wins → CalDAV updated)
- `tw_creates` — `PlannedOp::PullFromCalDav` ops (CalDAV entry pulled into TW)
- `tw_updates` — `PlannedOp::ResolveConflict { winner: Side::CalDav }` ops (CalDAV wins → TW updated)

Note: `PlannedOp::DeleteFromCalDav` and `PlannedOp::DeleteFromTw` are counted internally but do NOT
appear in the success summary line. `PlannedOp::Skip` ops are also not reflected in the summary.

### 1.2 Zero-Write Summary

There is no separate format string for the zero-write case. When nothing changes, the same format
string is used with all counters set to `0`:

```
Synced: 0 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

This applies regardless of sync direction (TW→CalDAV or CalDAV→TW). The stable-point condition
(identical content on both sides) produces `PlannedOp::Skip { reason: SkipReason::Identical }`,
which does not increment any counter.

### 1.3 Dry-Run Prefix and Summary

Each planned operation is printed to **stdout** by `format_planned_op()` in `src/output.rs:82-118`:

```
[DRY-RUN] [CREATE] CalDAV <- TW: {description}
[DRY-RUN] [CREATE] TW <- CalDAV: {description}
[DRY-RUN] [DELETE] CalDAV: {description}
[DRY-RUN] [DELETE] TW: {description}
[DRY-RUN] [UPDATE] Conflict resolved (TW wins): {description}
[DRY-RUN] [UPDATE] Conflict resolved (CalDAV wins): {description}
[DRY-RUN] [SKIP] {uuid} ({skip_reason})
[DRY-RUN] [SKIP] ? ({skip_reason})
```

The `{description}` is resolved by `get_description()` (prefers TW task description, then CalDAV
VTODO SUMMARY, then CalDAV UID, then `"unknown"`). When `tw_uuid` is `None` on a Skip op, `?` is
used as the identifier placeholder.

The dry-run summary line is the last line printed, from `format_dry_run_summary()` in
`src/output.rs:43-47`:

```
[DRY-RUN] Would: {} create(s), {} update(s), {} delete(s), {} skip(s)
```

Where creates = caldav_creates + tw_creates, updates = caldav_updates + tw_updates, deletes and
skips counted separately.

### 1.4 Warning Formats

Warnings are printed to **stderr** by `print_result()` in `src/output.rs:15-21`.

**With a task UUID:**
```
[WARN] [{uuid}] {message}
```

**Without a task UUID:**
```
[WARN] {message}
```

Specific warning messages by source:

| Source | Message format |
|--------|----------------|
| `src/config.rs:95` (Unix only) | `[WARN] Config file {path:?} has permissions {mode:04o} — recommended: 0600` |
| `src/mapper/status.rs:47-51` | `recurring task {uuid} skipped (recur: {recur_value:?})` |
| `src/ir.rs:62-64` | `RecurringCalDavSkipped: VTODO '{uid}' has RRULE and will not be synced` |
| `src/sync/deps.rs:44-49` | `UnresolvableDependency: TW UUID {uuid} has no CalDAV UID` |
| `src/sync/deps.rs:52-57` | `UnresolvableDependency: TW UUID {uuid} not found in IR` |
| `src/sync/deps.rs:140-144` | `CyclicEntry: task '{description}' is part of a dependency cycle` |

Note: The config file permission warning is emitted directly by `eprintln!` in `src/config.rs:95`,
bypassing the `Warning` struct and `print_result()`. Its format therefore differs from the others —
it always starts with `[WARN]` but its exact content is produced by `eprintln!` not the generic
`"[WARN] {message}"` pattern.

Full eprintln! call from `src/config.rs:95`:
```rust
eprintln!("[WARN] Config file {:?} has permissions {:04o} — recommended: 0600", path, mode & 0o777);
```

### 1.5 Error Formats

Errors are printed to **stderr** by `print_result()` in `src/output.rs:10-12`:

```
[ERROR] {message}
```

Runtime errors from CalDAV/TW operations land in `result.errors` and are surfaced this way.

**Fatal errors** that fail before `print_result()` is called are handled by `main()` in
`src/main.rs:31-34`, which prints to stderr and exits with code 1:

```
Error: {anyhow_error_chain}
```

**Auth failure** (`CaldaWarriorError::Auth`) uses the `thiserror` display format from
`src/error.rs:16`:

```
Authentication failed for {server_url}: check your credentials in the config file
```

When propagated through `anyhow` context (e.g., from `list_vtodos`), the full stderr line is:

```
Error: Failed to list VTODOs from calendar '{url}': Authentication failed for {server_url}: check your credentials in the config file
```

The process exit code is `1` in all error cases (fatal or non-fatal errors in `result.errors`).

---

## 2. Config File Schema

Verified against `src/config.rs`.

### 2.1 Required Top-Level Keys

- `server_url`: String — base URL of the CalDAV server (e.g., `"https://dav.example.com"`)
- `username`: String — HTTP Basic Auth username
- `password`: String — HTTP Basic Auth password (can be overridden by `CALDAWARRIOR_PASSWORD` env var)

### 2.2 Optional Top-Level Keys

- `completed_cutoff_days`: u32, default `90` — how many days back completed tasks are synced
- `allow_insecure_tls`: bool, default `false` — skip TLS certificate verification
- `caldav_timeout_seconds`: u64, default `30` — HTTP request timeout

### 2.3 `[[calendar]]` Section

Each `[[calendar]]` entry (TOML array of tables; internally deserialized as `calendars` via
`#[serde(rename = "calendar")]`) has exactly two required fields:

- `project`: String — the TaskWarrior project name this calendar maps to; use `"default"` as the
  fallback catch-all
- `url`: String — full CalDAV calendar collection URL (e.g., `"https://dav.example.com/alice/default/"`)

Example minimal config:

```toml
server_url = "https://dav.example.com"
username = "alice"
password = "secret"

[[calendar]]
project = "default"
url = "https://dav.example.com/alice/default/"
```

Config path resolution order:
1. `--config PATH` CLI flag
2. `CALDAWARRIOR_CONFIG` environment variable
3. `~/.config/caldawarrior/config.toml` (default)

---

## 3. caldavuid UDA

- **Key name:** `caldavuid`
- **Type:** `string`
- **Label:** `CaldavUID`
- **Source location:** `src/tw_adapter.rs:187-193`

The UDA is auto-registered on every run via:
```
task config uda.caldavuid.type string
task config uda.caldavuid.label CaldavUID
```

This is called in `TwAdapter::new()` → `register_uda()` before any other TW operations.

The UDA stores the CalDAV UID (a string, typically a UUID4) that pairs the TW task to its
corresponding VTODO. It is set on first push (TW → CalDAV) and read on subsequent syncs to
determine the paired/orphaned/new classification in the IR.

---

## 4. RELATED-TO → depends Mapping

**TW → CalDAV (forward mapping):**
Implemented in `src/sync/deps.rs` and `src/mapper/fields.rs:77-79`. Each TW `depends` UUID is
looked up in the IR index to find the corresponding CalDAV UID, then written as
`RELATED-TO;RELTYPE=DEPENDS-ON:{caldav_uid}` in the VTODO.

**CalDAV → TW (reverse mapping):**
Implemented in `src/sync/writeback.rs:146-152`. When building a TW task from a CalDAV VTODO,
`RELATED-TO;RELTYPE=DEPENDS-ON` UIDs are reverse-mapped to TW UUIDs using the IR index
(`caldav_uid_to_tw_uuid` HashMap). If the CalDAV UID is found in the index, the corresponding TW
UUID is added to the `depends` field of the TW task.

**Status:** Both forward and reverse mapping are implemented. The reverse mapping only works when
the depended-upon VTODO is also present in the current IR (i.e., the corresponding TW task is also
being synced). If a `RELATED-TO` UID does not match any known entry, it is silently dropped (no
warning for this specific case — the `UnresolvableDependency` warning is emitted only for TW-side
missing dependencies).

---

## 5. Existing Rust Integration Tests

**Scope:** Integration tests in `tests/integration/` that exercise the sync engine end-to-end
using Docker (Radicale CalDAV + TaskWarrior container). These are distinct from the 128 unit tests
in `src/`.

All integration tests guard with `if should_skip() { return; }`, which checks for the
`SKIP_INTEGRATION_TESTS` environment variable. They call the library API directly (not the CLI
binary subprocess) via `run_sync()`.

**Actual count: 18 total** (12 scenario tests + 6 harness utility tests in mod.rs)

The task description referenced 12, which corresponds to the 12 tests in the three scenario files
(`test_first_sync.rs`, `test_lww.rs`, `test_scenarios.rs`), excluding the 6 harness tests in
`mod.rs`.

| # | Test Name | File | Level | What It Tests |
|---|-----------|------|-------|---------------|
| 1 | `first_sync_pushes_tw_tasks_to_caldav` | `test_first_sync.rs` | library (Docker) | Two TW tasks pushed to CalDAV on first sync; `written_caldav == 2` |
| 2 | `first_sync_sets_caldavuid_uda_on_tw_task` | `test_first_sync.rs` | library (Docker) | After sync, `caldavuid` UDA is non-empty on TW task JSON |
| 3 | `first_sync_dry_run_does_not_write_vtodos` | `test_first_sync.rs` | library (Docker) | Dry-run produces zero CalDAV writes and at least one planned op |
| 4 | `first_sync_project_mapping_routes_to_default_calendar` | `test_first_sync.rs` | library (Docker) | Project-less TW task routes to the "default" calendar |
| 5 | `tw_wins_lww` | `test_lww.rs` | library (Docker) | TW-modified task wins LWW; CalDAV updated, TW not re-written |
| 6 | `caldav_wins_lww` | `test_lww.rs` | library (Docker) | CalDAV-modified VTODO wins LWW; TW task description updated |
| 7 | `loop_prevention_stable_point` | `test_lww.rs` | library (Docker) | After CalDAV-wins sync, immediate re-sync produces zero writes |
| 8 | `etag_conflict_scenario` | `test_lww.rs` | library (Docker) | Concurrent TW + CalDAV edits; ETag 412 handled without error |
| 9 | `status_sync_caldav_completed_to_tw` | `test_scenarios.rs` | library (Docker) | CalDAV COMPLETED status propagates to TW task status |
| 10 | `dependency_sync_tw_to_caldav` | `test_scenarios.rs` | library (Docker) | TW `depends` synced as RELATED-TO in CalDAV; reverse mapping verified |
| 11 | `orphaned_caldavuid_causes_tw_deletion` | `test_scenarios.rs` | library (Docker) | TW task with orphaned caldavuid is deleted, not re-created in CalDAV |
| 12 | `large_dataset_first_sync` | `test_scenarios.rs` | library (Docker) | 100-task first sync completes; second sync is a stable point |
| 13 | `parse_hrefs_extracts_ics_files_only` | `mod.rs` | unit (no Docker) | XML multistatus parser extracts only .ics hrefs |
| 14 | `parse_hrefs_empty_on_no_ics` | `mod.rs` | unit (no Docker) | XML multistatus parser returns empty when no .ics files present |
| 15 | `parse_hrefs_handles_bare_tags` | `mod.rs` | unit (no Docker) | XML multistatus parser handles bare `<href>` tags without namespace |
| 16 | `harness_creates_isolated_calendar_and_tw_dir` | `mod.rs` | library (Docker) | TestHarness creates non-empty calendar URL and TW data directory |
| 17 | `harness_reset_clears_tw_task_data` | `mod.rs` | library (Docker) | `reset()` removes `.data` files from TW temp directory |
| 18 | `harness_add_tw_task_returns_uuid` | `mod.rs` | library (Docker) | `add_tw_task()` returns a 36-character UUID string |

---

## 6. Unimplemented Behaviors (tag: skip-unimplemented)

The following behaviors are either not implemented or not exercised via the CLI binary subprocess,
and should be tagged `skip-unimplemented` in Robot Framework tests:

1. **Bad-auth CLI exit code (CLI-level test)** — The auth error path IS implemented in the library
   (`CaldaWarriorError::Auth` is raised on HTTP 401 in `src/caldav_adapter.rs:151`, `192`, `221`)
   and `main.rs` does call `process::exit(1)` for errors. However, no test exercises this via the
   CLI binary subprocess. A Robot Framework test invoking the binary with bad credentials would
   verify exit code 1 and the error message format. This is testable in principle but not currently
   covered by any test.

2. **Config file permission warning (CLI-level test, Unix only)** — The permission check IS
   implemented in `src/config.rs:88-98` (Unix only, via `#[cfg(unix)]`). It emits directly to
   stderr via `eprintln!`. No test currently exercises this path. A Robot Framework test would
   need to create a world-readable config file and verify the `[WARN]` output.

3. **Recurring TW task skip warning (CLI-level test)** — The skip IS implemented in
   `src/mapper/status.rs:45-51`: TW tasks with `status == "recurring"` emit a `Warning` with
   message `"recurring task {uuid} skipped (recur: {value:?})"`. The warning is surfaced via
   `print_result()` → stderr as `[WARN] [{uuid}] recurring task ...`. No CLI-level test covers
   this.

4. **Recurring CalDAV VTODO skip warning (CLI-level test)** — Implemented in `src/ir.rs:58-66`:
   CalDAV VTODOs with `RRULE` set are skipped with a warning message
   `"RecurringCalDavSkipped: VTODO '{uid}' has RRULE and will not be synced"`. No CLI-level test
   covers this.

5. **Cyclic dependency warning (CLI-level test)** — Implemented in `src/sync/deps.rs:138-145`:
   dependency cycles are detected via DFS and emit
   `"CyclicEntry: task '{description}' is part of a dependency cycle"` warnings. These tasks get
   `PlannedOp::Skip { reason: SkipReason::Cyclic }`. No CLI-level test covers this.

6. **CLI binary subprocess tests (any behavior)** — All 18 existing integration tests call the
   library API (`run_sync()`) directly via Rust, not the compiled binary subprocess. Robot
   Framework blackbox tests that invoke `caldawarrior sync ...` as a subprocess cover a gap that
   none of the existing Rust tests address.

---

## 7. Implemented Behaviors (ready for testing)

The following behaviors are fully implemented in the codebase and ready for Robot Framework
blackbox tests:

1. **First sync TW → CalDAV** — TW-only tasks (no `caldavuid`) are created as VTODOs on the
   CalDAV server. Verified by `first_sync_pushes_tw_tasks_to_caldav` and source at
   `src/sync/writeback.rs:484-492`.

2. **caldavuid UDA set after push** — After a TW task is pushed to CalDAV, its `caldavuid` field
   is updated in TW via `task {uuid} modify caldavuid:{uid}`. Verified by
   `first_sync_sets_caldavuid_uda_on_tw_task` and source at `src/tw_adapter.rs:303-306`.

3. **Dry-run mode** — `--dry-run` flag produces `[DRY-RUN]` prefixed stdout lines and final
   summary, without writing to CalDAV or TW. Verified by
   `first_sync_dry_run_does_not_write_vtodos` and source at `src/output.rs:23-27`.

4. **Project → calendar URL routing** — TW tasks without a project route to the `"default"`
   calendar entry. Verified by `first_sync_project_mapping_routes_to_default_calendar` and source
   at `src/ir.rs:14-25`.

5. **LWW conflict resolution (TW wins)** — When TW task `modified` timestamp is newer than the
   last sync timestamp, TW wins and CalDAV is updated. Verified by `tw_wins_lww` and source at
   `src/sync/lww.rs`.

6. **LWW conflict resolution (CalDAV wins)** — When CalDAV `LAST-MODIFIED` is newer and TW
   hasn't changed since last sync, CalDAV wins and TW is updated. Verified by `caldav_wins_lww`.

7. **Loop prevention (stable point)** — After a CalDAV-wins sync, re-running immediately produces
   zero writes on both sides. Verified by `loop_prevention_stable_point` and
   `dry_run_summary_correct_counts` in `src/output.rs`.

8. **ETag conflict handling** — HTTP 412 on PUT triggers re-fetch of the current VTODO and retry.
   Verified by `etag_conflict_scenario` and source at `src/caldav_adapter.rs:193-197`.

9. **CalDAV COMPLETED → TW completed** — When a VTODO is marked COMPLETED externally, the next
   sync updates the TW task status to `"completed"`. Verified by
   `status_sync_caldav_completed_to_tw`.

10. **TW depends → RELATED-TO (forward)** — TW `depends` UUIDs are resolved to CalDAV UIDs and
    written as `RELATED-TO;RELTYPE=DEPENDS-ON` in the VTODO. Verified by
    `dependency_sync_tw_to_caldav` and source at `src/mapper/fields.rs:77-79`.

11. **RELATED-TO → TW depends (reverse)** — CalDAV `RELATED-TO;RELTYPE=DEPENDS-ON` UIDs are
    reverse-mapped to TW UUIDs when building TW tasks from CalDAV. Verified by
    `dependency_sync_tw_to_caldav` (phase b) and source at `src/sync/writeback.rs:146-152`.

12. **Orphaned caldavuid → TW task deletion** — When a TW task has `caldavuid` set but no
    matching VTODO exists in CalDAV, the TW task is deleted (not re-created). Verified by
    `orphaned_caldavuid_causes_tw_deletion` and source at `src/sync/writeback.rs:279-281`.

13. **Large dataset stability** — Syncing 100 tasks completes without duplication or data loss,
    and the immediate re-sync is a stable point. Verified by `large_dataset_first_sync`.

14. **Success summary format (live mode)** — stdout line is `"Synced: N created, N updated in CalDAV; N created, N updated in TW"`. Source: `src/output.rs:31-35`.

15. **Dry-run summary format** — Last stdout line is `"[DRY-RUN] Would: N create(s), N update(s), N delete(s), N skip(s)"`. Source: `src/output.rs:43-47`.

16. **Error exit code** — When `result.errors` is non-empty, the process exits with code 1.
    Source: `src/main.rs:89-91`.

17. **Fatal error exit code** — When config load or adapter init fails, the process exits with
    code 1 via the `if let Err(e) = run()` handler in `src/main.rs:31-34`.
