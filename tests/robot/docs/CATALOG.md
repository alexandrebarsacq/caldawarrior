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
| S-20–S-22 | Orphan and Deletion   |
| S-30–S-33 | Status Mapping        |
| S-40–S-42 | Dependencies          |
| S-50–S-55 | CLI Behavior          |
| S-60–S-63 | Field Mapping         |
| S-70–S-79 | Bulk Operations       |
| S-80–S-89 | Multi-Sync Journeys   |

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

## Orphan and Deletion (S-20 to S-22)

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

## Status Mapping (S-30 to S-33)

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

## Dependencies (S-40 to S-42)

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

### S-42 · Dependencies — Cyclic Dependency Warning Emitted and Tasks Skipped

| Field | Value |
|-------|-------|
| **ID** | S-42 |
| **Category** | Dependencies |
| **Robot Test Case Name** | `Cyclic Dependency Emits Warning And Skips Tasks` |
| **skip-unimplemented** | Yes |
| **Status** | ⚠️ Skip |

**User Story**

Alice accidentally creates a dependency cycle: task A depends on task B, and task B depends on
task A. She runs `caldawarrior sync`. She expects caldawarrior to detect the cycle, emit a
`[WARN]` message for each task involved, skip those tasks without erroring, and exit with code 0.

**Setup**
- TW: 2 pending tasks; task A `depends` on task B; task B `depends` on task A (cycle)
- CalDAV: empty calendar

**Expected Stdout**
```
Synced: 0 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
```
[WARN] CyclicEntry: task '*' is part of a dependency cycle
[WARN] CyclicEntry: task '*' is part of a dependency cycle
```

**Exit Code**
0

**Expected CalDAV State**
No VTODOs created (cyclic tasks are skipped via `PlannedOp::Skip { reason: SkipReason::Cyclic }`)

**Expected TW State**
Neither TW task has `caldavuid` set

**Notes**
**skip-unimplemented: Yes** — Behavior is implemented in `src/sync/deps.rs:138-145` but is not
covered by any CLI-level test. Robot test tagged `skip-unimplemented` until a CLI blackbox test is
added. Warning format: `[WARN] CyclicEntry: task '{description}' is part of a dependency cycle`
(from GAP_ANALYSIS.md §1.4, sourced from `src/sync/deps.rs:140-144`). `*` in Expected Stderr
denotes the task description matched as a glob.

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

## Field Mapping (S-60 to S-63)

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
| **Status** | ⏳ Pending |

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
| **Status** | ⏳ Pending |

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
| **Status** | ⏳ Pending |

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
| **Status** | ⏳ Pending |

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
| **Status** | ⏳ Pending |

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

## Bulk Operations (S-70 to S-79)

### S-70 · Bulk Operations — 5 TW Tasks Synced to CalDAV on First Sync

| Field | Value |
|-------|-------|
| **ID** | S-70 |
| **Category** | Bulk Operations |
| **Robot Test Case Name** | `Five TW Tasks Created In CalDAV On First Sync` |
| **skip-unimplemented** | No |
| **Status** | ⚠️ No test |

**User Story**

Alice has 5 pending TW tasks covering different projects. She runs sync for the first time. She
expects all 5 tasks to appear in her CalDAV calendar and each to receive a caldavuid UDA.

**Setup**
- TW: 5 pending tasks (mix of descriptions), no `caldavuid` UDA set on any
- CalDAV: empty calendar

**Expected Stdout**
```
Synced: 5 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
5 VTODOs exist, each with SUMMARY matching one of the TW task descriptions

**Expected TW State**
All 5 TW tasks have a non-empty `caldavuid` UDA

**Notes**
Tests bulk push path. Output format from GAP_ANALYSIS.md §1.1.

---

### S-71 · Bulk Operations — 5 CalDAV VTODOs Synced to TW on First Sync

| Field | Value |
|-------|-------|
| **ID** | S-71 |
| **Category** | Bulk Operations |
| **Robot Test Case Name** | `Five CalDAV VTODOs Created In TW On First Sync` |
| **skip-unimplemented** | No |
| **Status** | ⚠️ No test |

