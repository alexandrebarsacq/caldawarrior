# Dependency Mapping: Taskwarrior ↔ CalDAV VTODO

## Overview

Taskwarrior's `depends` field expresses blocking relationships between tasks: a
task with dependencies cannot be started until all its dependencies are
completed. CalDAV (RFC 9253) expresses the same relationship via the
`RELATED-TO` property with `RELTYPE=DEPENDS-ON`.

Dependency resolution is handled via a three-step sync process that loads all
tasks into an intermediate representation (IR) before resolving any links. This
eliminates ordering constraints entirely — by the time dependencies are resolved,
every task is already in memory.

---

## Field mapping

| TW field | CalDAV property | Direction | Notes |
|----------|-----------------|-----------|-------|
| `depends` (list of TW UUIDs) | `RELATED-TO;RELTYPE=DEPENDS-ON:<caldav-uid>` | TW ↔ CalDAV | One `RELATED-TO` line per dependency |

The value used in `RELATED-TO` is the CalDAV UID (the `caldavuid` UDA), not the
TW UUID. TW UUIDs are internal; CalDAV clients only know about CalDAV UIDs.

---

## Intermediate representation (IR)

Each IR entry holds the full state of one task from either or both systems:

```
IREntry {
    tw_uuid:          Option<UUID>      # None if CalDAV-only
    caldav_uid:       String            # always present after step 1
    tw_data:          Option<TWTask>    # None if CalDAV-only
    caldav_data:      Option<VTODO>     # None if TW-only
    resolved_depends: Vec<IREntry>      # populated in step 2
    dirty_tw:         bool              # needs write to TW in step 3
    dirty_caldav:     bool              # needs write to CalDAV in step 3
}
```

Tasks that exist in both systems have both `tw_data` and `caldav_data`
populated. Tasks that exist in only one system have one `None`. After step 1,
every entry has a `caldav_uid` — new TW tasks are assigned one if they do not
already have the `caldavuid` UDA set.

---

## Sync algorithm

### Step 1 — Load IR

Read all tasks from both systems into the IR:

- Read all TW tasks → IR entries with TW UUID
- Read all CalDAV VTODOs → IR entries with CalDAV UID
- Match paired entries (tasks that exist in both systems) by `caldavuid`
- Assign a `caldavuid` to any TW task that does not yet have one

At the end of step 1, every entry in the IR has a `caldav_uid`. Paired entries
have both `tw_data` and `caldav_data`; unpaired entries have one `None`.

### Step 2 — Resolve dependencies

For each entry in the IR, resolve its dependency references:

- **From TW side:** for each UUID in `depends`, look up the entry in the IR by
  TW UUID → record the resolved link.
- **From CalDAV side:** for each `RELATED-TO;RELTYPE=DEPENDS-ON:<uid>`, look
  up the entry in the IR by CalDAV UID → record the resolved link.

Because all tasks are already in the IR, resolution is a simple map lookup.
No ordering is required.

**Unresolvable references** (deleted tasks, tasks on a different calendar,
corrupted data) are dropped with a per-task warning. The task itself is still
synced; only that dependency link is omitted.

