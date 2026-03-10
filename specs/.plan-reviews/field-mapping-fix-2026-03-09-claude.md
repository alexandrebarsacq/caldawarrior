# Review Summary

## Critical Blockers
Issues that MUST be fixed before this becomes a spec.

- **None identified.**

---

## Major Suggestions
Significant improvements to strengthen the plan.

- **[Risk]** `"(no title)"` sentinel collision is an active user-facing data risk, not just a resolved concern
  - **Description:** The Assumptions section correctly specifies the sentinel but treats `"(no title)"` as an unambiguous reserved value. In practice, a user can legitimately create a TW task with `description = "(no title)"` via `task add "(no title)"`. When this task syncs to CalDAV, `build_vtodo_from_tw` will produce a VTODO with no SUMMARY line — the CalDAV item loses its title permanently from the CalDAV client's perspective, even though the TW side is fine. The round-trip is stable (TW→CalDAV→TW roundtrip preserves `"(no title)"`), but a user managing tasks in a CalDAV client (e.g., Thunderbird) will see a title-less entry with no explanation.
  - **Impact:** Silent, hard-to-diagnose data presentation issue for any user who names a task exactly `"(no title)"`. Could also cause confusion if CalDAV clients reject VTODO entries without SUMMARY (some clients require it).
  - **Fix:** Add this to the Risks table (likelihood: low, impact: medium). Also: Add a note in the Constraints section and the planned README/docs update. Optionally add a validation warning in `tw_to_caldav_fields` when `task.description == "(no title)"` to log it as a special sentinel case.

- **[Risk]** Annotation `entry` timestamp semantics on slot-0 replacement are unspecified
  - **Description:** The Annotation Slot Invariant specifies all 6 branch cases for `build_tw_task_from_caldav`, but never specifies what `entry` timestamp to use when slot 0 is *replaced* (text differs). The Phase 5 task description says `TwAnnotation { entry: now, description: text }` — meaning the annotation timestamp resets to the sync time on every change. This is a behavioral decision with observable consequences: TW uses annotation timestamps for display (e.g., `task annotate` output) and some urgency/reporting workflows.
  - **Impact:** If a CalDAV client edits the DESCRIPTION field repeatedly, the TW annotation `entry` timestamp will update on every sync, making it appear as if the annotation was always just created. Conversely, preserving the original timestamp across replacement would require passing the base annotation's `entry` through the branch logic.
  - **Fix:** Add an explicit assumption: "When slot 0 is replaced (text differs), the `entry` timestamp is set to `now` (sync time), not the original annotation's `entry`." If preserving the original is preferred, document the additional branch logic required.

- **[Risk]** iCal PRIORITY normalization (2–4→H) silently mutates CalDAV data on first sync
  - **Description:** The Assumptions section documents: "VTODO entries with priorities 2–4 are normalized to 1 after the first TW→CalDAV sync." This is a one-way, lossy transformation. A CalDAV client that sets `PRIORITY:2` (e.g., to express "very high but not highest") will see it permanently overwritten to `PRIORITY:1` after the first sync in either direction. This is mentioned only as an assumption, not as a risk.
  - **Impact:** For CalDAV-centric users, priority granularity 1-9 is permanently collapsed to 3 levels after caldawarrior touches the record. This is arguably correct iCal behavior, but it's data-lossy and could be surprising.
  - **Fix:** Move this into the Risks table (likelihood: medium, impact: medium) with mitigation: "Document in README that caldawarrior normalizes iCal priority to three canonical values (1, 5, 9); CalDAV clients using intermediate values will see normalization on first TW sync."

