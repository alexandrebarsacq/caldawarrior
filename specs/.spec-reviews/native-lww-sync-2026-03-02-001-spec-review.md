# Spec Review: native-lww-sync

**Spec ID:** native-lww-sync-2026-03-02-001
**Review Type:** standalone spec review
**Verdict:** unknown
**Template:** PLAN_REVIEW_FULL_V1
**Date:** 2026-03-02T13:40:14.903846
**Provider:** claude

## Review Output

# Review Summary

## Critical Blockers

- **[Completeness]** `task-1-1` has no description, acceptance criteria, or timebox — it is effectively a blank task
  - **Description:** The research task "Validate Layer 2 round-trip determinism" (`task-1-1`) contains empty `metadata` and no description, no acceptance criteria, and no definition of done. The spec body mentions it is "time-boxed to 2 hours" and should surface "gap types", but none of this appears in the task definition that an implementer would actually read.
  - **Impact:** Without an acceptance definition, the implementer has no guidance on what to investigate, what output to produce, or when to stop. This task gates Phase 1 safety and directly informs the risk assessment for the entire spec. An implementer will either skip it or perform an unbounded investigation.
  - **Fix:** Add a `description` covering: (a) the specific TW import normalization paths to check (e.g., description whitespace, tag casing, date truncation), (b) the procedure (round-trip a known TW task through `build_vtodo_from_tw` → parse back → compare), (c) an explicit 2-hour timebox, and (d) acceptance criteria such as "document all gap types found; if ≤2, proceed; if >3, file follow-on spec and treat as accepted risk."

