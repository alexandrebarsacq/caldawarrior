# Scenario Catalog — caldawarrior Blackbox Integration Tests

## Purpose

This catalog is the single source of truth for all Robot Framework blackbox integration test
scenarios for caldawarrior. It bridges user-facing requirements and the concrete `.robot` test
files by providing a traceability chain: user story → catalog entry → `.robot` test case name.

## Traceability Chain

```
User story (plain English)
    └── Catalog entry (this file)
            └── Robot Test Case Name (exact string used in .robot file)
                        └── Robot keyword calls (implemented in .robot file)
```

Every Robot test case name in a `.robot` file MUST appear verbatim as the **Robot Test Case Name**
field in this catalog. Changes to names must be made here first, then propagated to `.robot` files.

## Scenario ID Convention

IDs are assigned in named ranges. Gaps between ranges are intentional — they are reserved for
future scenarios without requiring renumbering.

| Range     | Category              |
|-----------|-----------------------|
| S-01–S-05 | First Sync            |
| S-10–S-14 | LWW Conflict          |
| S-20–S-25 | Orphan and Deletion   |
| S-30–S-38 | Status Mapping        |
| S-40–S-45 | Dependencies          |
| S-50–S-55 | CLI Behavior          |
| S-60–S-89 | Field Mapping         |
| S-90–S-95 | Idempotency           |

## How to Read This Catalog

Each scenario entry contains these fields:

- **ID**: Unique scenario identifier (S-XX).
- **Category**: The functional area this scenario belongs to.
- **Robot Test Case Name**: The exact test case name string to use in the `.robot` file.
- **skip-unimplemented**: `Yes` means the Robot test is tagged `skip-unimplemented` and will not
  run until the gap is closed. `No` means the test is expected to pass.
- **User Story**: Plain-language narrative readable by a non-developer.
- **Setup**: The state that must be established before running the sync.
- **Expected Stdout**: Exact output format (sourced from GAP_ANALYSIS.md). `*` denotes variable
  content that Robot Framework matches with a glob or regex.
- **Expected Stderr**: Exact format or `(empty)` if nothing is expected on stderr.
- **Exit Code**: Expected process exit code (0 = success, 1 = error).
- **Expected CalDAV State**: What should exist or not exist in CalDAV after sync.
- **Expected TW State**: What should exist or not exist in TW after sync.
- **Status**: ✅ Pass (test passes), ⚠️ Skip (tagged `skip-unimplemented`), ⚠️ No test (no robot file yet), ❌ Fail (failing with issue link).
- **Notes**: Caveats, source references, and skip-unimplemented rationale.

All output format strings are sourced from `tests/robot/docs/GAP_ANALYSIS.md` and traced to
`src/output.rs`, `src/main.rs`, `src/config.rs`, and related source files. No format string in
this catalog is guessed.

---

## First Sync (S-01 to S-05)

### S-01 · First Sync — Task Creation in CalDAV

| Field | Value |
|-------|-------|
| **ID** | S-01 |
| **Category** | First Sync |
| **Robot Test Case Name** | `First Sync Creates TW Task In CalDAV` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice has one task in TaskWarrior ("Buy milk") that has never been synced. Her CalDAV calendar is
empty. She runs `caldawarrior sync`. She expects the task to appear as a VTODO in her CalDAV
calendar after the sync completes.

**Setup**
- TW: 1 pending task ("Buy milk"), no `caldavuid` UDA set
- CalDAV: empty calendar

**Expected Stdout**
```
Synced: 1 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
1 VTODO exists in the calendar with SUMMARY matching "Buy milk"

**Expected TW State**
The TW task `caldavuid` UDA is non-empty (a UUID string)

**Notes**
Output format sourced from GAP_ANALYSIS.md §1.1. Corresponds to library test
`first_sync_pushes_tw_tasks_to_caldav` in `tests/integration/test_first_sync.rs`.

---

### S-02 · First Sync — caldavuid UDA Written Back to TW

| Field | Value |
|-------|-------|
| **ID** | S-02 |
| **Category** | First Sync |
| **Robot Test Case Name** | `First Sync Sets Caldavuid UDA On TW Task` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice runs `caldawarrior sync` for the first time. After the sync, she queries her TW task and
notices the `caldavuid` field is now populated with a UUID string. This tells her the task is
paired with a CalDAV VTODO and future syncs will keep them in step.

**Setup**
- TW: 1 pending task with no `caldavuid` UDA
- CalDAV: empty calendar

**Expected Stdout**
```
Synced: 1 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
1 VTODO created

**Expected TW State**
TW task `caldavuid` attribute is a non-empty 36-character UUID4 string

**Notes**
Output format sourced from GAP_ANALYSIS.md §1.1. Corresponds to library test
`first_sync_sets_caldavuid_uda_on_tw_task`. UDA is set via `task {uuid} modify caldavuid:{uid}`
as documented in GAP_ANALYSIS.md §3.

---

### S-03 · First Sync — Dry-Run Produces [DRY-RUN] Output Without Writing

| Field | Value |
|-------|-------|
| **ID** | S-03 |
| **Category** | First Sync |
| **Robot Test Case Name** | `First Sync Dry Run Does Not Write VTODOs` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice wants to preview what would happen on her first sync without actually making any changes.
She runs `caldawarrior sync --dry-run`. She sees a `[DRY-RUN] [CREATE]` line for each of her
tasks and a summary line, but no VTODOs appear in her CalDAV calendar afterward.

**Setup**
- TW: 1 pending task ("Buy milk"), no `caldavuid` UDA set
- CalDAV: empty calendar
- CLI invoked with `--dry-run` flag

**Expected Stdout**
```
[DRY-RUN] [CREATE] CalDAV <- TW: Buy milk
[DRY-RUN] Would: 1 create(s), 0 update(s), 0 delete(s), 0 skip(s)
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
0 VTODOs exist (no writes performed)

**Expected TW State**
TW task `caldavuid` UDA remains empty (no writeback performed)

**Notes**
Output formats sourced from GAP_ANALYSIS.md §1.3. Corresponds to library test
`first_sync_dry_run_does_not_write_vtodos`. The `[DRY-RUN] [CREATE] CalDAV <- TW: {description}`
format is from `src/output.rs:82-118` via `format_planned_op()`.

---

### S-04 · First Sync — Task Without Project Routes to Default Calendar

| Field | Value |
|-------|-------|
| **ID** | S-04 |
| **Category** | First Sync |
| **Robot Test Case Name** | `First Sync Routes Projectless Task To Default Calendar` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice has a TW task with no project set. Her caldawarrior config maps the `"default"` project to
her main CalDAV calendar URL. She runs `caldawarrior sync`. She expects the unprojectified task to
land in her default calendar, not be skipped.

**Setup**
- TW: 1 pending task with no `project` attribute and no `caldavuid`
- CalDAV: 1 calendar mapped to `project = "default"` in config; calendar is empty

**Expected Stdout**
```
Synced: 1 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
1 VTODO created in the default calendar

**Expected TW State**
TW task `caldavuid` UDA is non-empty

**Notes**
Output format sourced from GAP_ANALYSIS.md §1.1. Corresponds to library test
`first_sync_project_mapping_routes_to_default_calendar`. Routing logic is in `src/ir.rs:14-25`.

---

### S-05 · First Sync — Multiple Tasks Produce Correct Summary Counts

| Field | Value |
|-------|-------|
| **ID** | S-05 |
| **Category** | First Sync |
| **Robot Test Case Name** | `Five CalDAV VTODOs Created In TW On First Sync` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice has three pending tasks in TW that have never been synced. She runs `caldawarrior sync` and
sees the summary line report exactly 3 tasks created in CalDAV and 0 in TW. She then checks her
CalDAV calendar and finds all three VTODOs.

**Setup**
- TW: 3 pending tasks, none with `caldavuid` UDA set
- CalDAV: empty calendar

**Expected Stdout**
```
Synced: 3 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
3 VTODOs exist, each matching a TW task description

**Expected TW State**
All 3 TW tasks have `caldavuid` UDA set to non-empty UUID strings

**Notes**
Output format sourced from GAP_ANALYSIS.md §1.1. Tests that the summary counter
(`caldav_creates`) increments for each `PlannedOp::PushToCalDav` op.

---

## LWW Conflict (S-10 to S-14)

### S-10 · LWW Conflict — TW Wins, CalDAV Updated

| Field | Value |
|-------|-------|
| **ID** | S-10 |
| **Category** | LWW Conflict |
| **Robot Test Case Name** | `TW Wins LWW Conflict Resolution` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice has a paired task in both TW and CalDAV. She edits the TW task description, making its
modification timestamp newer than the CalDAV VTODO's LAST-MODIFIED. She runs `caldawarrior sync`.
She expects the CalDAV VTODO to be updated with her new description, and her TW task to remain
unchanged.

**Setup**
- TW: 1 paired task with `caldavuid` set; task modified more recently than CalDAV VTODO
- CalDAV: 1 VTODO with the original description; LAST-MODIFIED older than TW modified timestamp

**Expected Stdout**
```
Synced: 0 created, 1 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
VTODO SUMMARY updated to match the new TW task description

**Expected TW State**
TW task unchanged (description stays as Alice edited it)

**Notes**
Output format sourced from GAP_ANALYSIS.md §1.1. `caldav_updates` counter increments for
`PlannedOp::ResolveConflict { winner: Side::Tw }`. Corresponds to library test `tw_wins_lww`.
LWW logic is in `src/sync/lww.rs`.

