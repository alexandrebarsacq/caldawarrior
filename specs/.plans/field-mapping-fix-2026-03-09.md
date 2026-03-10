# field-mapping-fix

## Mission

Fix four root-cause field-mapping bugs (RC-1 through RC-4) so that SUMMARY↔description,
DESCRIPTION↔annotations, PRIORITY↔priority, and project injection from config all work
correctly in both sync directions.

## Objectives

- RC-1: `caldav_to_tw_fields` reads `VTODO.SUMMARY` (not DESCRIPTION) as TW `description`
- RC-2: `build_vtodo_from_tw` writes TW `description` to SUMMARY only; annotations go to DESCRIPTION
- RC-3: `build_tw_task_from_caldav` injects `project` from config for CalDAV-only new tasks
- RC-4: PRIORITY is parsed from / serialized to VTODO and mapped bidirectionally to TW priority
- All fixes verified by Rust unit tests and Robot Framework blackbox scenarios S-64–S-68

## Success Criteria

- [ ] `cargo test` passes with ≥ 15 new unit tests covering RC-1 through RC-4 fixes (planned: ~22 across Phases 1–5)
- [ ] RF suite: S-64 through S-68 all pass (S-68 may be skipped if multi-calendar env not available)
- [ ] TW task created from a CalDAV VTODO with only SUMMARY has correct `description`
- [ ] TW task created from a CalDAV VTODO with PRIORITY:1 has `priority = "H"`
- [ ] CalDAV task in a project calendar results in TW task with matching `project` field
- [ ] TW task with annotation syncs DESCRIPTION to CalDAV; SUMMARY ≠ DESCRIPTION
- [ ] Round-trip: TW → CalDAV → TW preserves description, annotations, priority

## Assumptions

- Annotation strategy: Option C (single-slot); see Annotation Slot Invariant section below
- `"(no title)"` is a valid non-empty TW description string (TW enforces non-empty, not content constraints)
- `"(no title)"` round-trip: `build_vtodo_from_tw` must reverse-map `description == "(no title)"` back to `vtodo.summary = None` so CalDAV is not permanently mutated
- `TWTask.annotations` is typed as `Vec<TwAnnotation>` (not `Option<Vec<>>`) with `#[serde(default, skip_serializing_if = "Vec::is_empty")]`; empty vec = "no annotations", eliminating the `None` vs `Some(vec![])` dual-None problem
- Priority normalization is intentional: TW always writes canonical iCal values (1, 5, 9); VTODO entries with priorities 2–4 are normalized to 1 after the first TW→CalDAV sync (documented in README)
- `tw_date` serde module already exists in `src/types.rs` (lines 10–32) and handles `YYYYMMDDTHHMMSSZ` format — the same format TW uses for annotation `entry` fields; no new module required
- `build_ir` already receives `config: &Config` — confirmed from source; no signature change needed for Phase 4
- When `caldav_to_tw_fields` receives `annotations_text = Some("")` or whitespace-only, it must treat it identically to `None` (no-op, preserves existing TW annotations); this is part of the DESCRIPTION parsing contract
- Annotation slot-0 replacement uses `entry: now` (sync timestamp), not the original annotation's entry; repeated CalDAV edits always show a fresh "created at sync time" timestamp — this is intentional and acceptable
- "default" project name in config maps to `project: None` in TW (not the string "default")
- Priority mapping: iCal 1–4→H, 5→M, 6–9→L, 0/absent→None; TW H→1, M→5, L→9
- `VTODO.summary` already exists as `Option<String>` in `src/types.rs`; SUMMARY is already parsed
- `TWTask.priority` already exists as `Option<String>` in `src/types.rs`
- `TWTask.project` already exists as `Option<String>` in `src/types.rs` with skip-if-none serde
- `build_ir` already receives `config: &Config` — no signature change needed for Phase 4
- `build_tw_task_from_caldav` does NOT currently receive `now: DateTime<Utc>` — Phase 5 must add this parameter and thread it from `execute_op` (which already has `now`)
- `TwCalDavFields` is only consumed in `src/sync/writeback.rs` — renaming its fields only requires updating that file and its tests
- S-65 RF test: the helper keyword must create a VTODO with the SUMMARY *line absent* (not `SUMMARY:` with empty value) to ensure the parser produces `vtodo.summary = None`
- Project injection for paired entries (RC-3): only CalDAV-only new tasks get project from config; for paired entries, project is inherited from the existing TW task (unchanged behavior)
- S-68 skip mechanism: use `Skip If    '%{MULTI_CALENDAR_ENABLED:=false}' != 'true'    Multi-calendar env not configured` (RF OS-env syntax `%{VAR}` with default; consistent with project convention)

