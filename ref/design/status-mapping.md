# Status Mapping: Taskwarrior ↔ CalDAV VTODO

## Overview

Taskwarrior and CalDAV VTODO use different status vocabularies. This document
defines the canonical mapping between the two, for both directions:
TW → CalDAV and CalDAV → TW.

---

## Status vocabularies

**Taskwarrior statuses:** `pending`, `waiting`, `completed`, `deleted`, `recurring`

**CalDAV VTODO statuses (RFC 5545):** `NEEDS-ACTION`, `IN-PROCESS`, `COMPLETED`, `CANCELLED`

---

## TW → CalDAV

| TW status | CalDAV STATUS | Additional fields |
|-----------|--------------|-------------------|
| `pending` | `NEEDS-ACTION` | — |
| `waiting` | `NEEDS-ACTION` | `X-TASKWARRIOR-WAIT` = wait date |
| `completed` | `COMPLETED` | `COMPLETED` timestamp = TW completion date |
| `deleted` | *(item removed)* | CalDAV item is hard-deleted; see [deletion-detection.md] |
| `recurring` | *(skipped)* | Logged as a warning; not synced in v1 |

### Notes on `waiting`

`waiting` is a TW-specific concept (hide a task until a future date). It has no
standard CalDAV equivalent.

- The `STATUS` stays `NEEDS-ACTION` so other CalDAV clients can see and interact
  with the task.
- The wait date is stored in the custom property `X-TASKWARRIOR-WAIT` (ISO 8601
  datetime).
- Non-TW CalDAV clients will not see the wait date. This is a known, accepted
  limitation: `waiting` is not expressible to generic CalDAV clients.

### Notes on `scheduled`

TW's `scheduled` field maps to CalDAV `DTSTART`.

- `DTSTART` = TW `scheduled` (when the user plans to work on this task)
- `X-TASKWARRIOR-WAIT` = TW `wait` (when TW should start showing this task)
- Both can be present simultaneously on a single VTODO.

---

## CalDAV → TW

| CalDAV STATUS | Additional condition | TW status | Notes |
|---------------|---------------------|-----------|-------|
| `NEEDS-ACTION` | no `X-TASKWARRIOR-WAIT` | `pending` | — |
| `NEEDS-ACTION` | `X-TASKWARRIOR-WAIT` present and in the future | `waiting` | wait date = value of `X-TASKWARRIOR-WAIT` |
| `NEEDS-ACTION` | `X-TASKWARRIOR-WAIT` present but in the past | `pending` | wait expired; treat as normal pending |
| `COMPLETED` | — | `completed` | — |
| `IN-PROCESS` | — | `pending` | TW has no in-progress status; start timestamp preserved if `DTSTART` is present |
| `CANCELLED` | — | *(skipped)* | Logged as a warning; see below |

### Notes on `IN-PROCESS`

TW has no equivalent status. The task is imported as `pending`. If the CalDAV
item carries a `DTSTART` value, it is preserved as TW's `scheduled` field,
giving a partial signal that work had begun.

On the next sync, the task will be written back as `NEEDS-ACTION` (with
`DTSTART` if scheduled is set). This is a semantic demotion from the CalDAV
client's perspective and is a known limitation.

### Notes on `CANCELLED`

`CANCELLED` items are always skipped on CalDAV → TW sync, regardless of whether a paired TW
task exists. A warning is logged.

**Rationale:** importing `CANCELLED` as `deleted` creates a round-trip hazard —
the task would be deleted from TW and then the CalDAV item would be removed on
the next sync. Skipping preserves the CalDAV record and avoids silent data loss.

Non-TW CalDAV clients that cancel tasks should not expect those cancellations to
propagate into TW.

---

## Field mapping for dates

| TW field | CalDAV property | Direction | Notes |
|----------|-----------------|-----------|-------|
| `scheduled` | `DTSTART` | TW ↔ CalDAV | Standard field; visible to all clients |
| `wait` | `X-TASKWARRIOR-WAIT` | TW ↔ CalDAV | Custom property; opaque to non-TW clients |
| `due` | `DUE` | TW ↔ CalDAV | Standard field |
| `end` / completion date | `COMPLETED` | TW ↔ CalDAV | Only present when status = COMPLETED |

---

## Known limitations

1. **`waiting` invisible to third-party clients.** A task in `waiting` state
   appears as a normal `NEEDS-ACTION` task in non-TW CalDAV clients. The wait
   date is carried in `X-TASKWARRIOR-WAIT`, which generic clients ignore. There
   is no way to set or edit a TW wait date from a third-party client.

2. **`IN-PROCESS` is demoted on round-trip.** A task marked `IN-PROCESS` in a
   CalDAV client is imported as `pending` and written back as `NEEDS-ACTION`. The
   in-progress status is not preserved.

3. **`CANCELLED` does not propagate to TW.** Cancellations made in a CalDAV
   client are silently ignored (with a logged warning). There is no TW
   equivalent.

4. **`recurring` not synced in v1.** TW recurring tasks are skipped with a
   warning. See [recurring-tasks.md] (v2 scope).