---

### S-11 · LWW Conflict — CalDAV Wins, TW Updated

| Field | Value |
|-------|-------|
| **ID** | S-11 |
| **Category** | LWW Conflict |
| **Robot Test Case Name** | `CalDAV Wins LWW Conflict Resolution` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice edits a VTODO directly in her CalDAV client, making the VTODO's LAST-MODIFIED newer than
the TW task's modified timestamp. She runs `caldawarrior sync`. She expects the TW task
description to be updated to match the CalDAV VTODO, and the VTODO to remain as she edited it.

**Setup**
- TW: 1 paired task with `caldavuid` set; task NOT modified since last sync
- CalDAV: 1 VTODO edited externally; LAST-MODIFIED is newer than TW task modified timestamp

**Expected Stdout**
```
Synced: 0 created, 0 updated in CalDAV; 0 created, 1 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
VTODO unchanged (as Alice edited it in her CalDAV client)

**Expected TW State**
TW task description updated to match the CalDAV VTODO SUMMARY

**Notes**
Output format sourced from GAP_ANALYSIS.md §1.1. `tw_updates` counter increments for
`PlannedOp::ResolveConflict { winner: Side::CalDav }`. Corresponds to library test
`caldav_wins_lww`.

---

### S-12 · LWW Conflict — Re-Sync After CalDAV-Wins Produces Zero-Write Summary

| Field | Value |
|-------|-------|
| **ID** | S-12 |
| **Category** | LWW Conflict |
| **Robot Test Case Name** | `Immediate Re-Sync After Conflict Is Stable Point` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

After a sync where CalDAV won the conflict and TW was updated, Alice runs `caldawarrior sync`
again immediately. She expects no further writes to happen on either side — both data stores now
agree and the summary shows all zeros.

**Setup**
- TW: 1 paired task; TW description already matches CalDAV VTODO SUMMARY (post-conflict-resolution state)
- CalDAV: 1 VTODO; LAST-MODIFIED matches what was synced in previous run

**Expected Stdout**
```
Synced: 0 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
VTODO unchanged

**Expected TW State**
TW task unchanged

**Notes**
Output format sourced from GAP_ANALYSIS.md §1.2. This verifies loop prevention: the stable-point
condition produces `PlannedOp::Skip { reason: SkipReason::Identical }` which does not increment
any counter. Corresponds to library test `loop_prevention_stable_point`.

---

### S-13 · LWW Conflict — ETag 412 Handled Without Error

| Field | Value |
|-------|-------|
| **ID** | S-13 |
| **Category** | LWW Conflict |
| **Robot Test Case Name** | `ETag Conflict Is Handled Without Error` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice and her CalDAV client make concurrent edits. When caldawarrior tries to write the VTODO
update, the server returns HTTP 412 (Precondition Failed) because the ETag has changed. Alice
expects caldawarrior to handle this gracefully — no error message, exit code 0, and both sides
eventually reaching a consistent state.

**Setup**
- TW: 1 paired task modified after initial sync
- CalDAV: 1 VTODO also modified externally between the IR fetch and the PUT write (simulated via
  a test harness that modifies the VTODO between fetch and write)

**Expected Stdout**
```
Synced: 0 created, 1 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
VTODO exists and is consistent with one side winning the conflict

**Expected TW State**
TW task is consistent with the resolution

**Notes**
Output format sourced from GAP_ANALYSIS.md §1.1. ETag 412 handling is in
`src/caldav_adapter.rs:193-197` — it re-fetches the VTODO and retries. Corresponds to library
test `etag_conflict_scenario`. Note: this scenario may be difficult to exercise deterministically
as a CLI blackbox test; the Robot test may require harness-level cooperation to simulate a 412.

---

### S-14 · LWW Conflict — Dry-Run Shows [UPDATE] Lines Without Writing

| Field | Value |
|-------|-------|
| **ID** | S-14 |
| **Category** | LWW Conflict |
| **Robot Test Case Name** | `LWW Conflict Dry Run Shows Update Without Writing` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice wants to preview conflict resolution before committing. She runs `caldawarrior sync
--dry-run` when her TW task is newer than the CalDAV VTODO. She sees a `[DRY-RUN] [UPDATE]
Conflict resolved (TW wins)` line and a summary, but neither her TW task nor her CalDAV VTODO is
actually changed.

**Setup**
- TW: 1 paired task modified more recently than the CalDAV VTODO
- CalDAV: 1 VTODO with outdated content
- CLI invoked with `--dry-run` flag

**Expected Stdout**
```
[DRY-RUN] [UPDATE] Conflict resolved (TW wins): *
[DRY-RUN] Would: 0 create(s), 1 update(s), 0 delete(s), 0 skip(s)
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
VTODO unchanged (no write performed)

**Expected TW State**
TW task unchanged

**Notes**
Dry-run format sourced from GAP_ANALYSIS.md §1.3. The `[DRY-RUN] [UPDATE] Conflict resolved (TW
wins): {description}` line is produced by `format_planned_op()` in `src/output.rs:82-118`. The
`*` in Expected Stdout denotes the task description, which is matched with a glob in Robot.

---

## Orphan and Deletion (S-20 to S-25)

### S-20 · Orphan — TW Task with Orphaned caldavuid Is Deleted from TW

| Field | Value |
|-------|-------|
| **ID** | S-20 |
| **Category** | Orphan and Deletion |
| **Robot Test Case Name** | `Orphaned Caldavuid Causes TW Task Deletion` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice deleted a VTODO directly from her CalDAV client. Her TW task still has the `caldavuid` UDA
set, pointing to a VTODO that no longer exists. She runs `caldawarrior sync`. She expects the TW
task to be deleted because the CalDAV counterpart is gone — it should not be re-created in CalDAV.

**Setup**
- TW: 1 pending task with `caldavuid` set to a UID that no longer exists in CalDAV
- CalDAV: empty calendar (VTODO was deleted externally)

**Expected Stdout**
```
Synced: 0 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
Calendar remains empty; no VTODO is created

**Expected TW State**
TW task is deleted (does not appear in `task export`)

**Notes**
Output format sourced from GAP_ANALYSIS.md §1.2. Deletion ops (`PlannedOp::DeleteFromTw`) do not
appear in the summary counters — the summary correctly shows all zeros. Corresponds to library
test `orphaned_caldavuid_causes_tw_deletion`. Logic in `src/sync/writeback.rs:279-281`.

---

### S-21 · Deletion — CalDAV VTODO Deleted Externally Causes TW Task Deletion

| Field | Value |
|-------|-------|
| **ID** | S-21 |
| **Category** | Orphan and Deletion |
| **Robot Test Case Name** | `Externally Deleted CalDAV VTODO Causes TW Deletion` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice had a paired TW task and CalDAV VTODO. She deletes the VTODO from her CalDAV client
(simulating an external deletion). On the next `caldawarrior sync`, she expects the corresponding
TW task to also be removed, keeping both systems in sync.

**Setup**
- TW: 1 pending task with `caldavuid` set
- CalDAV: calendar is empty (VTODO deleted since last sync)

**Expected Stdout**
```
Synced: 0 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
Calendar remains empty

**Expected TW State**
TW task is deleted

**Notes**
Output format sourced from GAP_ANALYSIS.md §1.2. This scenario is functionally identical to S-20
from the sync engine's perspective — both present as an orphaned `caldavuid`. Included as a
separate scenario to document the distinct user story (deliberate deletion vs. unexpected
disappearance).

---

### S-22 · Deletion — Re-Sync After TW Deletion Produces Zero-Write Summary

| Field | Value |
|-------|-------|
| **ID** | S-22 |
| **Category** | Orphan and Deletion |
| **Robot Test Case Name** | `Re-Sync After Deletion Is Stable Point` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

After the previous sync deleted Alice's TW task because its CalDAV counterpart was gone, she runs
`caldawarrior sync` again. Both sides are now empty and consistent. She expects the summary to
show all zeros with no errors.

**Setup**
- TW: no tasks (empty)
- CalDAV: empty calendar

**Expected Stdout**
```
Synced: 0 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
Calendar remains empty

**Expected TW State**
No tasks present

**Notes**
Output format sourced from GAP_ANALYSIS.md §1.2. Verifies that an empty-vs-empty sync is a
stable point producing the zero-write summary.

---

### S-23 · Orphan — CalDAV CANCELLED VTODO Without TW Pair Does Not Create Ghost Task

| Field | Value |
|-------|-------|
| **ID** | S-23 |
| **Category** | Orphan and Deletion |
| **Robot Test Case Name** | `CalDAV Cancelled VTODO Without TW Pair Does Not Create Ghost Task` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

A CANCELLED VTODO exists on CalDAV but was never synced to TW (no TW pair). Running sync
should NOT create a TW task for it -- terminal CalDAV-only entries are skipped.

**Setup**
- TW: no tasks
- CalDAV: 1 VTODO with STATUS:CANCELLED, no paired TW task

**Expected Stdout**
```
Synced: 0 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
VTODO unchanged (still CANCELLED)

**Expected TW State**
No tasks present (0 tasks)

**Notes**
Verifies that CalDAV-only CANCELLED VTODOs do not produce ghost TW tasks. The
`SkipReason::Cancelled` branch handles this in writeback.rs.

---

