# ADR: TaskWarrior Field Clearing and Status Transitions

**Date:** 2026-02-26
**Status:** Accepted
**Context:** Empirical research for caldawarrior sync engine (Phase 0, task-1-3)
**TW Version:** 3.4.2

---

## Summary

Empirical research using isolated TASKDATA environments confirmed how TaskWarrior handles
field clearing, `task import`, and status transitions. All findings below are from
`tests/integration/tw-behavior-research.sh` (13/13 tests passed).

---

## Field Clearing via `task modify field:`

### Confirmed: Trailing colon removes field from exported JSON

Running `task <uuid> modify field:` (with a trailing colon and no value) **removes** the field
entirely from the `task export` JSON output. This was confirmed for:

| Field | Command | Result |
|-------|---------|--------|
| `caldavuid` (UDA) | `task <uuid> modify caldavuid:` | Field absent in export |
| `due` | `task <uuid> modify due:` | Field absent in export |
| `scheduled` | `task <uuid> modify scheduled:` | Field absent in export |

**Implication for sync:** To clear a CalDAV-sourced field on a task (e.g., when the
VTODO no longer has a due date), use `task <uuid> modify due:`. Do not set to an empty
string — the trailing colon syntax is required.

### Confirmed: `task import` omitting a UDA field also clears it

When a task is re-imported via `task import` and the exported JSON omits a previously-set
UDA field (e.g., `caldavuid`), TW treats the omission as a deletion — the field is absent
after import. This means:

> **Omit ≠ Preserve. Omit = Clear.**

**Implication for sync (LWW design):** When constructing the import JSON for a TW→CalDAV
sync, include all fields you want to keep. Any field omitted from the import payload will
be cleared. This is a footgun — always round-trip all known fields.

---

## `task import` Behavior

### `task import` preserves `modified` — does not mutate it

Re-importing an existing task via `task import` does **not** update the `modified` field
to the current timestamp. TW preserves whatever `modified` value is in the imported JSON.

```
Before import: modified=20260226T140818Z
After import:  modified=20260226T140818Z  (unchanged)
```

If the imported JSON carries a newer `modified` value, TW accepts and stores it.

**Implication for LWW sync:** The `modified` timestamp in TW's export is authoritative for
last-write-wins conflict resolution. The sync engine should:
1. Compare `modified` timestamps from TW vs CalDAV VTODO `LAST-MODIFIED`
2. Import the winner's payload — TW will preserve whatever `modified` is in the JSON
3. Never call `task modify` to update fields after an import, as that would mutate `modified`

### `task import` on a pending UUID: updates, does not duplicate

Importing a task whose UUID already exists as a pending task **updates** the existing task.
No duplicate is created. Count remains 1.

**Implication:** Safe to re-import the same UUID multiple times with updated data.

### `task import` on a deleted UUID: RESURRECTS the task

Importing a task whose UUID exists in TW as `deleted`, but with `status: pending` in the
import JSON, **resurrects** the task as pending.

```
Before: UUID exists, status=deleted
Import: {uuid: "...", status: "pending", ...}
After:  UUID exists, status=pending
```

**Implication for sync (critical):** This means CalDAV→TW sync can accidentally resurrect
deleted tasks if a VTODO still exists on the CalDAV server after the TW task was deleted.
The sync engine **must** track or check deletion state before importing:

- Option A: Maintain a "deleted UUIDs" log and skip import for known-deleted UUIDs.
- Option B: After import, check if the TW side's `modified` predates the deletion; if so,
  re-delete.
- Option C (selected): Use the two-layer loop-prevention design (see `loop-prevention.md`).
  CalDAV deletion of the VTODO propagates first; once removed from CalDAV, TW deletion
  won't resurface.

### `task import` with status:completed or status:deleted works

Both `status: "completed"` and `status: "deleted"` are valid in the import JSON payload.
TW stores them as-is. The `end` field is also preserved from the import payload.

---

## Status Transitions

| Transition | Mechanism | Works? |
|------------|-----------|--------|
| pending → completed | `task <uuid> done` | Yes |
| pending → deleted | `task <uuid> delete` | Yes |
| pending → deleted (alt) | `task <uuid> modify status:deleted` | Yes, exit 0 |
| deleted → pending | `task import` with `status:pending` | Yes (resurrection!) |
| any → completed | `task import` with `status:completed` | Yes |
| any → deleted | `task import` with `status:deleted` | Yes |

### `task delete` is NOT idempotent

Calling `task <uuid> delete` on an already-deleted task exits 1 with:
```
Task X 'description' is not deletable.
Deleted 0 tasks.
```

**Implication:** The sync engine must guard against double-delete by checking task status
before calling `task delete`, or by treating exit code 1 with this specific message as
a non-error.

---

## `status.not:purged` Filter

The filter expression `status.not:purged` is **valid** in TW 3.4.2 (exit 0). Both
`status:purged` and `status.not:purged` are accepted by the TW filter engine.

**Implication:** The sync engine can safely use `status.not:purged` in export filters
to exclude purged tasks (if TW ever exposes them). For normal operation, the default
`task export` already excludes deleted/completed tasks unless `all` is specified.

---

## Expired `wait` Date in Export

A task added with `wait:yesterday` (past date) exports with `status: "pending"` — not
`status: "waiting"`. The `wait` field is still present in the JSON with the past timestamp.

```json
{
  "status": "pending",
  "wait": "20260224T230000Z"
}
```

**Implication for CalDAV mapping:** When mapping TW tasks to VTODO, a task is "active"
(NEEDS-ACTION) if `status == "pending"`, regardless of whether a `wait` field is present.
The sync engine does not need special handling for expired-wait tasks — they are already
pending. For future-dated `wait`, the task would appear as `status: "waiting"` in the
export (this is the normal TW behavior for tasks not yet due).

---

## Decision Table for Sync Engine

| Scenario | Action |
|----------|--------|
| CalDAV VTODO updated | Import JSON with updated fields; `modified` is preserved |
| CalDAV VTODO deleted | `task <uuid> delete`; guard against exit 1 |
| TW task deleted, CalDAV VTODO still exists | Delete VTODO on CalDAV; do not re-import |
| TW field should be cleared (e.g., no due date) | `task modify field:` or re-import without field |
| TW task completed | Import with `status: completed` + `end` timestamp |
| Conflict (both sides modified) | Compare `modified` vs VTODO `LAST-MODIFIED`; import winner |