## Annotation Slot Invariant

The annotation vector is managed with a stable-index contract:

> **Index 0 = CalDAV-managed slot** (maps to / from VTODO `DESCRIPTION`).
> **Index 1+ = user-only slots** (TW-internal; never written to CalDAV; never removed by CalDAV sync).

This invariant drives all branch logic in `build_vtodo_from_tw` and `build_tw_task_from_caldav`:

**TW → CalDAV (`build_vtodo_from_tw`):**
- `annotations[0].description` → `vtodo.description` (if annotations non-empty)
- Empty or None annotations → `vtodo.description = None`
- **Exception:** if `task.description == "(no title)"` → `vtodo.summary = None` (reverse sentinel mapping)

**CalDAV → TW (`build_tw_task_from_caldav`, `annotations_text = Some(text)`):**
- base annotations = 0 (empty vec): create `vec![TwAnnotation { entry: now, description: text }]`
- base annotations = 1, text identical: no-op (leave slot 0 unchanged)
- base annotations = 1, text differs: replace → `[TwAnnotation { entry: now, description: text }]`
- base annotations ≥ 2, text identical: no-op (leave slot 0 unchanged, preserve slots 1+)
- base annotations ≥ 2, text differs: replace slot 0 → `[TwAnnotation { entry: now, description: text }] + base[1..]`

**CalDAV → TW (`build_tw_task_from_caldav`, `annotations_text = None`):**
- Treat as **no-op**: leave existing TW annotations unchanged. Rationale: a CalDAV client removing DESCRIPTION is ambiguous (intentional or race); preserving TW annotations avoids data loss. This is a deliberate exception to the LWW mirror policy.

## LWW Merge Policy for Writeback

When updating a TW task from CalDAV data (`build_tw_task_from_caldav`), each CalDAV-managed field mirrors the CalDAV state directly, with two explicit exceptions:

- **Priority**: `priority: fields.priority` — no fallback to `base`. If CalDAV removes PRIORITY, TW `priority` becomes `None`.
- **Exception — annotations None is a no-op**: If `annotations_text` is `None`, TW annotations are preserved unchanged (see Annotation Slot Invariant above for rationale).
- **Exception — user-only annotation slots**: Slots at index 1+ are never modified by CalDAV sync.
- Fields not managed by caldawarrior (`tags`, `urgency`, `id`) are still inherited from `base`.

## Constraints

- Must not break existing Rust unit tests or RF scenarios (S-60 through S-63)
- `IREntry.project`, `TWTask.annotations`, `VTODO.priority` additions must use backward-compatible serde (`#[serde(default, skip_serializing_if = "Option::is_none")]`)
- No changes to CalDAV protocol layer, LWW timestamp logic, or dependency resolution
- Annotation strategy is Option C (single-slot per invariant above); do NOT implement Option A or B
- **Known limitation:** deleting DESCRIPTION in a CalDAV client does not remove the TW annotation (the `annotations_text = None` → no-op policy). Users must delete annotations from the TW side. Document in README.
- **Known limitation:** existing paired TW tasks do not retroactively inherit project from CalDAV calendar config; only newly-created CalDAV-only tasks receive project injection. Document in README.
- **DESCRIPTION parsing contract:** `caldav_to_tw_fields` must normalize DESCRIPTION before using it — treat `Some("")` and `Some(whitespace-only)` identically to `None` (no-op for annotations). Also apply `.trim()` to PRIORITY value during iCal parsing to handle trailing whitespace/CR from some CalDAV clients.

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Renaming `TwCalDavFields.description` → `summary` breaks callers | high | medium | Update all call sites in writeback.rs and tests in same phase; Rust compiler catches missed sites |
| `build_tw_task_from_caldav` `now` parameter threading | medium | medium | `execute_op` already has `now`; only `build_tw_task_from_caldav` and its two call sites need updating |
| `TwAnnotation` serde field name mismatch with real TW JSON format | medium | medium | Add roundtrip test deserializing a real `task export` JSON annotation fragment; verify `entry` uses tw_date format |
| `"(no title)"` permanently mutates CalDAV tasks that had no SUMMARY | was-risk | n/a | Resolved: reverse-maps `"(no title)"` → `vtodo.summary = None` in `tw_to_caldav_fields` |
| `"(no title)"` sentinel collides with a legitimate user task title | low | medium | Round-trip is stable; CalDAV display is title-less (some clients may reject); document in README |
| iCal PRIORITY 2–4 normalized to 1 on first TW→CalDAV sync (lossy) | medium | medium | Document in README that caldawarrior uses three canonical values (1, 5, 9) |
| CalDAV DESCRIPTION deletion not mirrored to TW annotations | high | low | No-op by design; document as known limitation in README |
| S-68 test environment lacks multi-calendar config | medium | low | `Skip If '%{MULTI_CALENDAR_ENABLED:=false}' != 'true'`; document CI coverage gap if env not set |
| Priority deletion (CalDAV removes PRIORITY) silently keeps old TW priority | was-risk | n/a | Resolved: use plain `fields.priority` assignment per LWW merge policy |