### S-24 · Orphan — CalDAV COMPLETED VTODO Without TW Pair Does Not Create Task

| Field | Value |
|-------|-------|
| **ID** | S-24 |
| **Category** | Orphan and Deletion |
| **Robot Test Case Name** | `CalDAV Completed VTODO Without TW Pair Does Not Create Task` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

A COMPLETED VTODO exists on CalDAV but was never synced to TW. Running sync should NOT create
a TW task (CalDAV-only terminal entries are skipped).

**Setup**
- TW: no tasks
- CalDAV: 1 VTODO with STATUS:COMPLETED, no paired TW task

**Expected Stdout**
```
Synced: 0 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
VTODO unchanged (still COMPLETED)

**Expected TW State**
No tasks present (0 tasks)

**Notes**
Verifies that CalDAV-only COMPLETED VTODOs are skipped. The `SkipReason::Completed` branch
handles this in writeback.rs.

---

### S-25 · Orphan — CalDAV Completed And TW Completed Both Terminal Zero Writes

| Field | Value |
|-------|-------|
| **ID** | S-25 |
| **Category** | Orphan and Deletion |
| **Robot Test Case Name** | `CalDAV Completed And TW Completed Both Terminal Zero Writes` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice has a completed TW task paired with a COMPLETED CalDAV VTODO. Running sync should produce
zero writes because both sides are terminal and identical.

**Setup**
- TW: 1 completed task with caldavuid set
- CalDAV: 1 VTODO with STATUS:COMPLETED (from previous sync)

**Expected Stdout**
```
Synced: 0 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
VTODO unchanged (COMPLETED)

**Expected TW State**
Task still completed

**Notes**
Verifies that both-terminal-completed is a stable point producing zero writes.

---

## Status Mapping (S-30 to S-38)

### S-30 · Status Mapping — CalDAV COMPLETED Syncs to TW Completed

| Field | Value |
|-------|-------|
| **ID** | S-30 |
| **Category** | Status Mapping |
| **Robot Test Case Name** | `CalDAV Completed Status Syncs To TW Completed` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice marks a VTODO as COMPLETED in her CalDAV client. She runs `caldawarrior sync`. She then
checks her TW task list and finds the task has been marked as completed in TW, matching what she
did in CalDAV.

**Setup**
- TW: 1 paired pending task with `caldavuid` set
- CalDAV: 1 VTODO with `STATUS:COMPLETED` set; LAST-MODIFIED newer than TW task modified

**Expected Stdout**
```
Synced: 0 created, 0 updated in CalDAV; 0 created, 1 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
VTODO unchanged (still COMPLETED)

**Expected TW State**
TW task status is `completed`

**Notes**
Output format sourced from GAP_ANALYSIS.md §1.1. Corresponds to library test
`status_sync_caldav_completed_to_tw` in `tests/integration/test_scenarios.rs`. Status mapping is
in `src/mapper/status.rs`.

---

### S-31 · Status Mapping — TW Done Syncs COMPLETED to CalDAV

| Field | Value |
|-------|-------|
| **ID** | S-31 |
| **Category** | Status Mapping |
| **Robot Test Case Name** | `TW Completed Status Syncs To CalDAV Completed` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice marks a TW task as done (`task done`). She runs `caldawarrior sync`. She expects the
corresponding CalDAV VTODO's STATUS property to be updated to `COMPLETED`, reflecting the
completion in her CalDAV client.

**Setup**
- TW: 1 paired task with `caldavuid` set; task status is `completed`; modified more recently than
  CalDAV VTODO
- CalDAV: 1 VTODO with `STATUS:NEEDS-ACTION`; LAST-MODIFIED older than TW task modified

**Expected Stdout**
```
Synced: 0 created, 1 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
VTODO `STATUS` property is `COMPLETED`

**Expected TW State**
TW task remains completed

**Notes**
Output format sourced from GAP_ANALYSIS.md §1.1. TW wins LWW (newer modified); CalDAV is updated.
Status mapping for TW→CalDAV direction is in `src/mapper/status.rs`.

---

### S-32 · Status Mapping — Pending TW Task Stays Pending with NEEDS-ACTION VTODO

| Field | Value |
|-------|-------|
| **ID** | S-32 |
| **Category** | Status Mapping |
| **Robot Test Case Name** | `Pending TW Task Stays Pending With Needs-Action VTODO` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice has a paired TW task that is pending and a matching CalDAV VTODO with `STATUS:NEEDS-ACTION`.
Neither side has been modified since the last sync. She runs `caldawarrior sync`. She expects no
changes — both sides should remain as-is with a zero-write summary.

**Setup**
- TW: 1 paired pending task; not modified since last sync
- CalDAV: 1 VTODO with `STATUS:NEEDS-ACTION`; not modified since last sync

**Expected Stdout**
```
Synced: 0 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
VTODO `STATUS` remains `NEEDS-ACTION`

**Expected TW State**
TW task status remains `pending`

**Notes**
Output format sourced from GAP_ANALYSIS.md §1.2. Tests the stable-point case for status mapping:
both sides agree, no changes needed.

---

### S-33 · Status Mapping — Completed Task Within Cutoff Is Synced; Beyond Cutoff Is Not

| Field | Value |
|-------|-------|
| **ID** | S-33 |
| **Category** | Status Mapping |
| **Robot Test Case Name** | `Completed Task Within Cutoff Is Synced Beyond Is Not` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice has two completed TW tasks: one completed 30 days ago (within the default 90-day cutoff)
and one completed 200 days ago (beyond the cutoff). She runs `caldawarrior sync`. She expects
the recently completed task to appear in CalDAV but the old one to be ignored entirely.

**Setup**
- TW: 2 completed tasks; one completed 30 days ago (within cutoff), one completed 200 days ago
  (beyond `completed_cutoff_days = 90`); neither has `caldavuid`
- CalDAV: empty calendar
- Config: `completed_cutoff_days = 90` (default)

**Expected Stdout**
```
Synced: 1 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
1 VTODO exists (for the recently completed task only)

**Expected TW State**
Recently completed task has `caldavuid` set; old completed task has no `caldavuid`

**Notes**
Output format sourced from GAP_ANALYSIS.md §1.1. `completed_cutoff_days` is documented in
GAP_ANALYSIS.md §2.2. Tasks beyond the cutoff generate `PlannedOp::Skip` and are not counted
in the summary.

---

### S-34 · Status Mapping — CalDAV Reopen Completed VTODO Syncs To TW Pending

| Field | Value |
|-------|-------|
| **ID** | S-34 |
| **Category** | Status Mapping |
| **Robot Test Case Name** | `CalDAV Reopen Completed VTODO Syncs To TW Pending` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice completes a TW task (synced to CalDAV COMPLETED), then her colleague reopens the VTODO
in CalDAV by setting STATUS to NEEDS-ACTION. Alice runs sync and expects the TW task to be
back to pending.

**Setup**
- TW: 1 completed task with caldavuid set
- CalDAV: 1 VTODO with STATUS:NEEDS-ACTION (reopened), LAST-MODIFIED newer than TW

**Expected Stdout**
Sync summary showing 1 updated in TW

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
VTODO has STATUS:NEEDS-ACTION, no COMPLETED property

**Expected TW State**
Task is pending

**Notes**
Tests bidirectional reopen from CalDAV side. COMPLETED timestamp must be cleared when
reopening, otherwise it would confuse future syncs.

---

### S-35 · Status Mapping — TW Reopen Completed Task Syncs To CalDAV Needs-Action

| Field | Value |
|-------|-------|
| **ID** | S-35 |
| **Category** | Status Mapping |
| **Robot Test Case Name** | `TW Reopen Completed Task Syncs To CalDAV Needs-Action` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice marks a TW task as done, syncs (CalDAV COMPLETED), then modifies the task in TW
(making it pending again). Sync should update CalDAV to NEEDS-ACTION and remove the COMPLETED
timestamp.

**Setup**
- TW: 1 pending task (reopened from completed) with caldavuid set
- CalDAV: 1 VTODO with STATUS:COMPLETED (from previous sync)

**Expected Stdout**
Sync summary showing 1 updated in CalDAV

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
VTODO has STATUS:NEEDS-ACTION, no COMPLETED property

**Expected TW State**
Task is pending

**Notes**
Tests bidirectional reopen from TW side. Verifies that COMPLETED timestamp is cleared in
CalDAV when task is reopened from TW.

---

### S-36 · Status Mapping — TW Delete Syncs To CalDAV Cancelled

| Field | Value |
|-------|-------|
| **ID** | S-36 |
| **Category** | Status Mapping |
| **Robot Test Case Name** | `TW Delete Syncs To CalDAV Cancelled` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice deletes a TW task that was previously synced to CalDAV. After sync the CalDAV VTODO
should have STATUS:CANCELLED.

**Setup**
- TW: 1 deleted task with caldavuid set
- CalDAV: 1 VTODO with STATUS:NEEDS-ACTION (from previous sync)

**Expected Stdout**
Sync summary showing 1 updated in CalDAV

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
VTODO has STATUS:CANCELLED

**Expected TW State**
Task is deleted

**Notes**
Verifies TW delete propagation to CalDAV. The `UpdateReason::TwDeletedMarkCancelled` branch
handles this.

---

### S-37 · Status Mapping — CalDAV CANCELLED Syncs To TW Deleted

