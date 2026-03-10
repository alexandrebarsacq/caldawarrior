# Known Bugs

Developer-facing notes on confirmed real-world issues not yet covered by the test suite.

---

## Bug 1 — Unmapped-project tasks counted in sync summary

### Symptom

When TaskWarrior contains tasks whose `project` field has no matching `[[calendar]]` entry in
the config, they appear in the success summary ("Synced: X created … in CalDAV") even though
they cannot actually be written to any calendar.

### Root Cause

`build_ir()` in `src/ir.rs` (lines 78–102) always pushes an `IREntry` to the list regardless
of whether `calendar_url` is `None`. The entry gets a fresh UUID assigned and eventually
reaches `planned_ops` as a `PushToCalDav` operation.

`count_ops()` in `src/output.rs` counts every `PushToCalDav` as a CalDAV create without
checking whether `calendar_url` is set. If a write is actually attempted, the href becomes a
malformed path (`/{uid}.ics`), which almost certainly causes a 403 from the server (see Bug 2).

### Affected Files

| File | Location |
|------|----------|
| `src/ir.rs` | `build_ir()` — unmapped tasks should be filtered out early |
| `src/sync/writeback.rs` | `decide_op` / `execute_op` — should guard on `calendar_url` |
| `src/output.rs` | `count_ops` — should not count ops with no calendar URL |

### Required Fix

Tasks with `calendar_url = None` must be excluded from the IR (or at minimum from
`planned_ops`) so they never reach write-back or the summary count.

A `[WARN]` message should still be emitted listing each skipped task by description and UUID
so the user knows why it was skipped, e.g.:

```
[WARN] Task "Buy milk" (uuid: abc-123) has no matching calendar — skipped
```

---

## Bug 2 — 403 error gives no indication of which URL failed

### Symptom

The error message is:

```
[ERROR] CalDAV request failed with status 403: Access to the requested resource forbidden.
```

No URL, no calendar name, no task description. The user has no way to know which resource
triggered the error.

### Root Cause

`CaldaWarriorError::CalDav` in `src/error.rs` (line 12) only stores `status: u16` and
`body: String`. At every call site in `src/caldav_adapter.rs` where a non-success HTTP status
is returned (`put_vtodo` lines 167–202, `delete_vtodo` lines 206–231, `fetch_single_vtodo`
lines 83–116, `list_vtodos` lines 136–159), the resolved URL is available locally but is
discarded when building the error.

### Affected Files

| File | Location |
|------|----------|
| `src/error.rs` | `CalDav` variant — missing `url` field |
| `src/caldav_adapter.rs` | All four HTTP call sites — need to pass URL into error |

### Required Fix

1. Add `url: String` to `CaldaWarriorError::CalDav` and update its `#[error(…)]` template:

   ```rust
   #[error("CalDAV request failed for {url} with status {status}: {body}")]
   CalDav { url: String, status: u16, body: String },
   ```

2. At each call site in `caldav_adapter.rs`, pass the resolved URL into the error constructor.

3. Result: error messages become, e.g.:

   ```
   [ERROR] CalDAV request failed for https://dav.example.com/alice/work/abc.ics with status 403: Access to the requested resource forbidden.
   ```

---

## Bug 3 — Task without description crashes the program

### Symptom

If a TaskWarrior task has no `description` field (which TaskWarrior itself permits), caldawarrior
panics / returns an error and aborts the entire sync, e.g.:

```
[ERROR] Failed to parse TW export: missing field `description` at line 1 column …
```

Every other task in the export is also skipped as a result.

### Root Cause

`TWTask` in `src/types.rs` (line 137) declares `description` as a plain `String` with no
`#[serde(default)]` attribute. When serde deserializes the JSON produced by `task export` and
encounters a task object with no `"description"` key, it treats it as a hard error and rejects
the entire JSON array.

The failure surfaces at `src/tw_adapter.rs` lines 221–223 and 228–230, where the two
`serde_json::from_str::<Vec<TWTask>>()` calls propagate the deserialization error as
`CaldaWarriorError::Config("Failed to parse TW export: …")`.

The downstream write path (`src/tw_adapter.rs` line 276) also does
`format!("description:{}", task.description)` without any guard, so a task reconstructed from
CalDAV with an empty summary would produce a `description:` argument that creates a task with a
literally empty description.

### Affected Files