**User Story**

Bob has 5 VTODOs already in his CalDAV calendar (added via another app). He runs caldawarrior
sync. He expects all 5 to appear as TW tasks with `caldavuid` set.

**Setup**
- TW: empty (no tasks)
- CalDAV: 5 VTODOs with SUMMARY and STATUS:NEEDS-ACTION

**Expected Stdout**
```
Synced: 0 created, 0 updated in CalDAV; 5 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
Unchanged (5 VTODOs)

**Expected TW State**
5 pending TW tasks created, each with `caldavuid` matching the corresponding VTODO UID

**Notes**
Tests bulk pull path. Output format from GAP_ANALYSIS.md §1.1.

---

### S-72 · Bulk Operations — 4 TW Tasks and 3 CalDAV VTODOs on First Sync (Mixed)

| Field | Value |
|-------|-------|
| **ID** | S-72 |
| **Category** | Bulk Operations |
| **Robot Test Case Name** | `Mixed First Sync Links Tasks From Both Sides` |
| **skip-unimplemented** | No |
| **Status** | ⚠️ No test |

**User Story**

Carol has 4 TW tasks and 3 CalDAV VTODOs from different sources. None are linked yet. She syncs
for the first time. She expects 4 TW tasks to be pushed to CalDAV and 3 VTODOs to be pulled to
TW, with all 7 items linked.

**Setup**
- TW: 4 pending tasks, no `caldavuid` set on any
- CalDAV: 3 VTODOs

**Expected Stdout**
```
Synced: 4 created, 0 updated in CalDAV; 3 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
7 VTODOs total (4 new from TW + original 3)

**Expected TW State**
7 tasks total (4 original with `caldavuid` set + 3 new from CalDAV)

**Notes**
Tests simultaneous push and pull. Output format from GAP_ANALYSIS.md §1.1.

---

### S-73 · Bulk Operations — Immediate Re-Sync of 5 Tasks Is a Stable Point

| Field | Value |
|-------|-------|
| **ID** | S-73 |
| **Category** | Bulk Operations |
| **Robot Test Case Name** | `Bulk Stable Point Re-Sync Produces Zero Writes` |
| **skip-unimplemented** | No |
| **Status** | ⚠️ No test |

**User Story**

After a successful bulk first sync of 5 tasks, Alice runs sync again immediately without any
changes. She expects zero writes and the summary to show all zeroes.

**Setup**
- TW and CalDAV already in sync (5 paired tasks)
- No changes made since last sync

**Expected Stdout**
```
Synced: 0 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
Unchanged

**Expected TW State**
Unchanged

**Notes**
Loop prevention check at scale. Output format from GAP_ANALYSIS.md §1.2.

---

### S-74 · Bulk Operations — 5-Task Dry Run Produces 5 CREATE Lines and Correct Summary

| Field | Value |
|-------|-------|
| **ID** | S-74 |
| **Category** | Bulk Operations |
| **Robot Test Case Name** | `Bulk Dry Run Shows Five Create Lines And Correct Summary` |
| **skip-unimplemented** | No |
| **Status** | ⚠️ No test |

**User Story**

Dave wants to preview what sync would do before committing. He has 5 TW tasks not yet in CalDAV.
He runs sync with `--dry-run`. He expects 5 `[DRY-RUN] [CREATE]` lines and a summary of 5
creates.

**Setup**
- TW: 5 pending tasks, no `caldavuid` set
- CalDAV: empty calendar
- CLI invoked with `--dry-run` flag

**Expected Stdout**
```
[DRY-RUN] [CREATE] CalDAV <- TW: {description}   (one line per task, 5 total)
[DRY-RUN] Would: 5 create(s), 0 update(s), 0 delete(s), 0 skip(s)
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
Unchanged (empty — dry-run writes nothing)

**Expected TW State**
Unchanged (no `caldavuid` set — dry-run writes nothing)