## Open Questions

- None — `docs/bug4-investigation.md` and review feedback fully resolve all design decisions

## Dependencies

- No external dependencies; all changes are within the caldawarrior Rust codebase

## Phases

### Phase 1: Data Structures (types.rs)

**Goal:** Add `TwAnnotation` struct and `annotations` field to `TWTask`; add `priority` field to `VTODO`; add `project` field to `IREntry`.

**Description:** Extend core data types. All additions are purely additive and must not break existing serialization tests.

#### Tasks

- **Add TwAnnotation struct and annotations to TWTask** `implementation` `low`
  - Description: Add `#[derive(Debug, Clone, Serialize, Deserialize)] pub struct TwAnnotation { #[serde(with = "tw_date")] pub entry: DateTime<Utc>, pub description: String }` in `src/types.rs`. Add `#[serde(default, skip_serializing_if = "Vec::is_empty")] pub annotations: Vec<TwAnnotation>` to `TWTask` (use `Vec<>` not `Option<Vec<>>` to avoid dual-None semantics). Add a roundtrip unit test deserializing a real TW JSON fragment: `{"entry":"20260309T120000Z","description":"check expiry date"}`.
  - File: src/types.rs
  - Acceptance criteria:
    - `TwAnnotation` has `#[derive(Debug, Clone, Serialize, Deserialize)]`, `entry: DateTime<Utc>` (with tw_date), `description: String`
    - `TWTask.annotations` is `Vec<TwAnnotation>` with `#[serde(default, skip_serializing_if = "Vec::is_empty")]`
    - Existing `tw_task_roundtrip_minimal` test still passes (annotations defaults to empty vec)
    - New test deserializes `"annotations": [{"entry":"20260309T120000Z","description":"check expiry date"}]` and asserts `annotations[0].description == "check expiry date"`
  - Dependencies: None

- **Add priority field to VTODO** `implementation` `low`
  - Description: Add `#[serde(default, skip_serializing_if = "Option::is_none")] pub priority: Option<u8>` to `VTODO` struct.
  - File: src/types.rs
  - Acceptance criteria:
    - `VTODO.priority` is `Option<u8>` with skip-if-none serde
    - Existing `test_round_trip_basic` in ical.rs still passes
  - Dependencies: None

- **Add project field to IREntry** `implementation` `low`
  - Description: Add `#[serde(default, skip_serializing_if = "Option::is_none")] pub project: Option<String>` to `IREntry` struct. Default is None.
  - File: src/types.rs
  - Acceptance criteria:
    - `IREntry.project` is `Option<String>` with serde default and skip-if-none
    - All existing IR tests still pass (project defaults to None for all existing entries)
  - Dependencies: None

#### Verification

- **Run tests:** `cargo test`
- **Fidelity review:** Compare struct additions to spec requirements
- **Manual checks:** After adding `priority` to `VTODO`, run `grep -n "VTODO {" src/` to audit all struct literal sites; add `priority: None` to each (or use `..Default::default()` where `VTODO` derives `Default`). The Phase 1 "Add priority field" task implicitly includes this sweep — the Rust compiler will catch every missed site.

---

### Phase 2: iCal Layer (ical.rs)

**Goal:** Parse `PRIORITY` from VTODO iCal text and serialize it back.

