# Bug 4 Investigation: Field Mapping (description / annotations / priority / project)

> **Status:** Findings only — no code has been changed.
> **Date:** 2026-03-09

---

## 1. Field Naming Confusion

Three systems use different vocabulary for the same conceptual fields:

| Concept | Tasks.org / CalDAV (iCal RFC 5545) | TaskWarrior | Notes |
|---------|-------------------------------------|-------------|-------|
| Task title / name | `SUMMARY` | `description` | One-line string |
| Long-form notes | `DESCRIPTION` | `annotations` (list of `{entry, description}`) | Multi-line / multi-entry |
| Priority | `PRIORITY` (integer 1–9, 0=undefined) | `priority` (`"H"`, `"M"`, `"L"`, or absent) | iCal: 1=highest; TW: H=highest |
| Calendar / list | *(implicit: which calendar URL)* | `project` | Injected from `[[calendar]]` config entry |

**Key confusion:** The iCal property named `DESCRIPTION` does **not** map to the TW field named
`description`. The correct mapping is:

- TW `description` ↔ iCal `SUMMARY`
- TW `annotations` ↔ iCal `DESCRIPTION`

This mismatch in naming is the root of RC-1 and RC-2 below.

---

## 2. Current (Broken) Behavior

### CalDAV → TW direction

`src/mapper/fields.rs:104`:
```rust
let description = vtodo.description.clone().unwrap_or_default();
```
This reads `VTODO.DESCRIPTION` (iCal long-form notes) and places it in TW `description` (task title).
It completely ignores `VTODO.SUMMARY`. A task created in Tasks.org / any CalDAV client with a
title in `SUMMARY` and no `DESCRIPTION` will arrive in TaskWarrior with an **empty description**.

`src/sync/writeback.rs:153`:
```rust
priority: base.and_then(|t| t.priority.clone()),
```
Priority is taken only from the existing TW task (`base`). For CalDAV-only tasks (`base = None`),
priority is always `None`, even if the VTODO has a `PRIORITY` property. The VTODO struct has no
`priority` field and the parser does not handle the `PRIORITY` property.

`src/sync/writeback.rs:154`:
```rust
project: base.and_then(|t| t.project.clone()),
```
Project is taken only from the existing TW task. For CalDAV-only new tasks (`base = None`), project
is always `None`. The calendar→project mapping in config is never consulted in this direction.

### TW → CalDAV direction

`src/sync/writeback.rs:78–79`:
```rust
summary: fields.description.clone(),
description: fields.description,
```
TW `description` (task title) is written to **both** `SUMMARY` and `DESCRIPTION`. This is partially
correct: `SUMMARY` is right. But `DESCRIPTION` should carry TW annotations, not the task title.
CalDAV clients receive a redundant duplicate in `DESCRIPTION`.

`src/mapper/fields.rs:55`:
```rust
let description = Some(task.description.clone());
```
`TwCalDavFields.description` carries TW `description`. There is no field in `TwCalDavFields` for
annotations.

---

## 3. Root Causes

### RC-1 — Wrong VTODO field read for TW `description` (CalDAV→TW)

**File:** `src/mapper/fields.rs`, line 104
**File:** `src/mapper/fields.rs`, line 34 (struct field doc comment also says "VTODO DESCRIPTION")

`caldav_to_tw_fields()` reads `vtodo.description` (iCal `DESCRIPTION`) instead of `vtodo.summary`
(iCal `SUMMARY`) when producing the TW task title. The `CalDavTwFields` struct comment at line 34
even documents this wrong mapping.

### RC-2 — TW `description` written to both SUMMARY and DESCRIPTION (TW→CalDAV)

**File:** `src/sync/writeback.rs`, lines 78–79

`build_vtodo_from_tw()` clones `fields.description` into both `VTODO.summary` and
`VTODO.description`. The `SUMMARY` write is correct. `DESCRIPTION` should instead carry a
serialisation of TW annotations (or be absent when there are no annotations).

This means:
- TW→CalDAV produces `DESCRIPTION` = task title (wrong)
- CalDAV→TW reads `DESCRIPTION` as task title (consistent with the wrong TW→CalDAV output, but
  wrong relative to data written by other CalDAV clients)

