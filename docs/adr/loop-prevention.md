# ADR: Sync Loop Prevention — Two-Layer Design

**Date:** 2026-02-26
**Status:** Accepted
**Context:** Empirical research for caldawarrior sync engine (Phase 0, task-1-3)
**TW Version:** 3.4.2

---

## Problem

A bidirectional sync between TaskWarrior and a CalDAV server (Radicale) can create
infinite update loops:

1. caldawarrior reads TW change → writes VTODO to CalDAV
2. caldawarrior reads CalDAV change → writes back to TW via `task import`
3. `task import` updates `modified` (or TW writes a hook) → triggers step 1 again

Without loop prevention, every sync cycle re-triggers the next cycle indefinitely.

---

## Two-Layer Design

Loop prevention operates at two layers. Both layers are required; neither alone is
sufficient for all scenarios.

### Layer 1: ETag / `modified` Comparison (Change Detection)

Before writing to a side, check whether the value has actually changed:

- **CalDAV → TW direction:** Before calling `task import`, compare the VTODO's
  `LAST-MODIFIED` against the stored `modified` for that UUID. Only import if
  CalDAV's timestamp is newer.

- **TW → CalDAV direction:** Before PUTting a VTODO to CalDAV, fetch the current
  ETag or `LAST-MODIFIED`. Only write if TW's `modified` is newer.

**Why this alone is insufficient:** ETag/modified comparison prevents re-applying an
unchanged update, but does not prevent the case where our own write (step 1) causes
the other side to report a change that we then re-apply (step 2→1 loop). CalDAV servers
update `LAST-MODIFIED` on every PUT, so our own PUT looks like a remote change.

### Layer 2: Origin Tracking via `caldavuid` UDA

Each TW task that has been synced to CalDAV carries a `caldavuid` UDA value equal to
the CalDAV VTODO's `UID`. This serves a dual purpose:

1. **Lookup:** Map CalDAV UID → TW UUID for import/update operations.
2. **Origin fingerprint:** If a TW task was just written by caldawarrior (from a CalDAV
   change), its `modified` timestamp matches what was imported. The next TW→CalDAV pass
   will compare this `modified` against the CalDAV `LAST-MODIFIED` for that UID —
   they will match (we wrote both), so no write is needed.

**Combined invariant:**

```
TW.modified == CalDAV.LAST-MODIFIED  →  in sync, no action needed
TW.modified >  CalDAV.LAST-MODIFIED  →  TW is newer, push TW→CalDAV
TW.modified <  CalDAV.LAST-MODIFIED  →  CalDAV is newer, pull CalDAV→TW
```

Because `task import` **preserves** the `modified` field from the import JSON (confirmed
empirically — see `tw-field-clearing.md`), the sync engine can write exactly the CalDAV
`LAST-MODIFIED` into TW's `modified` during a CalDAV→TW sync. On the next cycle, the
comparison will show equality and no re-write will occur.

---

## Worked Examples

### Example A: Task created in TW, synced to CalDAV

```
t=0: User adds TW task. modified=T0. caldavuid=ABSENT.
t=1: Sync cycle. TW.modified=T0, CalDAV=none.
     → Push to CalDAV: PUT VTODO with UID=<new-uuid>
     → CalDAV LAST-MODIFIED=T1 (server time)
     → Store: TW modify caldavuid:<caldav-uid>
     → Import TW with modified=T1 (to align timestamps)
t=2: Next cycle. TW.modified=T1, CalDAV.LAST-MODIFIED=T1. Equal → no action.
```

### Example B: Task updated in CalDAV, synced to TW

```
t=0: CalDAV VTODO updated externally. LAST-MODIFIED=T2.
t=1: Sync cycle. CalDAV.LAST-MODIFIED=T2 > TW.modified=T1.
     → Pull from CalDAV: build import JSON with modified=T2
     → task import → TW.modified=T2 (preserved from JSON)
t=2: Next cycle. TW.modified=T2, CalDAV.LAST-MODIFIED=T2. Equal → no action.
```

### Example C: Conflict (both sides updated since last sync)