| Field | Value |
|-------|-------|
| **ID** | S-37 |
| **Category** | Status Mapping |
| **Robot Test Case Name** | `CalDAV Cancelled Syncs To TW Deleted` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice's colleague cancels a CalDAV VTODO (STATUS:CANCELLED). Alice runs sync and expects the
paired TW task to be deleted. This is the fix for the CANCELLED propagation asymmetry.

**Setup**
- TW: 1 pending task with caldavuid set
- CalDAV: 1 VTODO with STATUS:CANCELLED, LAST-MODIFIED newer than TW

**Expected Stdout**
Sync summary showing deletion

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
VTODO unchanged (STATUS:CANCELLED)

**Expected TW State**
Task is deleted

**Notes**
This is the key scenario for the CANCELLED propagation fix (writeback.rs). Before the fix,
CalDAV CANCELLED + TW active produced `SkipReason::Cancelled` instead of
`PlannedOp::DeleteFromTw`.

---

### S-38 · Status Mapping — Both Sides Deleted And Cancelled Produces Zero Writes

| Field | Value |
|-------|-------|
| **ID** | S-38 |
| **Category** | Status Mapping |
| **Robot Test Case Name** | `Both Sides Deleted And Cancelled Produces Zero Writes` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice has a TW task marked deleted and the paired CalDAV VTODO is STATUS:CANCELLED. Running
sync should produce zero writes (both terminal).

**Setup**
- TW: 1 deleted task with caldavuid set
- CalDAV: 1 VTODO with STATUS:CANCELLED (from previous delete sync)

**Expected Stdout**
```
Synced: 0 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
VTODO unchanged (CANCELLED)

**Expected TW State**
Task still deleted

**Notes**
Verifies that TW deleted + CalDAV CANCELLED is a stable point. The
`SkipReason::CalDavDeletedTwTerminal` branch handles this (TW deleted is a terminal state).

---

## Dependencies (S-40 to S-45)

### S-40 · Dependencies — TW `depends` Syncs to RELATED-TO in CalDAV

| Field | Value |
|-------|-------|
| **ID** | S-40 |
| **Category** | Dependencies |
| **Robot Test Case Name** | `TW Depends Syncs To CalDAV Related-To` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice has two TW tasks: "Buy groceries" depends on "Go to the store". She runs `caldawarrior
sync`. She then inspects the CalDAV VTODO for "Buy groceries" and finds a
`RELATED-TO;RELTYPE=DEPENDS-ON` property pointing to the UID of the "Go to the store" VTODO.

**Setup**
- TW: 2 pending tasks; task A has `depends` set to the UUID of task B; neither has `caldavuid`
- CalDAV: empty calendar

**Expected Stdout**
```
Synced: 2 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
2 VTODOs exist; VTODO for task A has `RELATED-TO;RELTYPE=DEPENDS-ON:{caldav_uid_of_B}` property

**Expected TW State**
Both TW tasks have `caldavuid` set

**Notes**
Output format sourced from GAP_ANALYSIS.md §1.1. Dependency forward mapping is documented in
GAP_ANALYSIS.md §4. Corresponds to library test `dependency_sync_tw_to_caldav` in
`tests/integration/test_scenarios.rs`. Source: `src/mapper/fields.rs:77-79`.

---

### S-41 · Dependencies — CalDAV RELATED-TO Syncs to TW `depends` Field

| Field | Value |
|-------|-------|
| **ID** | S-41 |
| **Category** | Dependencies |
| **Robot Test Case Name** | `CalDAV Related-To Syncs To TW Depends` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice has two paired CalDAV VTODOs where VTODO A has a `RELATED-TO;RELTYPE=DEPENDS-ON` property
pointing to the UID of VTODO B. She modifies VTODO A in her CalDAV client and re-runs
`caldawarrior sync`. She checks TW and finds that the TW task for A now has the `depends` field
pointing to the UUID of the TW task paired with B.

**Setup**
- TW: 2 paired pending tasks with `caldavuid` set; TW task A does NOT have `depends` set
- CalDAV: 2 VTODOs; VTODO A has `RELATED-TO;RELTYPE=DEPENDS-ON:{uid_of_B}` added externally;
  VTODO A LAST-MODIFIED newer than TW task A modified

**Expected Stdout**
```
Synced: 0 created, 0 updated in CalDAV; 0 created, 1 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
Both VTODOs unchanged

**Expected TW State**
TW task A has `depends` set to the UUID of TW task B

**Notes**
Output format sourced from GAP_ANALYSIS.md §1.1. Reverse mapping documented in GAP_ANALYSIS.md
§4. Source: `src/sync/writeback.rs:146-152`. Note: reverse mapping only works when both VTODOs
are present in the IR (both TW tasks are being synced).

---

### S-42 · Dependencies — Cyclic Tasks Synced Without RELATED-TO

| Field | Value |
|-------|-------|
| **ID** | S-42 |
| **Category** | Dependencies |
| **Robot Test Case Name** | `Cyclic Tasks Synced Without Related-To` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice creates a 2-node cyclic dependency: task A depends on task B, and task B depends on task A.
She runs `caldawarrior sync`. Both tasks sync to CalDAV with SUMMARY and STATUS, but neither has
RELATED-TO properties. Stderr contains CyclicEntry warnings for both tasks. Exit code is 0.

**Setup**
- TW: 2 pending tasks; task A `depends` on task B; task B `depends` on task A (cycle created via
  `task import` to bypass TW's built-in cycle rejection)
- CalDAV: empty calendar

**Expected Stdout**
```
Synced: 2 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
```
[WARN] CyclicEntry: task '*' is part of a dependency cycle
[WARN] CyclicEntry: task '*' is part of a dependency cycle
```

**Exit Code**
0

**Expected CalDAV State**
2 VTODOs exist; both have SUMMARY and STATUS set; neither has `RELATED-TO` property

**Expected TW State**
Both TW tasks have `caldavuid` set

**Notes**
Cyclic entries are detected by `resolve_dependencies()` DFS in `src/sync/deps.rs`.
`resolved_depends` is cleared for cyclic entries in `apply_entry()` (src/sync/writeback.rs),
so `build_vtodo_from_tw()` produces VTODOs without RELATED-TO. Warning format:
`[WARN] CyclicEntry: task '{description}' is part of a dependency cycle`. `*` in Expected Stderr
denotes the task description matched as a glob. TW 3.x rejects cyclic dependencies at modify
time, so the test uses `task import` to bypass validation.

---

### S-43 · Dependencies — Three-Node Cyclic Dependency Synced Without RELATED-TO

| Field | Value |
|-------|-------|
| **ID** | S-43 |
| **Category** | Dependencies |
| **Robot Test Case Name** | `Three-Node Cyclic Dependency Synced Without Related-To` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice creates three tasks in a cycle: A depends on B, B depends on C, C depends on A. She runs
`caldawarrior sync`. All three tasks sync to CalDAV with their fields but without any RELATED-TO
properties. Stderr has 3 CyclicEntry warnings.

**Setup**
- TW: 3 pending tasks; A depends B, B depends C, C depends A (cycle created via `task import`
  for the cycle-closing edge)
- CalDAV: empty calendar

**Expected Stdout**
```
Synced: 3 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
```
[WARN] CyclicEntry: task '*' is part of a dependency cycle
[WARN] CyclicEntry: task '*' is part of a dependency cycle
[WARN] CyclicEntry: task '*' is part of a dependency cycle
```

**Exit Code**
0

**Expected CalDAV State**
3 VTODOs exist; all have SUMMARY set; none has `RELATED-TO` property

**Expected TW State**
All three TW tasks have `caldavuid` set

**Notes**
Tests the DFS cycle detection with a 3-node cycle (A->B->C->A). The cycle-closing edge (C
depends A) is set via `task import` to bypass TW 3.x cycle rejection.

---

### S-44 · Dependencies — TW Blocks Field Reflects Inverse Dependency After Sync

| Field | Value |
|-------|-------|
| **ID** | S-44 |
| **Category** | Dependencies |
| **Robot Test Case Name** | `TW Blocks Field Reflects Inverse Dependency After Sync` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice sets task A depends on task B. After sync, only A's VTODO has RELATED-TO (B's does not).
In TW, B's export shows A in its computed blocks relationship (verified by checking that A's
depends field contains B's UUID).

**Setup**
- TW: 2 pending tasks; task A `depends` on task B
- CalDAV: empty calendar

**Expected Stdout**
```
Synced: 2 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
2 VTODOs exist; only A's VTODO has `RELATED-TO;RELTYPE=DEPENDS-ON` pointing to B's UID;
B's VTODO has no `RELATED-TO` property

**Expected TW State**
Task A has `depends` containing B's UUID; B's computed `blocks` relationship includes A

**Notes**
TW `blocks` is a computed inverse of `depends` — not a stored field. caldawarrior does NOT write
a separate RELATED-TO for the blocks direction. TW 3.x does not include `blocks` in `task export`
JSON, so the test verifies the inverse relationship by checking A's depends field contains B's
UUID.

---

### S-45 · Dependencies — Removing TW Dependency Clears CalDAV RELATED-TO

| Field | Value |
|-------|-------|
| **ID** | S-45 |
| **Category** | Dependencies |
| **Robot Test Case Name** | `Removing TW Dependency Clears CalDAV Related-To` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice has task A depending on task B (synced). She removes the dependency by modifying TW task A
to have no depends, then re-syncs. The VTODO for A no longer has a RELATED-TO property.