The two bugs partially mask each other during round-trips that go through caldawarrior only, but
break when Tasks.org or another CalDAV client creates/edits the VTODO.

### RC-3 — `project` not injected from config for CalDAV-only tasks

**File:** `src/sync/writeback.rs`, line 154
**File:** `src/ir.rs`, lines 14–25 (`resolve_calendar_url`)

`build_tw_task_from_caldav()` inherits `project` from the existing TW task (`base`). For new
CalDAV-only tasks (`base = None`), `project` is always `None`. The calendar-to-project mapping
already exists in `resolve_calendar_url()` (which maps project→URL), but the reverse (URL→project)
is never performed during CalDAV→TW construction.

**Additionally (not a separate RC but same area):**

### RC-4 — PRIORITY not mapped at all

**File:** `src/types.rs`, lines 211–230 — `VTODO` struct has no `priority` field
**File:** `src/ical.rs`, lines 50–102 — no `"PRIORITY"` match arm; falls through to `extra_props`
**File:** `src/mapper/fields.rs` — neither `TwCalDavFields` nor `CalDavTwFields` has a priority
**File:** `src/sync/writeback.rs`, line 153 — priority taken from existing TW task only

`PRIORITY` round-trips through `extra_props` at the iCal layer but is never surfaced to the mapper
or applied to the TW task.

---

## 4. Desired Behavior

| TW field | CalDAV VTODO field | Direction | Notes |
|----------|--------------------|-----------|-------|
| `description` | `SUMMARY` | bidirectional | Task title |
| `annotations` (list) | `DESCRIPTION` | bidirectional | Long-form notes |
| `priority` | `PRIORITY` | bidirectional | iCal 1–4 → H, 5 → M, 6–9 → L; absent/0 → None |
| `project` | *(from matched `[[calendar]]` entry)* | CalDAV→TW only | Injected from config on pull; TW→CalDAV direction uses calendar URL routing (unchanged) |

---

## 5. Per-Root-Cause Fix Guidance

### RC-1 fix — `caldav_to_tw_fields` must read `SUMMARY`

**`src/mapper/fields.rs`**

- Change the `description` field of `CalDavTwFields` to be sourced from `vtodo.summary` (not
  `vtodo.description`).
- Add a new field to `CalDavTwFields` (e.g., `annotations_text: Option<String>`) sourced from
  `vtodo.description`. This carries the raw DESCRIPTION string; the caller converts it to a TW
  annotation list.
- Update the doc comment on `CalDavTwFields::description` (line 34) to say `SUMMARY → TW description`.

### RC-2 fix — `build_vtodo_from_tw` must write SUMMARY and DESCRIPTION separately

**`src/sync/writeback.rs`, `src/mapper/fields.rs`**

- Add an `annotations: Vec<String>` (or similar) field to `TwCalDavFields`.
- In `tw_to_caldav_fields()`, populate it from `task.annotations` (once annotations are added to
  `TWTask`; see note below).
- In `build_vtodo_from_tw()`:
  - `summary:` ← `fields.description` (task title) — unchanged
  - `description:` ← serialised annotations (e.g., joined with `\n`), or `None` if empty

**Note on `TWTask.annotations`:** The `TWTask` struct (`src/types.rs`) currently has no
`annotations` field. Add `pub annotations: Option<Vec<TwAnnotation>>` with a serde-compatible
struct for `{entry: DateTime<Utc>, description: String}`. TW exports annotations as a JSON array.

### RC-3 fix — inject project from config in `build_tw_task_from_caldav`

**`src/sync/writeback.rs`**, **`src/ir.rs`** or **`src/types.rs`**

The cleanest approach: store the resolved `project` on `IREntry` at IR-construction time
(symmetrically to how `calendar_url` is stored). Then `build_tw_task_from_caldav` can read
`entry.project` for new CalDAV-only tasks.

Alternatively, pass a `config: &Config` parameter to `build_tw_task_from_caldav` and reverse-look
up project from `entry.calendar_url`:

```
config.calendars.iter()
    .find(|c| c.url == entry.calendar_url.as_deref().unwrap_or(""))
    .map(|c| c.project.clone())
    .filter(|p| p != "default")
```

The `"default"` project name is a sentinel in config; it should map to `project: None` in TW.

### RC-4 fix — add PRIORITY to VTODO struct, parser, and mapper

**`src/types.rs`**

Add `pub priority: Option<u8>` to `VTODO` (iCal integer 0–9; 0 = undefined/absent).

**`src/ical.rs`**

Add a `"PRIORITY"` match arm in `from_icalendar_string()` (after line 75):
```rust
"PRIORITY" => {
    priority = value.parse::<u8>().ok().filter(|&v| v > 0);
}
```
In `to_icalendar_string()`, emit `PRIORITY:{n}` when `vtodo.priority` is `Some(n)`.

**`src/mapper/fields.rs`**

Conversion rules:
- TW→iCal: `"H"` → 1, `"M"` → 5, `"L"` → 9, absent → omit
- iCal→TW: 1–4 → `"H"`, 5 → `"M"`, 6–9 → `"L"`, absent/0 → `None`

Add `priority: Option<u8>` to `TwCalDavFields` and `priority: Option<String>` to `CalDavTwFields`.

**`src/sync/writeback.rs`**

- `build_vtodo_from_tw()`: set `VTODO.priority` from `fields.priority`
- `build_tw_task_from_caldav()`: set `TWTask.priority` from `fields.priority`

---

## 6. TW Annotations Mapping Strategy

TW annotations are a list of objects:
```json
[
  {"entry": "20260309T120000Z", "description": "First note"},
  {"entry": "20260309T130000Z", "description": "Second note"}
]
```

A single iCal `DESCRIPTION` string must represent this list. Possible strategies:

**Option A — One annotation per sync (simplest):**
On CalDAV→TW: create one new annotation per sync with `entry = now` and
`description = DESCRIPTION_content`. Problem: creates duplicate annotations on repeated syncs
unless content-deduplication is applied.

**Option B — Replace / merge by content hash:**
On TW→CalDAV: join all annotation descriptions with `\n` (or a chosen separator).
On CalDAV→TW: compare current DESCRIPTION with the joined existing annotations. If unchanged, do
nothing. If changed, add a new annotation (preserving history). This preserves TW annotation
history while tracking CalDAV edits.

**Option C — Single annotation slot (simplest bidirectional):**
Treat the VTODO DESCRIPTION as a single "notes" blob. On pull, if no annotation exists, add one; if
one exists and its content differs, update it (replace description, update entry timestamp). Ignore
additional annotations beyond the first for CalDAV export purposes — they are TW-internal.

**Recommendation:** Start with Option C for implementation simplicity. Document that TW annotations
beyond the first are not synced to CalDAV. Option B can be implemented later if users request full
annotation history sync.

---

## 7. Robot Framework Tests to Write (S-64–S-68)

### S-64 — CalDAV task with SUMMARY only → TW description populated

```
VTODO body:
  UID: <uuid>
  SUMMARY: Buy oat milk
  STATUS: NEEDS-ACTION
```
Expected: TW task created with `description = "Buy oat milk"`.
Verifies RC-1 fix: SUMMARY is read as TW description.

### S-65 — CalDAV task with DESCRIPTION only (no SUMMARY) → TW description empty or from DESCRIPTION

```
VTODO body:
  UID: <uuid>
  DESCRIPTION: A note about milk
  STATUS: NEEDS-ACTION
```
Expected: TW task created with `description = ""` (or a sentinel); annotation contains "A note about milk".
Verifies RC-1 fix: DESCRIPTION is not used as task title.

### S-66 — TW task with annotation → CalDAV DESCRIPTION set, SUMMARY not duplicated

Create TW task with description "Buy milk" and annotation "check expiry date".
Expected: VTODO has `SUMMARY:Buy milk`, `DESCRIPTION:check expiry date`.
DESCRIPTION must not equal SUMMARY.
Verifies RC-2 fix.

### S-67 — CalDAV task with PRIORITY:1 → TW priority H