**Notes**
Dry-run format from GAP_ANALYSIS.md §1.3. The 5 `[DRY-RUN] [CREATE]` lines are verified by count,
not order, since task ordering may vary.

---

### S-75 · Bulk Operations — 4 TW Tasks Deleted from CalDAV Externally; All 4 Orphans Removed from TW

| Field | Value |
|-------|-------|
| **ID** | S-75 |
| **Category** | Bulk Operations |
| **Robot Test Case Name** | `Four Orphaned TW Tasks Deleted After CalDAV Purge` |
| **skip-unimplemented** | No |
| **Status** | ⚠️ No test |

**User Story**

Eve deleted 4 tasks directly from her CalDAV calendar. The TW tasks still have their `caldavuid`
set. She runs sync. She expects all 4 orphaned TW tasks to be deleted, and the summary to reflect
no creates or updates.

**Setup**
- TW: 4 tasks with valid `caldavuid` UDAs
- CalDAV: empty (VTODOs deleted externally)

**Expected Stdout**
```
Synced: 0 created, 0 updated in CalDAV; 0 created, 0 updated in TW
```

**Expected Stderr**
(empty)

**Exit Code**
0

**Expected CalDAV State**
Empty

**Expected TW State**
All 4 tasks deleted (or completed, per deletion semantics)

**Notes**
Bulk orphan handling. Deletes are not reflected in the summary counters (GAP_ANALYSIS.md §1.1 —
only creates and updates are shown). Output format from GAP_ANALYSIS.md §1.1.

---

## Multi-Sync Journeys (S-80 to S-89)

These are multi-step test scenarios. Each represents a realistic sequence of user actions across
several sync runs. In Robot Framework these are implemented as a single test case with multiple
sync steps and state assertions between steps.

### S-80 · Multi-Sync Journey — Growing TW Task List Syncs Incrementally

| Field | Value |
|-------|-------|
| **Category** | Multi-Sync Journey |
| **Robot Test Case Name** | `Growing TW Task List Syncs Incrementally` |
| **skip-unimplemented** | No |
| **Status** | ⚠️ No test |

**User Story**

Frank starts with 2 TW tasks, syncs, then adds 2 more TW tasks, syncs again, then adds 1 more,
syncs. He expects each sync to only push the new tasks and leave already-synced tasks alone.

**Steps**
1. Sync → stdout: `Synced: 2 created, 0 updated in CalDAV; 0 created, 0 updated in TW`. CalDAV: 2 VTODOs. TW: 2 tasks with `caldavuid`.
2. Add 2 more TW tasks (no `caldavuid`). Sync → stdout: `Synced: 2 created, 0 updated in CalDAV; 0 created, 0 updated in TW`. CalDAV: 4 VTODOs. TW: 4 tasks with `caldavuid`.
3. Add 1 more TW task. Sync → stdout: `Synced: 1 created, 0 updated in CalDAV; 0 created, 0 updated in TW`. CalDAV: 5 VTODOs. TW: 5 tasks with `caldavuid`.
4. Sync immediately again → `Synced: 0 created, 0 updated in CalDAV; 0 created, 0 updated in TW` (stable point).

**Setup**
- TW: 2 pending tasks, no `caldavuid` set
- CalDAV: empty calendar

**Expected Final State** (after all steps)
5 tasks in both TW and CalDAV, all paired, no duplicates.

**Expected Stdout (each sync)**
See Steps above.

**Expected Stderr**
(empty)

**Exit Code**
0 (each sync)

**Notes**
Validates that previously-synced tasks are not re-created. Tests the `caldavuid` lookup path.

---

### S-81 · Multi-Sync Journey — Growing CalDAV Calendar Syncs Incrementally to TW

| Field | Value |
|-------|-------|
| **Category** | Multi-Sync Journey |
| **Robot Test Case Name** | `Growing CalDAV Calendar Syncs Incrementally To TW` |
| **skip-unimplemented** | No |
| **Status** | ⚠️ No test |