**Description:** Add a `"PRIORITY"` match arm in `from_icalendar_string` and emit `PRIORITY:{n}` in `to_icalendar_string`. Add 4 unit tests.

#### Tasks

- **Parse PRIORITY in from_icalendar_string** `implementation` `low`
  - Description: Add `let mut priority: Option<u8> = None;` initialization. Add match arm `"PRIORITY" => { priority = value.trim().parse::<u8>().ok().filter(|&v| v > 0); }` (`.trim()` handles trailing whitespace/CR from some CalDAV clients). Include `priority` in the `Ok(VTODO { ... })` return value.
  - File: src/ical.rs
  - Acceptance criteria:
    - VTODO string with `PRIORITY:3` → `vtodo.priority == Some(3)`
    - VTODO string with `PRIORITY:0` → `vtodo.priority == None`
    - VTODO string without PRIORITY → `vtodo.priority == None`
  - Dependencies: Phase 1 (VTODO.priority field)

- **Emit PRIORITY in to_icalendar_string** `implementation` `low`
  - Description: After the `RRULE` emission block, add: `if let Some(p) = vtodo.priority { lines.push(format!("PRIORITY:{}", p)); }`.
  - File: src/ical.rs
  - Acceptance criteria:
    - `to_icalendar_string` with `priority = Some(1)` contains `PRIORITY:1`
    - `to_icalendar_string` with `priority = None` does NOT contain `PRIORITY`
    - Round-trip: parse → serialize → parse preserves priority value
  - Dependencies: Phase 1 (VTODO.priority field)

- **Add ical.rs unit tests for PRIORITY** `implementation` `low`
  - Description: Add 4 tests: `priority_parsed_from_vtodo`, `priority_zero_treated_as_absent`, `priority_serialized_to_vtodo`, `priority_absent_not_emitted`.
  - File: src/ical.rs
  - Acceptance criteria:
    - All 4 tests pass per names above
  - Dependencies: Parse and emit tasks above

#### Verification

- **Run tests:** `cargo test`
- **Fidelity review:** Compare PRIORITY parse/emit to RC-4 fix guidance
- **Manual checks:** none

---

### Phase 3: Mapper Layer (mapper/fields.rs)

**Goal:** Fix field structs and conversion functions so SUMMARY↔description, DESCRIPTION↔annotations, and PRIORITY are all correctly mapped.

**Description:** Rename `TwCalDavFields.description` to `summary`; add `annotations` and `priority` fields. Fix `CalDavTwFields` to source from `vtodo.summary`. Update both conversion functions. Add 10 unit tests.

#### Tasks

- **Refactor TwCalDavFields struct** `refactoring` `medium`
  - Description: Rename `description: Option<String>` to `summary: Option<String>`. Add `annotations: Option<String>`. Add `priority: Option<u8>` (iCal integer value 1/5/9 — comment: `// iCal priority value (1/5/9)`). Update doc comments only; no conversion logic here.
  - File: src/mapper/fields.rs
  - Acceptance criteria:
    - `TwCalDavFields` has `summary: Option<String>`, `annotations: Option<String>`, `priority: Option<u8>`
    - No field named `description` on `TwCalDavFields`
  - Dependencies: Phase 1 (TWTask.annotations, VTODO.priority)

- **Refactor CalDavTwFields struct** `refactoring` `low`
  - Description: Fix doc comment on `description` to "VTODO SUMMARY → TW description". Add `annotations_text: Option<String>`. Add `priority: Option<String>` (TW format — comment: `// TW priority string ("H"/"M"/"L")`). Struct definition only — no sourcing logic.
  - File: src/mapper/fields.rs
  - Acceptance criteria:
    - `CalDavTwFields` has `description: String`, `annotations_text: Option<String>`, `priority: Option<String>`
    - Doc comment on `description` says "VTODO SUMMARY → TW description"
  - Dependencies: None