**Setup**
- TW: 2 pending tasks; task A `depends` on task B; both already synced to CalDAV
- CalDAV: 2 VTODOs; A's VTODO has `RELATED-TO;RELTYPE=DEPENDS-ON`

**Expected Stdout** (second sync)
```
Synced: 0 created, 1 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
A's VTODO no longer has `RELATED-TO` property

**Expected TW State**
Task A no longer has `depends` field

**Notes**
Dependency removal works because `build_vtodo_from_tw()` rebuilds the VTODO from scratch using
`resolved_depends` (which is now empty), and the CalDAV PUT replaces the existing VTODO entirely.
No special code needed for dep removal.

---

## CLI Behavior (S-50 to S-55)

> **Note on skip-unimplemented scenarios (S-50 to S-53):** These behaviors are implemented in
> the caldawarrior library but have never been exercised via the CLI binary subprocess. The Robot
> Framework tests for these scenarios are tagged `skip-unimplemented` and will be skipped during
> test runs until the implementation gap is closed and the tag is removed. See GAP_ANALYSIS.md §6
> for the full rationale.

---

### S-50 · CLI Behavior — Invalid Credentials Produce Auth Error and Exit Code 1

| Field | Value |
|-------|-------|
| **ID** | S-50 |
| **Category** | CLI Behavior |
| **Robot Test Case Name** | `Invalid Credentials Produce Auth Error And Exit Code 1` |
| **skip-unimplemented** | Yes |
| **Status** | ⚠️ Skip |

**User Story**

Alice misconfigures her caldawarrior config with a wrong password. She runs `caldawarrior sync`.
She expects an error message on stderr explaining the authentication failure and the process to
exit with code 1.

**Setup**
- Config: valid server URL, valid username, wrong password
- CalDAV: server running and reachable

**Expected Stdout**
(empty)

**Expected Stderr**
```
Error: Failed to list VTODOs from calendar '*': Authentication failed for *: check your credentials in the config file
```

**Exit Code**
1

**Expected CalDAV State**
No changes (sync failed before writing)

**Expected TW State**
No changes

**Notes**
**skip-unimplemented: Yes** — Auth error path is implemented (`CaldaWarriorError::Auth` raised on
HTTP 401 in `src/caldav_adapter.rs:151,192,221`; `main.rs` exits with code 1) but not covered by
any CLI-level test. Error format sourced from GAP_ANALYSIS.md §1.5. `*` in Expected Stderr
denotes the calendar URL and server URL, matched as globs.

---

### S-51 · CLI Behavior — World-Readable Config Produces Permission Warning

| Field | Value |
|-------|-------|
| **ID** | S-51 |
| **Category** | CLI Behavior |
| **Robot Test Case Name** | `World Readable Config File Produces Permission Warning` |
| **skip-unimplemented** | Yes |
| **Status** | ⚠️ Skip |

**User Story**

Alice accidentally creates her config file with world-readable permissions (mode 0644). She runs
`caldawarrior sync`. She expects to see a `[WARN]` line on stderr warning her about the insecure
file permissions, while the sync itself proceeds normally.

**Setup**
- Config file: valid config written with permissions `0644` (world-readable)
- CalDAV: empty calendar
- TW: no tasks

**Expected Stdout**
```
Synced: 0 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
```
[WARN] Config file * has permissions 0644 — recommended: 0600
```

**Exit Code**
0

**Expected CalDAV State**
No changes

**Expected TW State**
No changes

**Notes**
**skip-unimplemented: Yes** — Permission check is implemented in `src/config.rs:88-98` (Unix
only, `#[cfg(unix)]`) and emits via `eprintln!` directly (not through `print_result()`). Warning
format sourced from GAP_ANALYSIS.md §1.4 full `eprintln!` call: `[WARN] Config file {:?} has
permissions {:04o} — recommended: 0600`. The `*` in Expected Stderr matches the quoted config
file path produced by the `{:?}` formatter.

---

### S-52 · CLI Behavior — TW Recurring Task Is Skipped with [WARN] Message

| Field | Value |
|-------|-------|
| **ID** | S-52 |
| **Category** | CLI Behavior |
| **Robot Test Case Name** | `TW Recurring Task Is Skipped With Warn Message` |
| **skip-unimplemented** | Yes |
| **Status** | ⚠️ Skip |

**User Story**

Alice has a recurring TW task (e.g., "Weekly report", recur:weekly). She runs `caldawarrior
sync`. She expects to see a `[WARN]` line on stderr noting the task was skipped, and the task
should not appear in CalDAV.

**Setup**
- TW: 1 recurring task (status `recurring`, recur value set)
- CalDAV: empty calendar

**Expected Stdout**
```
Synced: 0 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
```
[WARN] [*] recurring task * skipped (recur: *)
```

**Exit Code**
0

**Expected CalDAV State**
No VTODOs created (recurring task skipped)

**Expected TW State**
TW recurring task unchanged; no `caldavuid` set

**Notes**
**skip-unimplemented: Yes** — Skip logic is implemented in `src/mapper/status.rs:45-51`; the
warning is emitted via `print_result()` as `[WARN] [{uuid}] recurring task {uuid} skipped
(recur: {recur_value:?})`. No CLI-level test covers this path. Warning format sourced from
GAP_ANALYSIS.md §1.4. `*` in Expected Stderr matches the UUID and recur value.

---

### S-53 · CLI Behavior — CalDAV VTODO with RRULE Is Skipped with [WARN] Message

| Field | Value |
|-------|-------|
| **ID** | S-53 |
| **Category** | CLI Behavior |
| **Robot Test Case Name** | `CalDAV Recurring VTODO Is Skipped With Warn Message` |
| **skip-unimplemented** | Yes |
| **Status** | ⚠️ Skip |

**User Story**

Alice's CalDAV calendar contains a recurring event represented as a VTODO with an `RRULE`
property (e.g., a weekly meeting task). She runs `caldawarrior sync`. She expects to see a
`[WARN]` line on stderr noting the VTODO was skipped, and no corresponding TW task to be created.

**Setup**
- TW: no tasks
- CalDAV: 1 VTODO with `RRULE` property set

**Expected Stdout**
```
Synced: 0 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
```
[WARN] RecurringCalDavSkipped: VTODO '*' has RRULE and will not be synced
```

**Exit Code**
0

**Expected CalDAV State**
VTODO unchanged

**Expected TW State**
No TW tasks created

**Notes**
**skip-unimplemented: Yes** — Skip logic is implemented in `src/ir.rs:58-66`; the warning message
is `RecurringCalDavSkipped: VTODO '{uid}' has RRULE and will not be synced`. No CLI-level test
covers this path. Warning format sourced from GAP_ANALYSIS.md §1.4. `*` in Expected Stderr
matches the VTODO UID.

---

### S-54 · CLI Behavior — --dry-run Flag Enables Dry-Run Mode

| Field | Value |
|-------|-------|
| **ID** | S-54 |
| **Category** | CLI Behavior |
| **Robot Test Case Name** | `Dry Run Flag Enables Dry Run Mode` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice wants to preview what would happen if she synced her two TW tasks without actually changing
anything. She runs `caldawarrior sync --dry-run`. She sees `[DRY-RUN]` prefixed lines listing
the planned operations and a summary line, but nothing is written to CalDAV or TW.

**Setup**
- TW: 2 pending tasks with no `caldavuid`
- CalDAV: empty calendar
- CLI invoked with `--dry-run` flag

**Expected Stdout**
```
[DRY-RUN] [CREATE] CalDAV <- TW: *
[DRY-RUN] [CREATE] CalDAV <- TW: *
[DRY-RUN] Would: 2 create(s), 0 update(s), 0 delete(s), 0 skip(s)
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
0 VTODOs (no writes performed)

**Expected TW State**
Neither TW task has `caldavuid` set

**Notes**
Dry-run output formats sourced from GAP_ANALYSIS.md §1.3. Tests the `--dry-run` CLI flag
end-to-end as a subprocess invocation. `*` in Expected Stdout matches the task description.

---

### S-55 · CLI Behavior — Missing Config File Produces Fatal Error and Exit Code 1

| Field | Value |
|-------|-------|
| **ID** | S-55 |
| **Category** | CLI Behavior |
| **Robot Test Case Name** | `Missing Config File Produces Fatal Error And Exit Code 1` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice runs `caldawarrior sync` but has not created a config file yet. She expects an error
message on stderr telling her what went wrong and the process to exit with code 1.

**Setup**
- No config file exists at the default path (`~/.config/caldawarrior/config.toml`)
- `--config` flag not passed; `CALDAWARRIOR_CONFIG` env var not set

**Expected Stdout**
(empty)

**Expected Stderr**
```
Error: *
```

**Exit Code**
1

**Expected CalDAV State**
N/A (sync never started)

**Expected TW State**
N/A (sync never started)

**Notes**
Error format sourced from GAP_ANALYSIS.md §1.5: fatal errors before `print_result()` are printed
as `Error: {anyhow_error_chain}` by the `if let Err(e)` handler in `src/main.rs:31-34`. Config
path resolution order documented in GAP_ANALYSIS.md §2 (§2 config path section). `*` in Expected
Stderr matches the anyhow error chain which will include a filesystem not-found message.

---

## Field Mapping (S-60 to S-89)

### S-60 · Field Mapping — TW Description Syncs to VTODO SUMMARY

| Field | Value |
|-------|-------|
| **ID** | S-60 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `TW Description Syncs To VTODO Summary` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice creates a TW task with description "Pick up dry cleaning". She runs `caldawarrior sync`.
She checks the resulting VTODO in CalDAV and finds the SUMMARY property is exactly "Pick up dry
cleaning", matching her TW task description.

**Setup**
- TW: 1 pending task with description "Pick up dry cleaning", no `caldavuid`
- CalDAV: empty calendar

**Expected Stdout**
```
Synced: 1 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
1 VTODO with `SUMMARY:Pick up dry cleaning`