**User Story**

Grace uses a CalDAV client (e.g., a mobile app) to create tasks in her calendar. She creates 2
VTODOs, syncs to TW, then creates 2 more VTODOs, syncs again. She expects each sync to create
only the new TW tasks.

**Steps**
1. Sync → `Synced: 0 created, 0 updated in CalDAV; 2 created, 0 updated in TW`. TW: 2 tasks with `caldavuid`.
2. Add 2 more VTODOs to CalDAV. Sync → `Synced: 0 created, 0 updated in CalDAV; 2 created, 0 updated in TW`. TW: 4 tasks.
3. Sync immediately → `Synced: 0 created, 0 updated in CalDAV; 0 created, 0 updated in TW` (stable).

**Setup**
- TW: empty
- CalDAV: 2 VTODOs

**Expected Final State** (after all steps)
4 tasks in TW, 4 VTODOs in CalDAV, all paired.

**Expected Stdout (each sync)**
See Steps above.

**Expected Stderr**
(empty)

**Exit Code**
0 (each sync)

**Notes**
Validates that already-pulled VTODOs are not re-imported on subsequent syncs.

---

### S-82 · Multi-Sync Journey — Bidirectional Growth Converges Correctly

| Field | Value |
|-------|-------|
| **Category** | Multi-Sync Journey |
| **Robot Test Case Name** | `Bidirectional Growth Converges Correctly Over Multiple Syncs` |
| **skip-unimplemented** | No |
| **Status** | ⚠️ No test |

**User Story**

Henry adds tasks from TW on his laptop and from a CalDAV client on his phone, then syncs. Over
three rounds, tasks accumulate from both sides. He expects all tasks to converge correctly.

**Steps**
1. Sync → `Synced: 1 created, 0 updated in CalDAV; 1 created, 0 updated in TW`. Both sides: 2 tasks total.
2. Add 2 TW tasks + 2 CalDAV VTODOs (new, not yet synced). Sync → `Synced: 2 created, 0 updated in CalDAV; 2 created, 0 updated in TW`. Both sides: 6 tasks total.
3. Add 1 TW task + 1 CalDAV VTODO. Sync → `Synced: 1 created, 0 updated in CalDAV; 1 created, 0 updated in TW`. Both sides: 8 tasks total.
4. Sync → stable point: `Synced: 0 created, 0 updated in CalDAV; 0 created, 0 updated in TW`.

**Setup**
- TW: 1 pending task, no `caldavuid`
- CalDAV: 1 VTODO (new, never synced)

**Expected Final State** (after all steps)
8 tasks on each side, all paired, no duplicates, no data loss.

**Expected Stdout (each sync)**
See Steps above.

**Expected Stderr**
(empty)

**Exit Code**
0 (each sync)

**Notes**
Tests simultaneous incremental push and pull over multiple sync cycles.

---

### S-83 · Multi-Sync Journey — Repeated TW Edits Propagate to CalDAV Each Sync

| Field | Value |
|-------|-------|
| **Category** | Multi-Sync Journey |
| **Robot Test Case Name** | `Repeated TW Edits Propagate To CalDAV Each Sync` |
| **skip-unimplemented** | No |
| **Status** | ⚠️ No test |

**User Story**

Iris creates a task in TW, syncs it to CalDAV, then edits the description in TW, syncs again,
then edits again, syncs again. She expects each edit to be reflected in CalDAV after the next
sync.

**Steps**
1. Sync → "Buy apples" appears in CalDAV with SUMMARY "Buy apples". Stable.
2. Modify TW task description to "Buy apples and oranges" (newer `modified` timestamp). Sync → `Synced: 0 created, 1 updated in CalDAV; 0 created, 0 updated in TW`. CalDAV VTODO SUMMARY = "Buy apples and oranges".
3. Modify TW task description to "Buy fruit". Sync → `Synced: 0 created, 1 updated in CalDAV; 0 created, 0 updated in TW`. CalDAV SUMMARY = "Buy fruit".
4. Sync → stable point: `Synced: 0 created, 0 updated in CalDAV; 0 created, 0 updated in TW`.