```
t=0: Last sync state: TW.modified=T0, CalDAV.LAST-MODIFIED=T0.
t=1: User edits TW task. TW.modified=T1.
     User (or another client) edits CalDAV VTODO. LAST-MODIFIED=T1'.
     (T1 and T1' are different timestamps — both > T0)
t=2: Sync cycle detects conflict: both sides modified since T0.
     → Last-Write-Wins: compare T1 vs T1'.
     → Winner's payload overwrites the other side.
     → Both sides set to winner's modified timestamp.
t=3: Next cycle. Equal timestamps → no action.
```

LWW is an acceptable policy for personal task sync. A future enhancement could offer
a "manual merge" mode for conflicts, but this is out of scope for Phase 3.

### Example D: Task deleted in TW, CalDAV VTODO still exists

```
t=0: TW task deleted. Status=deleted in TW.
t=1: Sync cycle. TW.status=deleted, CalDAV VTODO exists.
     → DELETE VTODO on CalDAV.
t=2: Next cycle. VTODO gone from CalDAV. TW task still deleted.
     → No CalDAV entry to process → no resurrection.
```

**Key insight (from empirical research):** If caldawarrior tries to re-import a TW-deleted
task (e.g., because a VTODO still exists on CalDAV), TW will **resurrect** the task as
pending. The correct sequence is: delete CalDAV first, then the CalDAV→TW direction has
nothing to import. Resurrection is only possible if the CalDAV deletion hasn't propagated.

**Guard:** Before pulling a CalDAV VTODO into TW, check if TW has a task with that UUID
in `status=deleted`. If so, delete the VTODO on CalDAV instead of importing.

---

## State Machine

The sync engine maintains a state table keyed by `(tw_uuid, caldav_uid)`.

**No separate database required.** The `last_synced_modified` value is not stored in an
external database — it is derived at runtime from TW's own `modified` field (which is
preserved by `task import`). After a successful sync, `TW.modified == CalDAV.LAST-MODIFIED`,
so the equality check on the next cycle works using TW's own data. This upholds the
project's no-database design invariant.

```
State: { tw_uuid, caldav_uid, last_synced_modified }

On each sync cycle:
  For each TW task with caldavuid set:
    tw_mod = TW export modified
    cal_mod = CalDAV REPORT LAST-MODIFIED
    tw_status = TW export status

    if tw_status == "deleted":
      → DELETE VTODO on CalDAV (if exists)
      → clear state entry

    elif tw_mod > last_synced_modified and cal_mod > last_synced_modified:
      → CONFLICT → LWW: pick winner by max(tw_mod, cal_mod)
      → apply winner to both sides
      → last_synced_modified = winner.modified

    elif tw_mod > last_synced_modified:
      → TW is newer → PUT VTODO to CalDAV
      → last_synced_modified = tw_mod

    elif cal_mod > last_synced_modified:
      → CalDAV is newer → task import (with modified=cal_mod)
      → last_synced_modified = cal_mod

    else:
      → in sync, no action

  For each CalDAV VTODO without a matching TW task (caldavuid not in TW):
    → Check if TW has this UUID as deleted
    → If deleted: DELETE VTODO on CalDAV
    → If not found: task import (new task from CalDAV)
```

---

## Why `caldavuid` UDA Is the Right Anchor

Alternative considered: Using TW's `uuid` directly as the CalDAV UID. This is simpler
but problematic:

- CalDAV UIDs are case-insensitive strings; TW UUIDs are lowercase hex. Collision risk
  is low but non-zero when importing from other clients.
- Storing the CalDAV UID separately (`caldavuid`) allows a TW task to have a different
  internal UUID than its CalDAV representation, supporting migration and multi-source sync.
- The `caldavuid` field can be cleared via `task modify caldavuid:` (confirmed empirically),
  enabling "unlink" operations without deleting either side.

---

## Implementation Requirements for Phase 3

1. **`caldavuid` UDA must be declared** in user's `.taskrc` (caldawarrior can auto-add it).
2. **`task import` must always include `modified`** from the CalDAV `LAST-MODIFIED` header.
3. **`task import` must include all known fields** — omitted fields are cleared (see
   `tw-field-clearing.md`).
4. **Sync engine must check `status=deleted` before import** to prevent resurrection.
5. **`task delete` non-idempotency** must be handled: check status before deleting, or
   treat exit-1 "not deletable" as a non-error.
6. **`status.not:purged`** is valid and can be used in export filters.
7. **Expired `wait` tasks** export as `status=pending` — no special handling needed.