**Cycle detection** is run on the resolved dependency graph using a depth-first
search. Tasks involved in a cycle are excluded from step 3 and a warning is
emitted. See [Cycle detection](#cycle-detection) below.

### Step 3 — Write back

For each IR entry, determine what action to take based on the presence of
`tw_data` and `caldav_data` and the status of each:

```
TW only (no caldav_data)
  ├─ status=deleted, no caldavuid UDA      → SKIP
  ├─ status=deleted, has caldavuid UDA     → DELETE in TW (CalDAV deletion wins)
  ├─ status=completed, no caldavuid UDA    → CREATE in CalDAV as COMPLETED
  └─ status=pending/waiting, no caldavuid  → CREATE in CalDAV as NEEDS-ACTION

CalDAV only (no tw_data)
  ├─ STATUS=CANCELLED or COMPLETED         → SKIP
  └─ STATUS=NEEDS-ACTION                   → CREATE in TW as pending

Both exist
  ├─ TW deleted,    CalDAV active          → UPDATE CalDAV → CANCELLED
  ├─ TW completed,  CalDAV NEEDS-ACTION    → UPDATE CalDAV → COMPLETED
  ├─ CalDAV CANCELLED, TW active           → DELETE in TW
  ├─ CalDAV COMPLETED, TW pending          → UPDATE TW → completed
  ├─ both deleted / cancelled / completed  → SKIP
  └─ both active
      ├─ content identical                 → SKIP (no spurious writes)
      └─ content differs                   → Last Write Wins
                                             compare TW modified vs CalDAV LAST-MODIFIED
                                             newer side's full state overwrites the other
```

**"TW only, has caldavuid, CalDAV gone"** means the VTODO was deleted on the
CalDAV side since the last sync. The correct action is to delete the TW task
too. Skipping would cause the task to re-create the VTODO on the next sync,
undoing the remote deletion.

**"TW deleted, CalDAV active"** maps to CANCELLED rather than a hard delete.
CANCELLED is the safe, recoverable signal — the CalDAV client may have
attached notes or shared the task. Hard-deleting a remote VTODO is
destructive and irreversible.

**Last Write Wins** compares the `modified` field (TW) against `LAST-MODIFIED`
(CalDAV). The newer side's entire task state is written to the older side. See
[Known limitations](#known-limitations) for the field-level conflict blind spot
this creates.

---

## Non-`DEPENDS-ON` relationship types

RFC 9253 defines additional `RELTYPE` values: `PARENT`, `CHILD`, `SIBLING`,
`REFID`, `CONCEPT`, and others. TW has no equivalent for any of these.

**TW → CalDAV:** only `RELTYPE=DEPENDS-ON` is ever generated. No other
relationship types are written.

**CalDAV → TW:** any `RELATED-TO` with a `RELTYPE` other than `DEPENDS-ON` is
silently skipped per item. A single summary warning is emitted at the end of
the sync run:

```
Skipped 12 RELATED-TO relationships with unsupported types: PARENT (8), CHILD (4)
```

One summary line per run regardless of how many items are affected. This avoids
flooding output when a hierarchy-aware CalDAV client has annotated every task.

---

## Cycle detection

TW's data model forbids dependency cycles. A CalDAV client may create one (no
enforcement at the protocol level).

Cycle detection runs in step 2 as a depth-first search over the resolved
dependency graph. Any task that is part of a cycle is flagged.

**Behaviour on detection:**

- Flagged tasks are **excluded from step 3** (not written to either system).
- A warning is emitted identifying all tasks in the cycle by name and UUID:

```
Skipped 3 tasks involved in a dependency cycle:
  "Buy milk"     (uuid-a)
  "Cook dinner"  (uuid-b)
  "Go shopping"  (uuid-c)
```

- Tasks not in the cycle are unaffected and sync normally.

**TW → CalDAV:** cycles in TW data indicate data corruption (TW normally
prevents them). Detection here is a safety net.

**CalDAV → TW:** cycles in CalDAV data are the realistic failure case — a
CalDAV client may not enforce acyclicity.

---

## Known limitations

1. **Dependency links to tasks on other calendars are dropped.** If task A
   depends on task B and B is on a calendar not included in the sync, B will
   not be in the IR and the link is unresolvable. It is dropped with a warning.

2. **Non-`DEPENDS-ON` relationships are not preserved.** `PARENT`, `CHILD`,
   `SIBLING`, and other RFC 9253 relationship types have no TW equivalent and
   are discarded on CalDAV → TW. They are never generated on TW → CalDAV.

3. **Last Write Wins is task-level, not field-level.** If TW description
   changed on one side and the due date changed on the other since the last
   sync, LWW picks one task's entire state as the winner and silently discards
   the other side's changes. Field-level merging is not implemented.

4. **First sync creates duplicates if the same task exists in both systems.**
   On the first sync, no TW task has a `caldavuid`. A task entered manually
   into both TW and CalDAV before the first sync will be created twice — once
   in each direction — resulting in duplicates. There is no deduplication
   heuristic. The recommended approach is to treat one system as authoritative
   before the first sync and not pre-populate the other.