**Setup**
- TW: 1 pending task ("Buy apples"), no `caldavuid`
- CalDAV: empty calendar

**Expected Final State** (after all steps)
TW task description = "Buy fruit", CalDAV VTODO SUMMARY = "Buy fruit".

**Expected Stdout (each sync)**
See Steps above.

**Expected Stderr**
(empty)

**Exit Code**
0 (each sync)

**Notes**
Each edit must produce a newer `modified` timestamp than the CalDAV VTODO's LAST-MODIFIED so that
TW wins LWW on each cycle. Tests the update counter incrementing on each sync.

---

### S-84 · Multi-Sync Journey — Repeated CalDAV Edits Propagate to TW Each Sync

| Field | Value |
|-------|-------|
| **Category** | Multi-Sync Journey |
| **Robot Test Case Name** | `Repeated CalDAV Edits Propagate To TW Each Sync` |
| **skip-unimplemented** | No |
| **Status** | ⚠️ No test |

**User Story**

Jack edits a task from his phone (via CalDAV). He edits the SUMMARY twice, syncing between each
edit. He expects TW to reflect the latest CalDAV state each time.

**Steps**
1. Edit CalDAV VTODO SUMMARY to "Meeting prep v2" (with newer LAST-MODIFIED). Sync → `Synced: 0 created, 0 updated in CalDAV; 0 created, 1 updated in TW`. TW task description = "Meeting prep v2".
2. Edit CalDAV VTODO SUMMARY to "Meeting prep final". Sync → `Synced: 0 created, 0 updated in CalDAV; 0 created, 1 updated in TW`. TW task description = "Meeting prep final".
3. Sync → stable point: `Synced: 0 created, 0 updated in CalDAV; 0 created, 0 updated in TW`.

**Setup**
- 1 task already paired in both TW and CalDAV (from a prior sync). TW task description: "Meeting prep".

**Expected Final State** (after all steps)
TW task description = "Meeting prep final", CalDAV VTODO SUMMARY = "Meeting prep final".

**Expected Stdout (each sync)**
See Steps above.

**Expected Stderr**
(empty)

**Exit Code**
0 (each sync)

**Notes**
Each VTODO edit must carry a LAST-MODIFIED newer than the TW task's `modified` timestamp so that
CalDAV wins LWW on each cycle.

---

### S-85 · Multi-Sync Journey — Full Task Lifecycle: Create, Edit from Both Sides, Complete

| Field | Value |
|-------|-------|
| **Category** | Multi-Sync Journey |
| **Robot Test Case Name** | `Full Task Lifecycle Create Edit From Both Sides Then Complete` |
| **skip-unimplemented** | No |
| **Status** | ⚠️ No test |

**User Story**

Kate creates a task in TW, syncs it, edits the description from CalDAV (phone), syncs back, then
edits from TW, syncs, then marks it done in TW and syncs to complete in CalDAV. She expects the
full lifecycle to work end-to-end.

**Steps**
1. Sync → "Write report" appears in CalDAV. `Synced: 1 created, 0 updated in CalDAV; 0 created, 0 updated in TW`. Stable.
2. Edit CalDAV VTODO SUMMARY to "Write quarterly report". Sync → `Synced: 0 created, 0 updated in CalDAV; 0 created, 1 updated in TW`. TW description = "Write quarterly report". Stable.
3. Edit TW task description to "Write quarterly report — DONE draft". Sync → `Synced: 0 created, 1 updated in CalDAV; 0 created, 0 updated in TW`. CalDAV SUMMARY updated. Stable.
4. Mark TW task as `done`. Sync → `Synced: 0 created, 1 updated in CalDAV; 0 created, 0 updated in TW`. CalDAV VTODO STATUS = COMPLETED.
5. Sync → stable point: `Synced: 0 created, 0 updated in CalDAV; 0 created, 0 updated in TW`. CalDAV STATUS:COMPLETED. TW status: completed.