**Expected TW State**
TW task `caldavuid` UDA is non-empty

**Notes**
Output format sourced from GAP_ANALYSIS.md §1.1. Tests the TW description → CalDAV SUMMARY
field mapping in `src/mapper/fields.rs`.

---

### S-61 · Field Mapping — TW Due Date Syncs to VTODO DUE Property

| Field | Value |
|-------|-------|
| **ID** | S-61 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `TW Due Date Syncs To VTODO DUE Property` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice creates a TW task with a due date of tomorrow. She runs `caldawarrior sync`. She checks
the VTODO in CalDAV and finds a `DUE` property whose date matches the TW task's due date.

**Setup**
- TW: 1 pending task with `due` date set to tomorrow's date, no `caldavuid`
- CalDAV: empty calendar

**Expected Stdout**
```
Synced: 1 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
1 VTODO with `DUE` property matching the TW task's due date

**Expected TW State**
TW task `caldavuid` UDA is non-empty

**Notes**
Output format sourced from GAP_ANALYSIS.md §1.1. Tests the TW `due` date → CalDAV `DUE`
property mapping in `src/mapper/fields.rs`.

---

### S-62 · Field Mapping — CalDAV VTODO SUMMARY Syncs to TW Description

| Field | Value |
|-------|-------|
| **ID** | S-62 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `CalDAV VTODO Summary Syncs To TW Description` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice edits the SUMMARY of a VTODO directly in her CalDAV client, changing it from "Old
description" to "New description". She runs `caldawarrior sync`. She checks TW and finds the
task description has been updated to "New description".

**Setup**
- TW: 1 paired pending task with description "Old description" and `caldavuid` set; not modified
  since last sync
- CalDAV: 1 VTODO with `SUMMARY:New description`; LAST-MODIFIED newer than TW task modified

**Expected Stdout**
```
Synced: 0 created, 0 updated in CalDAV; 0 created, 1 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
VTODO SUMMARY remains "New description"

**Expected TW State**
TW task description is "New description"

**Notes**
Output format sourced from GAP_ANALYSIS.md §1.1. CalDAV wins LWW; TW task description is
updated from CalDAV VTODO SUMMARY. Tests the reverse field mapping path in
`src/sync/writeback.rs`.

---

### S-63 · Field Mapping — caldavuid UDA Stores CalDAV UID as UUID4 String

| Field | Value |
|-------|-------|
| **ID** | S-63 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `Caldavuid UDA Stores CalDAV UID As UUID4 String` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

After Alice runs `caldawarrior sync` for the first time, she inspects the `caldavuid` field on
her TW task. She finds it contains a UUID4 string (36 characters, hyphen-separated hex groups in
the 8-4-4-4-12 format), which matches the UID of the VTODO in her CalDAV calendar.

**Setup**
- TW: 1 pending task with no `caldavuid`, no `caldavuid` UDA registered yet
- CalDAV: empty calendar

**Expected Stdout**
```
Synced: 1 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
1 VTODO with a UID property that is a UUID4 string matching the TW task `caldavuid`

**Expected TW State**
TW task `caldavuid` UDA is a 36-character UUID4 string matching the CalDAV VTODO UID

**Notes**
Output format sourced from GAP_ANALYSIS.md §1.1. The `caldavuid` UDA key name, type, and label
are documented in GAP_ANALYSIS.md §3: key `caldavuid`, type `string`, label `CaldavUID`. The UDA
is auto-registered via `task config uda.caldavuid.type string` and `task config
uda.caldavuid.label CaldavUID` on every run (`src/tw_adapter.rs:187-193`). UUID4 format is
verified by the Robot test using a regex pattern: `[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}`.

---

### S-64 · Field Mapping — CalDAV SUMMARY-only VTODO Creates TW Task With Matching Description

| Field | Value |
|-------|-------|
| **ID** | S-64 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `CalDAV SUMMARY-only VTODO Creates TW Task With Matching Description` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice creates a CalDAV VTODO with only `SUMMARY` set (no `DESCRIPTION` line). She runs
`caldawarrior sync` and finds a TW task was created with `description` equal to the SUMMARY value.

**Setup**
- TW: empty
- CalDAV: 1 VTODO with `SUMMARY:Buy oat milk`, no `DESCRIPTION` property

**Expected Stdout**
```
Synced: 0 created, 0 updated in CalDAV; 1 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected TW State**
1 pending task with `description == "Buy oat milk"` and `caldavuid` set to the VTODO UID

**Notes**
Verifies that when no DESCRIPTION is present the SUMMARY is used as-is for the TW description.

---

### S-65 · Field Mapping — CalDAV DESCRIPTION-only VTODO Creates TW Task With Sentinel And Annotation

| Field | Value |
|-------|-------|
| **ID** | S-65 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `CalDAV DESCRIPTION-only VTODO Creates TW Task With Sentinel And Annotation` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice creates a CalDAV VTODO with `DESCRIPTION` set but no `SUMMARY` line at all. She runs
`caldawarrior sync` and finds a TW task created with description `"(no title)"` and an annotation
containing the DESCRIPTION text.

**Setup**
- TW: empty
- CalDAV: 1 VTODO with `DESCRIPTION:A note about milk`, no `SUMMARY` property

**Expected Stdout**
```
Synced: 0 created, 0 updated in CalDAV; 1 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected TW State**
1 pending task with `description == "(no title)"` and at least one annotation with description
`"A note about milk"`

**Notes**
When SUMMARY is absent, caldawarrior uses a sentinel title to keep TW happy (tasks must have a
description). The actual content is preserved as an annotation.

---

### S-66 · Field Mapping — TW Task With Annotation Syncs To CalDAV With SUMMARY And DESCRIPTION

| Field | Value |
|-------|-------|
| **ID** | S-66 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `TW Task With Annotation Syncs To CalDAV With SUMMARY And DESCRIPTION` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice creates a TW task with description `"Pick up groceries"` and adds an annotation
`"Don't forget the milk"`. After sync the CalDAV VTODO has `SUMMARY` equal to the TW description
and `DESCRIPTION` equal to the annotation text — confirming they are not duplicated.

**Setup**
- TW: 1 pending task with description `"Pick up groceries"` and annotation `"Don't forget the milk"`
- CalDAV: empty

**Expected Stdout**
```
Synced: 1 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
1 VTODO with `SUMMARY:Pick up groceries`, `DESCRIPTION:Don't forget the milk`, and
`SUMMARY != DESCRIPTION`

**Notes**
Verifies the mapping: TW description → VTODO SUMMARY, TW annotation → VTODO DESCRIPTION.
The two fields must remain distinct.

---

### S-67 · Field Mapping — CalDAV PRIORITY Maps to TW Priority Field

| Field | Value |
|-------|-------|
| **ID** | S-67 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `CalDAV PRIORITY Maps To TW Priority Field` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice creates three CalDAV VTODOs with `PRIORITY:1`, `PRIORITY:5`, and `PRIORITY:9`. After sync
she finds TW tasks with priority `H`, `M`, and `L` respectively. She also creates a TW task with
priority `H` and confirms it syncs to CalDAV with `PRIORITY:1`.

**Setup (CalDAV → TW)**
- CalDAV: 3 VTODOs with PRIORITY 1, 5, and 9
- TW: empty

**Expected TW State (CalDAV → TW)**
3 TW tasks with priority H, M, L respectively

**Setup (TW → CalDAV)**
- TW: 1 pending task with priority H
- CalDAV: empty

**Expected CalDAV State (TW → CalDAV)**
1 VTODO with `PRIORITY:1`

**Exit Code**
0 (each sync)

**Notes**
RFC 5545 PRIORITY values: 1–4 = high, 5 = medium, 6–9 = low. caldawarrior maps 1→H, 5→M, 9→L
and the reverse for TW→CalDAV.

---

### S-68 · Field Mapping — CalDAV-only Task in Project Calendar Sets TW Project

| Field | Value |
|-------|-------|
| **ID** | S-68 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `CalDAV-Only Task In Project Calendar Sets TW Project` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice's CalDAV setup has a separate "work" calendar mapped to `project = "work"` in the
caldawarrior config. She creates a VTODO in the work calendar and runs sync. The resulting TW
task has `project == "work"`.

**Setup**
- CalDAV: 1 VTODO in the "work" calendar collection
- TW: empty
- Config: two `[[calendar]]` entries — `project = "default"` and `project = "work"`

**Expected TW State**
1 pending task with `project == "work"` and `caldavuid` set

**Exit Code**
0

**Notes**
This test is skipped unless the `MULTI_CALENDAR_ENABLED` environment variable is set.
Multi-calendar support requires multiple `[[calendar]]` entries in the caldawarrior config.

---

### S-69 · Field Mapping — TW Description Update Syncs to CalDAV SUMMARY