- **Update tw_to_caldav_fields function** `implementation` `medium`
  - Description: Change `let description = Some(task.description.clone())` to `let summary = if task.description == "(no title)" { None } else { Some(task.description.clone()) }` (reverse sentinel mapping lives here in the mapper, not in writeback). Populate `annotations`: take `task.annotations.first().map(|a| a.description.clone())`. Populate `priority`: match `task.priority.as_deref()` — `"H"` → `Some(1u8)`, `"M"` → `Some(5u8)`, `"L"` → `Some(9u8)`, other/None → `None`. Return `TwCalDavFields { summary, annotations, priority, due, dtstart, wait, depends }`.
  - File: src/mapper/fields.rs
  - Acceptance criteria:
    - Task `description="Buy milk"` → `summary=Some("Buy milk")`
    - Task `description="(no title)"` → `summary=None` (reverse sentinel)
    - Task with one annotation `{description:"check expiry"}` → `annotations=Some("check expiry")`
    - Task with no annotations → `annotations=None`
    - Task `priority=Some("H")` → `priority=Some(1u8)`
    - Task `priority=None` → `priority=None`
  - Dependencies: Refactor TwCalDavFields struct task above

- **Update caldav_to_tw_fields function** `implementation` `medium`
  - Description: Change description source to `vtodo.summary.clone().unwrap_or_else(|| "(no title)".to_string())`. Add `let annotations_text = vtodo.description.as_deref().filter(|s| !s.trim().is_empty()).map(str::to_owned)` (empty/whitespace DESCRIPTION treated as None per DESCRIPTION parsing contract). Add priority conversion: `Some(1..=4)` → `Some("H")`, `Some(5)` → `Some("M")`, `Some(6..=9)` → `Some("L")`, `_ =>` `None` (catch-all; `Some(0)` is impossible after Phase 2 filter but covered by catch-all). Return updated `CalDavTwFields`.
  - File: src/mapper/fields.rs
  - Acceptance criteria:
    - VTODO `summary=Some("X")`, `description=None` → `description=="X"`, `annotations_text==None`
    - VTODO `summary=Some("X")`, `description=Some("note")` → `description=="X"`, `annotations_text==Some("note")`
    - VTODO `summary=None`, `description=Some("note")` → `description=="(no title)"`, `annotations_text==Some("note")`
    - VTODO `priority=Some(1)` → `Some("H")`; `Some(2)` → `Some("H")`; `Some(3)` → `Some("H")`; `Some(4)` → `Some("H")`
    - VTODO `priority=Some(5)` → `Some("M")`; `Some(9)` → `Some("L")`; `None` → `None`
  - Dependencies: Refactor CalDavTwFields struct task above; Phase 1 (VTODO.priority)

- **Update writeback.rs field references after struct rename (compilation fix)** `refactoring` `low`
  - Description: Last task in Phase 3. Mechanical find-replace in `src/sync/writeback.rs` — rename `fields.description` → `fields.summary` so the codebase compiles after the struct rename. No annotation/priority/project logic yet; that remains in Phase 5. This stub keeps `cargo test` green between phases.
  - File: src/sync/writeback.rs
  - Acceptance criteria:
    - `cargo test` compiles and passes at end of Phase 3
    - No functional logic changed — only field name references updated
  - Dependencies: Refactor TwCalDavFields struct task above

- **Add mapper unit tests** `implementation` `low`
  - Description: Add 11 tests: `caldav_summary_mapped_to_tw_description`, `caldav_both_summary_and_description_present`, `caldav_description_mapped_to_annotations`, `caldav_no_summary_gives_no_title_sentinel`, `tw_description_becomes_summary`, `tw_no_title_becomes_absent_summary`, `tw_annotations_become_description`, `priority_tw_to_caldav_h`, `priority_caldav_to_tw_1_gives_h`, `priority_caldav_2_3_4_give_h`, `priority_caldav_5_gives_m_9_gives_l_0_gives_none`. Update existing tests that use `fields.description` to use `fields.summary`.
  - File: src/mapper/fields.rs
  - Acceptance criteria:
    - All 11 new tests pass; existing tests updated and passing
  - Dependencies: Both update function tasks above

#### Verification

- **Run tests:** `cargo test`
- **Fidelity review:** Compare mapper changes to RC-1, RC-2, RC-4 fix guidance
- **Manual checks:** none

---

### Phase 4: IR Layer — Project for CalDAV-only Entries (ir.rs)

**Goal:** Populate `IREntry.project` for CalDAV-only entries by reverse-looking up the project name from `config.calendars`.

**Description:** Add a `resolve_project_from_url` helper. In `build_ir` CalDAV-only pass, call it to set `entry.project`. Add 2 unit tests.

#### Tasks