- **[Architecture]** The Radicale LAST-MODIFIED override risk is mis-rated and its failure mode invalidates the entire LWW mechanism
  - **Description:** The risk "Radicale overrides LAST-MODIFIED with its own timestamp on PUT" is rated low likelihood / medium impact, but the failure mode is actually catastrophic for correctness: if Radicale replaces the client-supplied LAST-MODIFIED with a server wall-clock time on every PUT, then after a TW-wins write, the stored `caldav_timestamp` (Radicale's server time, slightly after TW.modified) will be **greater** than `tw_timestamp`, causing CalDAV to win on the very next sync. Layer 2 content check only helps if Radicale does not add or reformat any iCal properties — which cannot be assumed without testing. This creates a write–revert–write loop exactly like the old bug.
  - **Impact:** The core LWW correctness guarantee — "after a TW-wins write, the next sync is a no-op" — collapses silently if Radicale overrides LAST-MODIFIED. This would only manifest against a live Radicale server, making it hard to detect in CI.
  - **Fix:** (a) Upgrade this to the primary Phase 1 research task: empirically confirm Radicale's behavior by performing a PUT with an explicit LAST-MODIFIED and then GET-ing it back. (b) Add an acceptance criterion to `task-1-1`: "Radicale preserves or does not overwrite client-supplied LAST-MODIFIED after PUT." (c) If Radicale does override, the spec must define a fallback strategy (e.g., re-fetch LAST-MODIFIED after every PUT and store it as the effective ts for the next comparison).

---

## Major Suggestions

- **[Data Model]** Using `vtodo.dtstamp` as a fallback for `last_modified` is semantically incorrect per RFC 5545
  - **Description:** The spec falls back to `vtodo.dtstamp` when `vtodo.last_modified` is absent (`caldav_timestamp = vtodo.last_modified.or(vtodo.dtstamp)`). Per RFC 5545 §3.8.7.2, DTSTAMP represents the date-time at which the **iCalendar object instance** was created or sent — it is a scheduling artifact, not a content modification timestamp. For a VTODO created by a third-party CalDAV client that never sets LAST-MODIFIED, DTSTAMP could be years old and entirely unrelated to when the item was last modified. Using it as an LWW tiebreaker would cause TW to win or lose based on meaningless data.
  - **Impact:** Incorrect LWW decisions when LAST-MODIFIED is absent and DTSTAMP is stale, affecting CalDAV clients that omit LAST-MODIFIED (legal per RFC 5545 — the property is OPTIONAL).
  - **Fix:** Either (a) drop the DTSTAMP fallback and treat absent `last_modified` as "TW wins" (simpler, already covered by decision tree item 1), or (b) explicitly document this is an acknowledged approximation and add a unit test scenario labelled "DTSTAMP-as-fallback" so the semantic compromise is visible in the test suite. Option (a) is cleaner since the spec already handles `None` → TW wins.

- **[Data Model]** Fractional-second truncation in LAST-MODIFIED creates a systematic bias not acknowledged by the spec
  - **Description:** TW stores `modified` as `DateTime<Utc>` with sub-second precision. The spec mandates LAST-MODIFIED is written as `YYYYMMDDThhmmssZ` (no fractional seconds). After a TW-wins PUT, the stored `caldav_timestamp` is `tw_timestamp` truncated to seconds. On the next sync, `tw_timestamp` (with milliseconds) is compared against the truncated `caldav_timestamp`. If TW.modified has any sub-second component, `tw_timestamp > caldav_timestamp` is always true — TW always "wins" Layer 1. Layer 2 saves the day by detecting identical content, but this is a load-bearing reliance on Layer 2 that the spec does not make explicit.
  - **Impact:** The spec's claimed Layer 1 stability guarantee ("TW.modified >= LAST-MODIFIED → TW wins, then Layer 2 detects identical content → Skip") only works correctly because of truncation, not by design. If Layer 2 were ever weakened or bypassed, truncation would cause permanent re-writes. The relationship should be intentional, not incidental.
  - **Fix:** Add an explicit note in `task-1-3` and `task-2-1` that `tw_timestamp` should be **truncated to second precision** before comparison, matching the precision of the stored LAST-MODIFIED. Alternatively, add a dedicated test scenario: "TW.modified with sub-second component vs truncated LAST-MODIFIED from previous write → Layer 2 Skip."

- **[Completeness]** The `writeback.rs` LAST_SYNC_PROP constant is mentioned only as an aside and lacks a removal task
  - **Description:** In `task-2-1`, the description notes: "The LAST_SYNC_PROP constant is removed here (writeback.rs has its own local constant)." This implies `writeback.rs` has a separate `LAST_SYNC_PROP` constant from the one in `lww.rs` deleted in `task-1-2`. This is mentioned parenthetically mid-description rather than as an explicit acceptance criterion or task step.
  - **Impact:** If an implementer misses this aside, `writeback.rs` will retain a dead constant, the Phase 3 grep check (`grep -r 'LAST.SYNC|last_sync|LAST_SYNC'`) will fail unexpectedly, and the implementer will need to trace back to find the missed deletion.
  - **Fix:** Add an explicit acceptance criterion to `task-2-1`: "The `LAST_SYNC_PROP` constant in `writeback.rs` is deleted." Alternatively, confirm whether this constant is the same one as in `lww.rs` (cross-file re-export vs. duplication) and clarify which file owns it.

- **[Verification]** No test or investigation task confirms Radicale's actual LAST-MODIFIED round-trip behavior
  - **Description:** Three risks explicitly depend on Radicale behavior (LAST-MODIFIED passthrough, content preservation), and Phase 1's research task is supposed to confirm this — but `task-1-1` has no description (see Critical Blockers). Beyond task-1-1, there is no RF integration test that asserts the LAST-MODIFIED written to Radicale is the same value returned on the subsequent GET.
  - **Impact:** The spec's correctness depends on an empirical assumption about Radicale that is never explicitly tested. If behavior changes across Radicale versions, there is no regression test.
  - **Fix:** Add an acceptance criterion to `task-1-1`: "Perform a PUT to Radicale with LAST-MODIFIED=T, then GET and confirm returned LAST-MODIFIED=T (not server wall-clock)." If the RF test harness supports it, add a lightweight test that writes a VTODO with a specific LAST-MODIFIED and asserts the value round-trips correctly.

---

## Minor Suggestions

- **[Completeness]** `task-4-1` has `file_path: "src/sync/lww.rs"` but the task scope is all tests
  - **Description:** The file_path for "Run unit and integration tests" points to `lww.rs`, but the task covers `cargo test` (all targets) and a grep of `tests/robot/`.
  - **Fix:** Change `file_path` to `null` or list multiple paths, or remove the field — it is misleading for a task that is not file-specific.

- **[Verification]** `verify-3-1` uses `verification_type: "fidelity"` but performs test execution
  - **Description:** verify-3-1 runs `cargo test` and a grep assertion — this is `run-tests`, not a fidelity comparison against the spec.
  - **Fix:** Change `verification_type` to `"run-tests"` for consistency with verify-1-1 and verify-2-1.

- **[Completeness]** No migration note for existing CalDAV VTODOs that already contain X-CALDAWARRIOR-LAST-SYNC
  - **Description:** After deployment, existing CalDAV entries from previous syncs will still carry `X-CALDAWARRIOR-LAST-SYNC` in their VTODO bodies. The spec notes they'll "fall through to generic extra_props passthrough and be silently ignored," but this is only stated in `task-3-1` and only for `ical.rs`. There is no explicit statement about what happens to these properties on the next TW-wins PUT (will the extra_prop be echoed back or dropped?).
  - **Fix:** Add a sentence to Phase 2 or Phase 3 description: "On the first TW-wins PUT after deployment, `build_vtodo_from_tw()` does not copy the old `X-CALDAWARRIOR-LAST-SYNC` from the incoming VTODO's `extra_props`, so the property is dropped naturally from that point forward." Confirm this is actually true in the current `build_vtodo_from_tw()` implementation.

- **[Architecture]** The equality case (TW.modified == LAST-MODIFIED → TW wins) deserves a more robust justification
  - **Description:** The spec states "equality: prefer local edit, self-stabilises via Layer 2 next sync." This is reasonable, but the justification skips over the case where a CalDAV client sets LAST-MODIFIED equal to TW.modified (possible if a CalDAV client imports a VTODO with the same timestamp). In that case, TW would win even if the CalDAV client made a different change.
  - **Fix:** Add a brief note: "TW wins on equality because the primary modification source is assumed to be TW; a CalDAV client that sets LAST-MODIFIED == TW.modified is an accepted edge case since Layer 2 would catch a true content-identical scenario."

- **[Verification]** The 7 pre-existing RF failures are excluded from scope without confirming they are unrelated to LWW
  - **Description:** Phase 4 states pre-existing 7 failures are "out of scope unless directly caused by the LWW change," but there is no task to verify this claim. If any of the 7 failures test LWW-related behavior, the LWW refactoring could accidentally fix or change them, which should be noted and tracked.
  - **Fix:** Add a step to `task-4-2`: "Before running RF tests, grep the 7 failing test names against `LAST-SYNC`, `LAST_MODIFIED`, and `conflict` patterns to confirm they are unrelated to LWW. Document the result in the task completion note."

---

## Questions

- **[Architecture]** How does the system handle a `completed` task's LAST-MODIFIED?
  - **Context:** `task-2-1` explicitly notes "the `now` parameter is still needed for `completed: Some(now)` for completed tasks." But if `last_modified` is set to `tw.modified.unwrap_or(tw.entry)` uniformly, the LAST-MODIFIED of a completed task reflects when it was last modified in TW (which could be before completion), not when it was completed. A CalDAV client that displays "last modified" would show the pre-completion timestamp.
  - **Needed:** Clarify whether LAST-MODIFIED for completed tasks should use `completed_at` or `modified` and whether this distinction matters for LWW correctness in the context of completed tasks being re-opened on CalDAV.

- **[Data Model]** Are there any non-UTC LAST-MODIFIED values in the wild that the parser would produce for `vtodo.last_modified`?
  - **Context:** The spec mandates UTC output (`Z` suffix), but real CalDAV servers or third-party clients may store LAST-MODIFIED as local time with a TZID parameter. If `vtodo.last_modified` is parsed as a naive datetime or silently dropped, the `caldav_timestamp` comparison would be incorrect.
  - **Needed:** Confirm whether the existing `ical.rs` parser normalizes all LAST-MODIFIED variants to UTC `DateTime<Utc>`, returns `None` for non-UTC formats, or panics. Add a note (and optionally a test) to `task-3-1` covering this case.

- **[Interface Design]** Does `resolve_lww()` currently receive both `vtodo.last_modified` and `vtodo.dtstamp` as separate fields, or is only one exposed through the interface?
  - **Context:** `task-1-3` assumes `vtodo.last_modified.or(vtodo.dtstamp)` is accessible in `resolve_lww()`. If the function signature currently takes a single `Option<DateTime<Utc>>` for the CalDAV timestamp, the fallback logic would need to be implemented in the caller or the signature changed.
  - **Needed:** Clarify whether the VTODO struct fields `last_modified` and `dtstamp` are both passed into `resolve_lww()` or if one needs to be threaded through the call chain.

---

## Praise

- **[Architecture]** The two-layer conflict resolution strategy is elegantly designed
  - **Why:** Separating content-based deduplication (Layer 2) from timestamp-based LWW (Layer 1), and explicitly ordering Layer 2 first, is a sound design that avoids false write loops without requiring any custom metadata. The spec clearly articulates why this ordering matters and documents it as the "sole loop-prevention layer," which is a well-considered architectural choice.

- **[Completeness]** Success criteria are concrete, grep-verifiable, and binary
  - **Why:** Each success criterion is either a grep that returns zero results or a test exit code — there is no ambiguity about whether the spec is complete. This makes the final verification mechanical rather than judgmental, reducing reviewer subjectivity.

- **[Verification]** The 7 unit test scenarios for `task-1-4` are comprehensive and well-ordered
  - **Why:** Scenarios (a)–(g) cover all meaningful branches including the equality edge case, the None fallback chain, and the critical regression scenario (g) confirming Layer 2 fires before Layer 1. The explicit assertion that test (g) returns `Skip(Identical)` rather than `ResolveConflict` is precisely the right test to guard against regression.

- **[Architecture]** Risk log is honest and appropriately distinguishes operational from code-level risks
  - **Why:** Explicitly calling out clock skew as an "accepted operational risk" with no code-level mitigation is the right call. Many specs try to over-engineer around clock drift; this one correctly identifies that NTP is the right layer for that concern.

- **[Completeness]** Phase dependencies are correctly encoded and sequential
  - **Why:** The phase dependency chain (1 → 2 → 3 → 4) correctly reflects that LAST-SYNC must be removed from the reader (Phase 1) before it can be safely removed from the writer (Phase 2), and both before the parser cleanup (Phase 3). This prevents any phase from being started in an inconsistent state.

---
*Generated by Foundry MCP Spec Review*