- **[Architecture]** Scope of VTODO struct literal breakage is unquantified
  - **Description:** Adding `priority: Option<u8>` to the `VTODO` struct (even with serde defaults) will break every `VTODO { field: value, ... }` struct literal in the codebase at compile time. This is flagged under Phase 1's "Manual checks" but is not a tracked task and has no effort estimate. If there are 10–20 VTODO literals in existing tests (common in iCal-heavy codebases), this could be 30–60 minutes of mechanical work that derails Phase 1.
  - **Impact:** Phase 1 may take significantly longer than its `low` complexity tag implies if VTODO struct literals are widespread. The same issue applies to any other struct with new required fields.
  - **Fix:** Add an explicit subtask to Phase 1: "Audit and update all VTODO struct literal sites." Use `grep -n "VTODO {" src/` upfront to get a count; add `priority: None` to each site. Alternatively, use `..Default::default()` where struct update syntax is applicable.

---

## Minor Suggestions
Smaller refinements.

- **[Clarity]** Phase 3 "compilation stub" task name is ambiguous
  - **Description:** The task is named "Add compilation stub in writeback.rs after Phase 3" but lives *within* Phase 3. The phrase "after Phase 3" appears to mean "immediately after the struct rename, within this phase" — but could be misread as a separate post-Phase-3 step.
  - **Fix:** Rename to "Update writeback.rs field references after struct rename (compilation fix)" and move it explicitly to be the *last* task in Phase 3 with a note "must be last task in this phase."

- **[Completeness]** Success criteria lack a TW→CalDAV direction round-trip for priority
  - **Description:** The success criteria include "Round-trip: TW → CalDAV → TW preserves description, annotations, priority" but there is no RF scenario that explicitly tests the TW→CalDAV direction for priority (S-67 tests CalDAV→TW only). The unit tests in Phase 3/5 cover it, but blackbox coverage is asymmetric.
  - **Fix:** Either add a note that priority TW→CalDAV direction is covered by unit tests only (and is acceptable), or extend S-67 to include an assertion that a TW task with `priority=H` produces `PRIORITY:1` in the VTODO after sync.

- **[Clarity]** `priority: Option<String>` in `CalDavTwFields` vs `priority: Option<u8>` in `TwCalDavFields` — type asymmetry could confuse
  - **Description:** The two mapper structs use different types for their `priority` field: `CalDavTwFields` uses `Option<String>` (TW format: "H"/"M"/"L") and `TwCalDavFields` uses `Option<u8>` (iCal format: 1/5/9). This is correct, but the parallel naming in both structs masks the type difference.
  - **Fix:** Add a brief inline comment to each field: `priority: Option<String>, // TW priority string ("H"/"M"/"L")` and `priority: Option<u8>, // iCal priority value (1/5/9)`. This adds no code complexity but prevents confusion during implementation.

- **[Completeness]** "Known limitation" for annotation deletion is in Constraints but missing from Risks
  - **Description:** The Constraints section mentions: "Deleting DESCRIPTION in a CalDAV client does not remove the TW annotation (the `annotations_text = None` → no-op policy). Users must delete annotations from the TW side." This is a UX limitation that could generate user confusion/bug reports and should be in the risk table.
  - **Fix:** Add a row: `| CalDAV DESCRIPTION deletion not mirrored to TW | high | low | Document as known limitation in README; no mitigation (by design) |`

- **[Architecture]** Project injection for paired entries with no existing TW project is explicitly excluded but worth flagging
  - **Description:** The project assignment in Phase 5 is `project: base.map_or_else(|| entry.project.clone(), |t| t.project.clone())`. This means a TW task that was created manually with no project (`project: None`) and later gets paired with a CalDAV calendar that has `project="work"` configured will *never* inherit the project — because `t.project.clone()` returns `None`. The assumption documents this ("only CalDAV-only new tasks get project from config") but the user-visible consequence is subtle.
  - **Fix:** No code change needed, but add to the Constraints or Known Limitations: "Existing paired TW tasks do not retroactively inherit project from CalDAV calendar config; only newly-created CalDAV-only tasks receive project injection."

---

## Questions
Clarifications needed before proceeding.