```
VTODO body:
  UID: <uuid>
  SUMMARY: Urgent task
  PRIORITY: 1
  STATUS: NEEDS-ACTION
```
Expected: TW task created with `priority = "H"`.
Also test PRIORITY:5 → M, PRIORITY:9 → L.
Verifies RC-4 fix.

### S-68 — CalDAV-only task routed to project calendar → TW project set

Config has `[[calendar]] project = "work" url = "http://…/work/"`.
CalDAV task in the work calendar with no existing TW counterpart.
Expected: TW task created with `project = "work"`.
Verifies RC-3 fix.

---

## 8. Rust Unit Test Guidance

### `src/mapper/fields.rs`

- `caldav_summary_mapped_to_tw_description`: VTODO with `summary = Some("X")`, `description = None`
  → `CalDavTwFields.description == "X"`.
- `caldav_description_mapped_to_annotations`: VTODO with `description = Some("note")` →
  `CalDavTwFields.annotations_text == Some("note")`.
- `caldav_no_summary_gives_empty_description`: VTODO with both fields `None` →
  `CalDavTwFields.description == ""`.
- `tw_description_becomes_summary`: TWTask with `description = "Buy milk"` →
  `TwCalDavFields.summary == Some("Buy milk")`.
- `tw_annotations_become_description`: TWTask with one annotation →
  `TwCalDavFields.description == Some(<annotation text>)`.
- `priority_tw_to_caldav_h`: `priority = Some("H")` → `TwCalDavFields.priority == Some(1)`.
- `priority_caldav_to_tw_1_gives_h`: VTODO `priority = Some(1)` → `CalDavTwFields.priority == Some("H")`.
- `priority_caldav_5_gives_m`, `priority_caldav_9_gives_l`, `priority_caldav_0_gives_none`.

### `src/ical.rs`

- `priority_parsed_from_vtodo`: VTODO string with `PRIORITY:3` → `vtodo.priority == Some(3)`.
- `priority_zero_treated_as_absent`: `PRIORITY:0` → `vtodo.priority == None`.
- `priority_serialized_to_vtodo`: VTODO with `priority = Some(1)` → serialised string contains
  `PRIORITY:1`.
- `priority_absent_not_emitted`: VTODO with `priority = None` → serialised string does not contain
  `PRIORITY`.

### `src/sync/writeback.rs` (integration-level unit tests)

- `build_vtodo_from_tw_uses_summary_not_description`: after `build_vtodo_from_tw`, assert
  `vtodo.summary == Some(tw_description)` and `vtodo.description != Some(tw_description)`.
- `build_tw_task_caldav_only_injects_project`: CalDAV-only entry with `calendar_url` matching a
  config `[[calendar]]` entry → `tw_task.project == Some("work")`.
- `build_tw_task_reads_summary_as_description`: VTODO `summary = "X"` → `tw_task.description == "X"`.

### `src/ir.rs`

- `project_stored_on_caldav_only_irentry`: after `build_ir`, CalDAV-only entry whose calendar URL
  matches a `[[calendar]]` config entry has `entry.project == Some("work")` (if project field is
  added to `IREntry`).

---

## Key File Reference

| File | Relevant lines | Subject |
|------|----------------|---------|
| `src/mapper/fields.rs` | 34, 104 | `CalDavTwFields.description` reads wrong VTODO field |
| `src/mapper/fields.rs` | 55 | `TwCalDavFields` has no annotations field |
| `src/sync/writeback.rs` | 78–79 | VTODO built with both SUMMARY and DESCRIPTION = TW description |
| `src/sync/writeback.rs` | 153 | priority not read from VTODO |
| `src/sync/writeback.rs` | 154 | project not injected from config |
| `src/ical.rs` | 50–102 | No PRIORITY match arm; falls to extra_props |
| `src/types.rs` | 211–230 | VTODO struct: no priority field |
| `src/types.rs` | 133–183 | TWTask struct: no annotations field |
| `src/ir.rs` | 14–25 | `resolve_calendar_url` (project→URL; reverse not yet used) |
| `src/config.rs` | 6–9 | `CalendarEntry { project, url }` |