| Field | Value |
|-------|-------|
| **ID** | S-69 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `TW Description Update Syncs To CalDAV SUMMARY` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice updates a TW task description from "Old title" to "New title". After sync the CalDAV VTODO SUMMARY reflects the new description.

---

### S-70 · Field Mapping — CalDAV VTODO DUE Syncs to TW Due Date

| Field | Value |
|-------|-------|
| **ID** | S-70 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `CalDAV VTODO DUE Syncs To TW Due Date` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice creates a CalDAV VTODO with DUE:20260615T120000Z. After sync the TW task has due matching that date.

---

### S-71 · Field Mapping — TW Due Update Syncs to CalDAV DUE

| Field | Value |
|-------|-------|
| **ID** | S-71 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `TW Due Update Syncs To CalDAV DUE` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice changes the due date on a TW task from 2026-03-15 to 2026-06-20. After sync the CalDAV VTODO DUE property reflects the new date.

---

### S-72 · Field Mapping — CalDAV DUE Update Syncs to TW Due

| Field | Value |
|-------|-------|
| **ID** | S-72 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `CalDAV DUE Update Syncs To TW Due` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice changes the DUE on a CalDAV VTODO. After sync the TW task due date reflects the change.

---

### S-73 · Field Mapping — TW Due Clear Removes CalDAV DUE

| Field | Value |
|-------|-------|
| **ID** | S-73 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `TW Due Clear Removes CalDAV DUE Property` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice clears the due date on a TW task. After sync the CalDAV VTODO should no longer have a DUE property.

---

### S-74 · Field Mapping — CalDAV DUE Removal Syncs to TW Due Cleared

| Field | Value |
|-------|-------|
| **ID** | S-74 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `CalDAV DUE Removal Syncs To TW Due Cleared` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice removes the DUE property from a CalDAV VTODO. After sync the TW task should have no due date.

---

### S-75 · Field Mapping — TW Scheduled Date Syncs to CalDAV DTSTART

| Field | Value |
|-------|-------|
| **ID** | S-75 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `TW Scheduled Date Syncs To CalDAV DTSTART` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice creates a TW task with scheduled:2026-04-01. After sync the CalDAV VTODO has a DTSTART property matching that date.

---

### S-76 · Field Mapping — CalDAV DTSTART Syncs to TW Scheduled Date

| Field | Value |
|-------|-------|
| **ID** | S-76 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `CalDAV DTSTART Syncs To TW Scheduled Date` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice creates a CalDAV VTODO with DTSTART:20260501T090000Z. After sync the TW task has a scheduled date matching that datetime.

---

### S-77 · Field Mapping — TW Scheduled Clear Removes CalDAV DTSTART

| Field | Value |
|-------|-------|
| **ID** | S-77 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `TW Scheduled Clear Removes CalDAV DTSTART` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice clears the scheduled date on a TW task. After sync the CalDAV VTODO should no longer have a DTSTART property.

---

### S-78 · Field Mapping — TW Priority Syncs to CalDAV PRIORITY

| Field | Value |
|-------|-------|
| **ID** | S-78 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `TW Priority Syncs To CalDAV PRIORITY` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice creates a TW task with priority M. After sync the CalDAV VTODO has PRIORITY:5 (iCal medium).

---

### S-79 · Field Mapping — TW Priority Update Syncs to CalDAV PRIORITY

| Field | Value |
|-------|-------|
| **ID** | S-79 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `TW Priority Update Syncs To CalDAV PRIORITY` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice changes a TW task priority from M to H. After sync the CalDAV VTODO PRIORITY changes from 5 to 1.

---

### S-80 · Field Mapping — TW Priority Clear Removes CalDAV PRIORITY

| Field | Value |
|-------|-------|
| **ID** | S-80 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `TW Priority Clear Removes CalDAV PRIORITY` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice clears the priority on a TW task. After sync the CalDAV VTODO should no longer have a PRIORITY property.

---

### S-81 · Field Mapping — CalDAV CATEGORIES Syncs to TW Tags

| Field | Value |
|-------|-------|
| **ID** | S-81 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `CalDAV CATEGORIES Syncs To TW Tags` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice creates a CalDAV VTODO with CATEGORIES:home,errands. After sync the TW task has tags home and errands.

---

### S-82 · Field Mapping — TW Tags Update Syncs to CalDAV CATEGORIES

| Field | Value |
|-------|-------|
| **ID** | S-82 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `TW Tags Update Syncs To CalDAV CATEGORIES` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice adds a new tag to a TW task (already has +work, adds +urgent). After sync the CalDAV VTODO CATEGORIES includes both tags.

---

### S-83 · Field Mapping — TW Tags Cleared Removes CalDAV CATEGORIES

| Field | Value |
|-------|-------|
| **ID** | S-83 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `TW Tags Cleared Removes CalDAV CATEGORIES Property` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice removes all tags from a TW task. After sync the CalDAV VTODO should NOT have a CATEGORIES property (removed entirely, not empty).

---

### S-84 · Field Mapping — TW Annotation Update Syncs to CalDAV DESCRIPTION

| Field | Value |
|-------|-------|
| **ID** | S-84 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `TW Annotation Update Syncs To CalDAV DESCRIPTION` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice adds an annotation to a TW task. After sync the CalDAV VTODO DESCRIPTION reflects the annotation text.

---

### S-85 · Field Mapping — CalDAV DESCRIPTION Update Syncs to TW Annotation

| Field | Value |
|-------|-------|
| **ID** | S-85 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `CalDAV DESCRIPTION Update Syncs To TW Annotation` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice edits the DESCRIPTION of a CalDAV VTODO. After sync the TW task annotation text matches the updated DESCRIPTION.

---

### S-86 · Field Mapping — TW Task Without Annotations Has No CalDAV DESCRIPTION

| Field | Value |
|-------|-------|
| **ID** | S-86 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `TW Task Without Annotations Has No CalDAV DESCRIPTION` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice creates a TW task with no annotations. After sync the CalDAV VTODO should not have a DESCRIPTION property.

---

### S-87 · Field Mapping — TW Wait Date Syncs to CalDAV X-TASKWARRIOR-WAIT

| Field | Value |
|-------|-------|
| **ID** | S-87 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `TW Wait Date Syncs To CalDAV X-TASKWARRIOR-WAIT` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice creates a TW task with wait:2026-12-01. After sync the CalDAV VTODO has an X-TASKWARRIOR-WAIT property with the matching date.

---

### S-88 · Field Mapping — TW Wait Clear Removes CalDAV X-TASKWARRIOR-WAIT

| Field | Value |
|-------|-------|
| **ID** | S-88 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `TW Wait Clear Removes CalDAV X-TASKWARRIOR-WAIT` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice clears the wait date on a TW task. After sync the CalDAV VTODO should no longer have an X-TASKWARRIOR-WAIT property.

---

### S-89 · Field Mapping — Completing Task Sets COMPLETED Timestamp And Reopening Clears It

| Field | Value |
|-------|-------|
| **ID** | S-89 |
| **Category** | Field Mapping |
| **Robot Test Case Name** | `Completing Task Sets COMPLETED Timestamp And Reopening Clears It` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice completes a TW task and verifies the CalDAV VTODO gains a COMPLETED timestamp. She then reopens the task (status=pending) and verifies the COMPLETED property is removed.

---

## Idempotency (S-90 to S-95)

### S-90 · Idempotency -- Idempotent After TW Task Creation

| Field | Value |
|-------|-------|
| **ID** | S-90 |
| **Category** | Idempotency |
| **Robot Test Case Name** | `Idempotent After TW Task Creation` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice creates a TW task and syncs. Running sync a second time produces zero writes -- the task is already paired and nothing changed.

---

### S-91 · Idempotency -- Idempotent After CalDAV Task Creation

| Field | Value |
|-------|-------|
| **ID** | S-91 |
| **Category** | Idempotency |
| **Robot Test Case Name** | `Idempotent After CalDAV Task Creation` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice creates a CalDAV VTODO and syncs. Running sync again produces zero writes -- the task was pulled to TW and nothing changed.

---

### S-92 · Idempotency -- Idempotent After TW Field Update

| Field | Value |
|-------|-------|
| **ID** | S-92 |
| **Category** | Idempotency |
| **Robot Test Case Name** | `Idempotent After TW Field Update` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice updates due date, priority, and tags on a TW task and syncs. Running sync again produces zero writes -- all fields are in sync.

---

### S-93 · Idempotency -- Idempotent After CalDAV Field Update

| Field | Value |
|-------|-------|
| **ID** | S-93 |
| **Category** | Idempotency |
| **Robot Test Case Name** | `Idempotent After CalDAV Field Update` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice changes the SUMMARY on a CalDAV VTODO and syncs. Running sync again produces zero writes.

---

### S-94 · Idempotency -- Idempotent After TW Complete

| Field | Value |
|-------|-------|
| **ID** | S-94 |
| **Category** | Idempotency |
| **Robot Test Case Name** | `Idempotent After TW Complete` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice completes a TW task and syncs. Running sync again produces zero writes -- COMPLETED status and timestamp are in sync.

---

### S-95 · Idempotency -- Idempotent After TW Delete

| Field | Value |
|-------|-------|
| **ID** | S-95 |
| **Category** | Idempotency |
| **Robot Test Case Name** | `Idempotent After TW Delete` |
| **skip-unimplemented** | No |
| **Status** | ✅ Pass |