| File | Location |
|------|----------|
| `src/types.rs` | `TWTask.description: String` — must become `Option<String>` or get `#[serde(default)]` |
| `src/tw_adapter.rs` | Lines 221–230 — deserialization failure site; line 276 — `create()` write-back |
| `src/output.rs` | `get_description()` (line 125) — already handles `Option` gracefully via `VTODO.summary` fallback, but needs updating for the type change |

### Required Fix

1. Change `TWTask.description` to `Option<String>` in `src/types.rs`.
2. Anywhere `task.description` is used as a plain `String`, handle the `None` case:
   - In `src/tw_adapter.rs` `create()`, skip or substitute a placeholder when description is absent.
   - In `src/output.rs` `get_description()`, fall through to the existing UUID/unknown fallback.
   - In `src/sync/writeback.rs` line 78–79, pass `description.as_deref().unwrap_or("")` (or omit the VTODO field entirely when absent).
3. A task with no description should be synced normally — the absence of a description is not an error.

---

## Bug 4 — CalDAV VTODO with no SUMMARY imported as empty task

### Symptom

A VTODO created by a third-party client (e.g. Tasks.org on Android) that has no `SUMMARY`
field — or where the field is not mapped to anything caldawarrior recognises — is imported into
TaskWarrior with no description, no project, and no meaningful fields. The resulting TW task is
effectively empty.

Example VTODO that triggers this:

```
BEGIN:VTODO
CREATED:20260309T111742Z
DTSTAMP:20260309T111751Z
LAST-MODIFIED:20260309T111748Z
PRIORITY:9
SUMMARY:Dada
UID:3762675243138060895
END:VTODO
```

Despite having `SUMMARY:Dada` and `PRIORITY:9`, the imported TW task has no description, no
project, and no priority. The calendar config maps the collection to `project = "testcal"` but
that project field is also absent.

### Root Cause (suspected)

The VTODO parser (`src/caldav_adapter.rs` and/or `src/types.rs`) likely fails to extract
`SUMMARY` from VTODOs produced by Tasks.org. Possible causes:

- Line endings or encoding differences (Tasks.org may emit CRLF; parser may only handle LF).
- The `SUMMARY` value is parsed but not propagated to the TW `description` field during
  CalDAV → TW mapping in `src/ir.rs` or `src/tw_adapter.rs`.
- The `project` field is not being injected from the matched `[[calendar]]` config entry when
  creating TW tasks from CalDAV.
- `PRIORITY:9` (lowest priority in iCalendar) may not be mapped at all.

### Affected Files (suspected)

| File | Location |
|------|----------|
| `src/caldav_adapter.rs` | VTODO → IR parsing — `SUMMARY`, `PRIORITY` extraction |
| `src/ir.rs` | CalDAV → IR → TW mapping — `project` injection from config |
| `src/tw_adapter.rs` | `create()` — fields written to `task add` command |

### Required Fix

1. Confirm that `SUMMARY` is correctly extracted from VTODOs produced by Tasks.org (check line
   endings, encoding, and ical parser behaviour).
2. Ensure the matched `[[calendar]]` `project` value is injected into the TW task when
   importing from CalDAV.
3. Map `PRIORITY` from iCalendar scale (1–9) to TaskWarrior priority (`H`/`M`/`L`).
4. Add a test scenario in the Robot Framework suite covering a CalDAV-originated task with
   `SUMMARY`, `PRIORITY`, and a mapped project.

---

## Observation 5 — Logs are too sparse for troubleshooting

### Symptom

Beyond the two bugs above, the only feedback during a sync is the final summary line. There is
no way to see which tasks are being processed, which CalDAV collections are being queried, or
why a task was skipped.

### What Would Help

- A `--verbose` / `-v` flag (or `CALDAWARRIOR_LOG=debug` env var) that prints per-task
  actions as they happen, e.g.:

  ```
  [INFO] Pushing task "Buy milk" → https://dav.example.com/alice/work/abc.ics
  [INFO] Skipping unchanged task "Read book" (no delta)
  ```

- Warnings for skipped tasks should include the task description and UUID, not just a generic
  category name.

### Affected Files (for future fix)

| File | Change needed |
|------|---------------|
| `src/main.rs` | Add `--verbose` / `-v` CLI flag |
| `src/output.rs` | Add verbose print helpers |
| `src/sync/writeback.rs` | Emit per-operation log lines when verbose is enabled |