- **Add resolve_project_from_url helper** `implementation` `low`
  - Description: In `src/ir.rs`, define `const DEFAULT_PROJECT: &str = "default"; // Reserved name (exact lowercase match) that opts a calendar out of project injection; "Default" or "DEFAULT" are treated as real project names`. Add `fn resolve_project_from_url(url: &str, config: &Config) -> Option<String>` that normalizes both URLs before comparing (`trim_end_matches('/')`) and filters out `DEFAULT_PROJECT`: `config.calendars.iter().find(|c| c.url.trim_end_matches('/') == url.trim_end_matches('/')).map(|c| c.project.clone()).filter(|p| p != DEFAULT_PROJECT)`.
  - File: src/ir.rs
  - Acceptance criteria:
    - URL matching `project="work"` → `Some("work")`
    - URL matching `project="default"` → `None`
    - URL not matching any entry → `None`
    - URL differing only by trailing slash (e.g., `"http://dav/work/"` vs `"http://dav/work"`) → still matches correctly
    - `"Default"` (different case) does NOT match `DEFAULT_PROJECT` → returns `Some("Default")`
  - Dependencies: Phase 1 (IREntry.project field)

- **Populate entry.project in build_ir CalDAV-only pass** `implementation` `low`
  - Description: In the CalDAV-only pass of `build_ir`, compute `let project = resolve_project_from_url(&calendar_url, config)` and set `project` on the new `IREntry`.
  - File: src/ir.rs
  - Acceptance criteria:
    - CalDAV-only entry with `calendar_url` matching `project="work"` → `entry.project = Some("work")`
    - CalDAV-only entry with `calendar_url` matching `project="default"` → `entry.project = None`
    - TW-only and paired entries still have `entry.project = None`
  - Dependencies: resolve_project_from_url task above

- **Add IR unit tests for project injection** `implementation` `low`
  - Description: Add 2 tests: `caldav_only_entry_gets_project_from_config` and `caldav_only_entry_with_default_project_gets_none`.
  - File: src/ir.rs
  - Acceptance criteria:
    - `caldav_only_entry_gets_project_from_config`: config `[("work","http://dav/work/")]`, VTODO in work calendar → `entry.project == Some("work")`
    - `caldav_only_entry_with_default_project_gets_none`: config `[("default","http://dav/cal/")]`, VTODO in default calendar → `entry.project == None`
    - Existing IR tests unaffected
  - Dependencies: Populate entry.project task above

#### Verification

- **Run tests:** `cargo test`
- **Fidelity review:** Compare IREntry and build_ir changes to RC-3 fix guidance
- **Manual checks:** none

---

### Phase 5: Writeback Layer (sync/writeback.rs)

**Goal:** Fix `build_vtodo_from_tw` and `build_tw_task_from_caldav`; add `now` parameter; apply Annotation Slot Invariant and LWW merge policy.

**Description:** Thread `now: DateTime<Utc>` into `build_tw_task_from_caldav`. Fix all four field mappings. Apply the Annotation Slot Invariant for all branch cases. Reverse-map `"(no title)"` sentinel in `build_vtodo_from_tw`. Add 3 unit tests.

#### Tasks

- **Add now parameter to build_tw_task_from_caldav** `refactoring` `low`
  - Description: Add `now: DateTime<Utc>` parameter. Update the two call sites inside `execute_op` (PullFromCalDav and ResolveConflict CalDAVWins branches) to pass `now`. Update any test helpers that call `build_tw_task_from_caldav` directly using a fixed deterministic timestamp (e.g., `Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()`) — never `Utc::now()` in tests, to keep annotation timestamps deterministic and assertable. The Rust compiler will catch all missed call sites.
  - File: src/sync/writeback.rs
  - Acceptance criteria:
    - `build_tw_task_from_caldav` signature includes `now: DateTime<Utc>`
    - All call sites (production and test) compile and pass `now`
  - Dependencies: None