- **[Architecture]** Has `build_ir`'s existing signature been confirmed to include `config: &Config`?
  - **Context:** Phase 4 relies on the assumption that "`build_ir` already receives `config: &Config` — no signature change needed for Phase 4." If this assumption is incorrect, Phase 4 requires a larger refactor (modifying callers of `build_ir` throughout the codebase) and the phase complexity should be elevated.
  - **Needed:** Confirm via `grep -n "fn build_ir" src/ir.rs` and checking the call sites before committing to the plan. If the signature doesn't currently include `config`, add a dedicated task for threading it in.

- **[Architecture]** What is the TW JSON date format for annotation `entry` fields, and does `tw_date` serde handle it?
  - **Context:** The plan adds a roundtrip test deserializing `"entry":"20260309T120000Z"`. But TW may use a different timestamp format in practice (some TW versions use `"20260309T120000Z"`, others use epoch seconds or ISO 8601 variants). The `tw_date` serde module is referenced without definition. If it doesn't handle the annotation timestamp format correctly, the deserializer will silently fail or panic.
  - **Needed:** Confirm: (1) what format does `task export` actually produce for annotation `entry` fields, and (2) does the existing `tw_date` serde module handle that format. If a separate serde format is needed for annotations, document it.

- **[Architecture]** How is `resolve_project_from_url` called when a CalDAV task's `calendar_url` doesn't match any configured calendar?
  - **Context:** Phase 4 defines `resolve_project_from_url` to return `None` when no calendar matches. But what is the `calendar_url` of a CalDAV-only task in `build_ir`? Is it always a fully-normalized URL from the CalDAV response, or could there be trailing-slash mismatches beyond what `trim_end_matches('/')` handles (e.g., URL encoding differences, auth-stripped vs auth-included URLs)?
  - **Needed:** Clarify how `calendar_url` is populated on `IREntry` and whether URL normalization via `trim_end_matches('/')` alone is sufficient, or if more robust URL comparison is needed.

---

## Praise
What the plan does well.

- **[Architecture]** Annotation Slot Invariant is exemplary specification
  - **Why:** Specifying all 6 branch cases (3 base-annotation counts × 2 DESCRIPTION presence states) with explicit behavior for each eliminates ambiguity entirely. The "no-op on None" policy with explicit rationale ("preserving TW annotations avoids data loss") gives the implementer clear guidance without needing to re-derive the design.

- **[Clarity]** "was-risk" pattern in risk table is excellent
  - **Why:** Explicitly tracking resolved risks with `was-risk / n/a` entries communicates that potential blockers were considered and deliberately addressed, not overlooked. This is valuable for reviewers and future maintainers who need to understand *why* certain designs were chosen.

- **[Sequencing]** Compilation stub task in Phase 3 shows practical engineering awareness
  - **Why:** The explicit "keep `cargo test` green between phases" task prevents a common pitfall where a refactor leaves the codebase in an uncompilable state mid-phase. This is especially important for a multi-phase plan where each phase needs independent verification.

- **[Completeness]** S-68 skip mechanism specified with exact syntax and project-consistent pattern
  - **Why:** Using `Skip If    '%{MULTI_CALENDAR_ENABLED:=false}' != 'true'` with the project's documented RF OS-env syntax convention (`%{VAR}`) eliminates any ambiguity about how to implement the skip condition. The success criterion also explicitly allows this skip.

- **[Architecture]** LWW Merge Policy section explicitly documents policy exceptions
  - **Why:** Documenting the two deliberate exceptions to the LWW mirror policy (annotations None as no-op; user-only slots 1+) as named deviations prevents future implementers from "fixing" them as apparent bugs. The rationale for each exception is included, making the design self-documenting.

- **[Completeness]** The assumption section is thorough and pre-resolves structural questions
  - **Why:** Assumptions like "`TWTask.annotations` is typed as `Vec<TwAnnotation>` (not `Option<Vec<>>`) with `#[serde(default, skip_serializing_if = "Vec::is_empty")]`" eliminate the dual-None anti-pattern before implementation. This level of pre-specification prevents a whole class of common bugs and review cycles.