**Setup**
- TW: 1 pending task ("Write report"), no `caldavuid`
- CalDAV: empty calendar

**Expected Final State** (after all steps)
TW task completed, CalDAV VTODO STATUS:COMPLETED, descriptions in sync.

**Expected Stdout (each sync)**
See Steps above.

**Expected Stderr**
(empty)

**Exit Code**
0 (each sync)

**Notes**
End-to-end lifecycle test. Covers first sync push, CalDAV-wins LWW, TW-wins LWW, and status
mapping (TW `done` → CalDAV `STATUS:COMPLETED`).

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
| S-30 | Status Mapping | `CalDAV Completed Status Syncs To TW Completed` | No | ✅ Pass |
| S-31 | Status Mapping | `TW Completed Status Syncs To CalDAV Completed` | No | ✅ Pass |
| S-32 | Status Mapping | `Pending TW Task Stays Pending With Needs-Action VTODO` | No | ✅ Pass |
| S-33 | Status Mapping | `Completed Task Within Cutoff Is Synced Beyond Is Not` | No | ✅ Pass |
| S-40 | Dependencies | `TW Depends Syncs To CalDAV Related-To` | No | ✅ Pass |
| S-41 | Dependencies | `CalDAV Related-To Syncs To TW Depends` | No | ✅ Pass |
| S-42 | Dependencies | `Cyclic Dependency Emits Warning And Skips Tasks` | Yes | ⚠️ Skip |
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
| S-64 | Field Mapping | `CalDAV SUMMARY-only VTODO Creates TW Task With Matching Description` | No | ⏳ Pending |
| S-65 | Field Mapping | `CalDAV DESCRIPTION-only VTODO Creates TW Task With Sentinel And Annotation` | No | ⏳ Pending |
| S-66 | Field Mapping | `TW Task With Annotation Syncs To CalDAV With SUMMARY And DESCRIPTION` | No | ⏳ Pending |
| S-67 | Field Mapping | `CalDAV PRIORITY Maps To TW Priority Field` | No | ⏳ Pending |
| S-68 | Field Mapping | `CalDAV-Only Task In Project Calendar Sets TW Project` | No | ⏳ Pending |
| S-70 | Bulk Operations | `Five TW Tasks Created In CalDAV On First Sync` | No | ⚠️ No test |
| S-71 | Bulk Operations | `Five CalDAV VTODOs Created In TW On First Sync` | No | ⚠️ No test |
| S-72 | Bulk Operations | `Mixed First Sync Links Tasks From Both Sides` | No | ⚠️ No test |
| S-73 | Bulk Operations | `Bulk Stable Point Re-Sync Produces Zero Writes` | No | ⚠️ No test |
| S-74 | Bulk Operations | `Bulk Dry Run Shows Five Create Lines And Correct Summary` | No | ⚠️ No test |
| S-75 | Bulk Operations | `Four Orphaned TW Tasks Deleted After CalDAV Purge` | No | ⚠️ No test |
| S-80 | Multi-Sync Journey | `Growing TW Task List Syncs Incrementally` | No | ⚠️ No test |
| S-81 | Multi-Sync Journey | `Growing CalDAV Calendar Syncs Incrementally To TW` | No | ⚠️ No test |
| S-82 | Multi-Sync Journey | `Bidirectional Growth Converges Correctly Over Multiple Syncs` | No | ⚠️ No test |
| S-83 | Multi-Sync Journey | `Repeated TW Edits Propagate To CalDAV Each Sync` | No | ⚠️ No test |
| S-84 | Multi-Sync Journey | `Repeated CalDAV Edits Propagate To TW Each Sync` | No | ⚠️ No test |
| S-85 | Multi-Sync Journey | `Full Task Lifecycle Create Edit From Both Sides Then Complete` | No | ⚠️ No test |

**Total scenarios: 47** (42 active, 5 skip-unimplemented)