- **Fix build_vtodo_from_tw** `implementation` `low`
  - Description: Change `summary: fields.description.clone(), description: fields.description` to `summary: fields.summary.clone(), description: fields.annotations.clone()`. Add `priority: fields.priority` to the VTODO struct literal. The reverse sentinel (`"(no title)"` → `summary=None`) is handled upstream in `tw_to_caldav_fields` (Phase 3); `build_vtodo_from_tw` blindly uses `fields.summary`.
  - File: src/sync/writeback.rs
  - Acceptance criteria:
    - `vtodo.summary` = `fields.summary` (may be None when TW description was "(no title)")
    - `vtodo.description` = first annotation text, or None if no annotations
    - `vtodo.priority` = `fields.priority` (e.g., TW `priority="H"` → `vtodo.priority == Some(1)`)
    - `vtodo.summary != vtodo.description` when task has no annotations
  - Dependencies: Phase 3 (TwCalDavFields.summary/annotations/priority)

- **Fix build_tw_task_from_caldav — priority, annotations, project** `implementation` `medium`
  - Description: Per LWW merge policy and Annotation Slot Invariant: (1) Priority: `priority: fields.priority` — no fallback to `base`. (2) Project: `project: base.map_or_else(|| entry.project.clone(), |t| t.project.clone())`. (3) Annotations: implement all 6 branch cases from the Annotation Slot Invariant section (0/1/≥2 × Some(text)/None combinations). `annotations_text = None` is a no-op (leave existing TW annotations unchanged). For `Some(text)`: replace slot 0 only if text differs; preserve slots 1+.
  - File: src/sync/writeback.rs
  - Acceptance criteria:
    - CalDAV task with `PRIORITY:1` → TW `priority = Some("H")`
    - CalDAV task with `PRIORITY:None` → TW `priority = None` (no fallback to base)
    - CalDAV task with `DESCRIPTION:text`, base has 0 annotations → one annotation created
    - CalDAV task with `DESCRIPTION:text`, base has 1 annotation and identical text → no-op
    - CalDAV task with `DESCRIPTION:text`, base has 1 annotation and different text → annotation replaced
    - CalDAV task with `DESCRIPTION:text`, base has 2+ annotations, different text → slot 0 replaced, slots 1+ preserved
    - CalDAV task with `DESCRIPTION:text`, base has 2+ annotations, identical text → no-op (slots 0, 1+ all unchanged)
    - CalDAV task with no DESCRIPTION, base has 1 annotation → no-op (annotation preserved)
    - CalDAV-only entry (`base = None`) with `entry.project = Some("work")` → TW `project = Some("work")`
    - Paired entry: project inherited from existing TW task
  - Dependencies: Phase 3 (CalDavTwFields), Phase 4 (IREntry.project), now parameter task above

- **Add writeback unit tests** `implementation` `low`
  - Description: Add 3 tests: `build_vtodo_from_tw_uses_summary_not_description`, `build_tw_task_caldav_only_injects_project`, `build_tw_task_reads_summary_as_description`. Update existing test helpers if needed (e.g., `make_vtodo` that sets both summary and description to "Task" may need adjustment for the now-correct mapping).
  - File: src/sync/writeback.rs
  - Acceptance criteria:
    - `build_vtodo_from_tw_uses_summary_not_description`: `vtodo.summary == Some("Buy milk")`, `vtodo.description == None` (no annotations)
    - `build_tw_task_caldav_only_injects_project`: CalDAV-only entry, `entry.project=Some("work")` → `tw_task.project==Some("work")`
    - `build_tw_task_reads_summary_as_description`: `vtodo.summary=Some("X")` → `tw_task.description=="X"`
    - Existing writeback tests still pass
  - Dependencies: Fix tasks above

#### Verification

- **Run tests:** `cargo test`
- **Fidelity review:** Compare writeback changes to RC-2, RC-3, RC-4 fix guidance
- **Manual checks:** none

---

### Phase 6: Robot Framework Tests (S-64–S-68)

**Goal:** Add 5 new blackbox test scenarios to `tests/robot/suites/07_field_mapping.robot`.

**Description:** Add scenarios S-64 through S-68. S-65 expects `"(no title)"` sentinel. S-67 tests three PRIORITY values. S-68 may be skipped if multi-calendar env not configured.

#### Tasks

- **Add S-64: CalDAV SUMMARY-only task → TW description** `implementation` `low`
  - Description: Create a CalDAV VTODO with SUMMARY set, no DESCRIPTION line; sync; assert TW task `description == SUMMARY value`.
  - File: tests/robot/suites/07_field_mapping.robot
  - Acceptance criteria:
    - VTODO with `SUMMARY:Buy oat milk` → TW `description == "Buy oat milk"`
  - Dependencies: Phase 5 complete