**User Story**

Alice deletes a TW task and syncs (CalDAV becomes CANCELLED). Running sync again produces zero writes.

---

## Summary Table

| ID | Category | Robot Test Case Name | skip-unimplemented | Status |
|----|----------|----------------------|--------------------|--------|
| S-01 | First Sync | `First Sync Creates TW Task In CalDAV` | No | ✅ Pass |
| S-02 | First Sync | `First Sync Sets Caldavuid UDA On TW Task` | No | ✅ Pass |
| S-03 | First Sync | `First Sync Dry Run Does Not Write VTODOs` | No | ✅ Pass |
| S-04 | First Sync | `First Sync Routes Projectless Task To Default Calendar` | No | ✅ Pass |
| S-05 | First Sync | `Five CalDAV VTODOs Created In TW On First Sync` | No | ✅ Pass |
| S-10 | LWW Conflict | `TW Wins LWW Conflict Resolution` | No | ✅ Pass |
| S-11 | LWW Conflict | `CalDAV Wins LWW Conflict Resolution` | No | ✅ Pass |
| S-12 | LWW Conflict | `Immediate Re-Sync After Conflict Is Stable Point` | No | ✅ Pass |
| S-13 | LWW Conflict | `ETag Conflict Is Handled Without Error` | No | ✅ Pass |
| S-14 | LWW Conflict | `LWW Conflict Dry Run Shows Update Without Writing` | No | ✅ Pass |
| S-20 | Orphan and Deletion | `Orphaned Caldavuid Causes TW Task Deletion` | No | ✅ Pass |
| S-21 | Orphan and Deletion | `Externally Deleted CalDAV VTODO Causes TW Deletion` | No | ✅ Pass |
| S-22 | Orphan and Deletion | `Re-Sync After Deletion Is Stable Point` | No | ✅ Pass |
| S-23 | Orphan and Deletion | `CalDAV CANCELLED VTODO Without TW Pair Does Not Create Ghost Task` | No | ✅ Pass |
| S-24 | Orphan and Deletion | `CalDAV COMPLETED VTODO Without TW Pair Does Not Create Task` | No | ✅ Pass |
| S-25 | Orphan and Deletion | `CalDAV Completed And TW Completed Both Terminal Zero Writes` | No | ✅ Pass |
| S-30 | Status Mapping | `CalDAV Completed Status Syncs To TW Completed` | No | ✅ Pass |
| S-31 | Status Mapping | `TW Completed Status Syncs To CalDAV Completed` | No | ✅ Pass |
| S-32 | Status Mapping | `Pending TW Task Stays Pending With Needs-Action VTODO` | No | ✅ Pass |
| S-33 | Status Mapping | `Completed Task Within Cutoff Is Synced Beyond Is Not` | No | ✅ Pass |
| S-34 | Status Mapping | `CalDAV Reopen Completed VTODO Syncs To TW Pending` | No | ✅ Pass |
| S-35 | Status Mapping | `TW Reopen Completed Task Syncs To CalDAV Needs-Action` | No | ✅ Pass |
| S-36 | Status Mapping | `TW Delete Syncs To CalDAV Cancelled` | No | ✅ Pass |
| S-37 | Status Mapping | `CalDAV Cancelled Syncs To TW Deleted` | No | ✅ Pass |
| S-38 | Status Mapping | `Both Sides Deleted And Cancelled Produces Zero Writes` | No | ✅ Pass |
| S-40 | Dependencies | `TW Depends Syncs To CalDAV Related-To` | No | ✅ Pass |
| S-41 | Dependencies | `CalDAV Related-To Syncs To TW Depends` | No | ✅ Pass |
| S-42 | Dependencies | `Cyclic Tasks Synced Without Related-To` | No | ✅ Pass |
| S-43 | Dependencies | `Three-Node Cyclic Dependency Synced Without Related-To` | No | ✅ Pass |
| S-44 | Dependencies | `TW Blocks Field Reflects Inverse Dependency After Sync` | No | ✅ Pass |
| S-45 | Dependencies | `Removing TW Dependency Clears CalDAV Related-To` | No | ✅ Pass |
| S-50 | CLI Behavior | `Invalid Credentials Produce Auth Error And Exit Code 1` | Yes | ⚠️ Skip |
| S-51 | CLI Behavior | `World Readable Config File Produces Permission Warning` | Yes | ⚠️ Skip |
| S-52 | CLI Behavior | `TW Recurring Task Is Skipped With Warn Message` | Yes | ⚠️ Skip |
| S-53 | CLI Behavior | `CalDAV Recurring VTODO Is Skipped With Warn Message` | Yes | ⚠️ Skip |
| S-54 | CLI Behavior | `Dry Run Flag Enables Dry Run Mode` | No | ✅ Pass |
| S-55 | CLI Behavior | `Missing Config File Produces Fatal Error And Exit Code 1` | No | ✅ Pass |
| S-60 | Field Mapping | `TW Description Syncs To VTODO Summary` | No | ✅ Pass |
| S-61 | Field Mapping | `TW Due Date Syncs To VTODO DUE Property` | No | ✅ Pass |
| S-62 | Field Mapping | `CalDAV VTODO Summary Syncs To TW Description` | No | ✅ Pass |
| S-63 | Field Mapping | `Caldavuid UDA Stores CalDAV UID As UUID4 String` | No | ✅ Pass |
| S-64 | Field Mapping | `CalDAV SUMMARY-only VTODO Creates TW Task With Matching Description` | No | ✅ Pass |
| S-65 | Field Mapping | `CalDAV DESCRIPTION-only VTODO Creates TW Task With Sentinel And Annotation` | No | ✅ Pass |
| S-66 | Field Mapping | `TW Task With Annotation Syncs To CalDAV With SUMMARY And DESCRIPTION` | No | ✅ Pass |
| S-67 | Field Mapping | `CalDAV PRIORITY Maps To TW Priority Field` | No | ✅ Pass |
| S-68 | Field Mapping | `CalDAV-Only Task In Project Calendar Sets TW Project` | No | ✅ Pass |
| S-69 | Field Mapping | `TW Description Update Syncs To CalDAV SUMMARY` | No | ✅ Pass |
| S-70 | Field Mapping | `CalDAV VTODO DUE Syncs To TW Due Date` | No | ✅ Pass |
| S-71 | Field Mapping | `TW Due Update Syncs To CalDAV DUE` | No | ✅ Pass |
| S-72 | Field Mapping | `CalDAV DUE Update Syncs To TW Due` | No | ✅ Pass |
| S-73 | Field Mapping | `TW Due Clear Removes CalDAV DUE Property` | No | ✅ Pass |
| S-74 | Field Mapping | `CalDAV DUE Removal Syncs To TW Due Cleared` | No | ✅ Pass |
| S-75 | Field Mapping | `TW Scheduled Date Syncs To CalDAV DTSTART` | No | ✅ Pass |
| S-76 | Field Mapping | `CalDAV DTSTART Syncs To TW Scheduled Date` | No | ✅ Pass |
| S-77 | Field Mapping | `TW Scheduled Clear Removes CalDAV DTSTART` | No | ✅ Pass |
| S-78 | Field Mapping | `TW Priority Syncs To CalDAV PRIORITY` | No | ✅ Pass |
| S-79 | Field Mapping | `TW Priority Update Syncs To CalDAV PRIORITY` | No | ✅ Pass |
| S-80 | Field Mapping | `TW Priority Clear Removes CalDAV PRIORITY` | No | ✅ Pass |
| S-81 | Field Mapping | `CalDAV CATEGORIES Syncs To TW Tags` | No | ✅ Pass |
| S-82 | Field Mapping | `TW Tags Update Syncs To CalDAV CATEGORIES` | No | ✅ Pass |
| S-83 | Field Mapping | `TW Tags Cleared Removes CalDAV CATEGORIES Property` | No | ✅ Pass |
| S-84 | Field Mapping | `TW Annotation Update Syncs To CalDAV DESCRIPTION` | No | ✅ Pass |
| S-85 | Field Mapping | `CalDAV DESCRIPTION Update Syncs To TW Annotation` | No | ✅ Pass |
| S-86 | Field Mapping | `TW Task Without Annotations Has No CalDAV DESCRIPTION` | No | ✅ Pass |
| S-87 | Field Mapping | `TW Wait Date Syncs To CalDAV X-TASKWARRIOR-WAIT` | No | ✅ Pass |
| S-88 | Field Mapping | `TW Wait Clear Removes CalDAV X-TASKWARRIOR-WAIT` | No | ✅ Pass |
| S-89 | Field Mapping | `Completing Task Sets COMPLETED Timestamp And Reopening Clears It` | No | ✅ Pass |
| S-90 | Idempotency | `Idempotent After TW Task Creation` | No | ✅ Pass |
| S-91 | Idempotency | `Idempotent After CalDAV Task Creation` | No | ✅ Pass |
| S-92 | Idempotency | `Idempotent After TW Field Update` | No | ✅ Pass |
| S-93 | Idempotency | `Idempotent After CalDAV Field Update` | No | ✅ Pass |
| S-94 | Idempotency | `Idempotent After TW Complete` | No | ✅ Pass |
| S-95 | Idempotency | `Idempotent After TW Delete` | No | ✅ Pass |

**Total scenarios: 75** (70 active, 1 skipped (S-68 multi-calendar), 4 skip-unimplemented)