- **Add S-65: CalDAV DESCRIPTION-only task → TW sentinel and annotation** `implementation` `low`
  - Description: Create a CalDAV VTODO with the SUMMARY line *absent* (not empty), DESCRIPTION present; sync; assert TW `description == "(no title)"` and one annotation with the DESCRIPTION text.
  - File: tests/robot/suites/07_field_mapping.robot
  - Acceptance criteria:
    - VTODO with no SUMMARY line and `DESCRIPTION:A note about milk` → TW `description == "(no title)"`
    - TW task has annotation with description "A note about milk"
    - SUMMARY line is truly absent (keyword must not emit `SUMMARY:`)
  - Dependencies: Phase 5 complete

- **Add S-66: TW task with annotation → CalDAV DESCRIPTION set, SUMMARY not duplicated** `implementation` `low`
  - Description: Create TW task with description and annotation; sync to CalDAV; assert `SUMMARY == task description`, `DESCRIPTION == annotation text`, and `SUMMARY ≠ DESCRIPTION`.
  - File: tests/robot/suites/07_field_mapping.robot
  - Acceptance criteria:
    - VTODO `SUMMARY` == TW description; `DESCRIPTION` == annotation text; SUMMARY ≠ DESCRIPTION
  - Dependencies: Phase 5 complete

- **Add S-67: CalDAV PRIORITY → TW priority mapping** `implementation` `low`
  - Description: Create three CalDAV VTODOs with `PRIORITY:1`, `PRIORITY:5`, `PRIORITY:9`; sync; assert TW `priority H`, `M`, `L` respectively.
  - File: tests/robot/suites/07_field_mapping.robot
  - Acceptance criteria:
    - PRIORITY:1 → `priority == "H"`; PRIORITY:5 → `priority == "M"`; PRIORITY:9 → `priority == "L"` (CalDAV→TW direction)
    - TW task with `priority=H` syncs to CalDAV with `PRIORITY:1` in VTODO (TW→CalDAV direction; extends existing S-60/S-61 pattern)
  - Dependencies: Phase 5 complete

- **Add S-68: CalDAV-only task in project calendar → TW project set** `implementation` `low`
  - Description: VTODO in work calendar; sync; assert TW `project == "work"`. Use `Skip If    '%{MULTI_CALENDAR_ENABLED:=false}' != 'true'    Multi-calendar env not configured` at the top of the test body.
  - File: tests/robot/suites/07_field_mapping.robot
  - Acceptance criteria:
    - CalDAV VTODO in work calendar → TW `project == "work"` (or test is explicitly skipped)
  - Dependencies: Phase 4 and Phase 5 complete

- **Update CATALOG.md with S-64–S-68** `implementation` `low`
  - Description: Add entries for S-64 through S-68 with status markers reflecting actual RF run results.
  - File: tests/robot/CATALOG.md
  - Acceptance criteria:
    - CATALOG.md contains entries for S-64, S-65, S-66, S-67, S-68 with status markers
  - Dependencies: RF tests above

#### Verification

- **Run tests:** `CURRENT_UID=$(id -u) CURRENT_GID=$(id -g) docker compose -f tests/robot/docker-compose.yml run --rm robot`
- **Fidelity review:** Compare RF scenarios to S-64–S-68 descriptions in investigation doc
- **Manual checks:** CATALOG.md updated with status markers

---

### Phase 7: Final Integration Verification

**Goal:** Confirm all phases combine cleanly, all tests pass, no regressions.

#### Tasks

- **Run full test suite** `verification` `low`
  - Description: Run `cargo test` and the Robot Framework suite. Confirm zero failures and ≥ 15 new Rust unit tests.
  - File: N/A
  - Acceptance criteria:
    - `cargo test` exits 0
    - RF suite: S-60–S-67 all pass; S-68 passes or is marked skipped
    - No regressions in existing tests
  - Dependencies: All previous phases completed

- **Fidelity review** `verification` `low`
  - Description: Use `foundry-review` skill to compare implementation against this spec.
  - File: N/A
  - Acceptance criteria:
    - Review verdict is `aligned` or `minor deviations`
    - No critical/high issues unresolved
  - Dependencies: Run full test suite task above

#### Verification

- **Run tests:** `cargo test` + RF docker compose
- **Fidelity review:** `foundry-review` skill
- **Manual checks:** none
