# Review Summary

## Critical Blockers
Issues that MUST be fixed before this becomes a spec.

- **[Clarity]** Equality tie-breaker contradicts itself between Success Criteria and implementation body
  - **Description:** Success Criteria states: *"TW.modified == LAST-MODIFIED resolves as CalDAV wins (no update needed)"*. Every other location in the plan contradicts this: the Phase 1 task decision tree uses `>=` → TW wins; Assumption 5 says *"TW wins (see tie-breaker policy below)"*; unit test (b) explicitly asserts *"TW wins on equal timestamps (not CalDAV wins)"*. These are directly incompatible — the acceptance criterion for the success criteria checkbox will fail against the implementation the tasks describe.
  - **Impact:** An implementer following the task description and unit tests will produce code that fails the success criterion. A reviewer verifying the spec will get contradictory signals. The equality case is the normal steady-state after a TW write (LAST-MODIFIED is set to TW.modified), so this code path is exercised on every sync cycle.
  - **Fix:** Resolve which policy is correct and apply it uniformly. The plan's own reasoning (Assumption 5, task description) favours **TW wins on equality** because: (a) content will also be identical at steady state so Layer 2 fires first anyway; (b) choosing CalDAV wins on equality for the case where content *differs* but timestamps match risks an oscillation — TW imports CalDAV data, TW.modified becomes `now > LAST-MODIFIED`, next sync TW pushes back, potentially looping until Layer 2 stabilises it. Correct the Success Criteria entry to: *"TW.modified == LAST-MODIFIED (with differing content) resolves as TW wins; self-stabilises via Layer 2 on the next sync"*.

---

## Major Suggestions
Significant improvements to strengthen the plan.

- **[Sequencing]** Investigation task does not gate the implementation tasks that depend on its findings
  - **Description:** *"Validate Layer 2 round-trip determinism"* has `Depends on: none` and no tasks depend on it in return. The two Phase 1 implementation tasks (`Remove get_last_sync()` and `Rewrite Layer 1 LWW comparison`) are also `Depends on: none`, meaning an implementer can freely skip or parallelise the investigation and begin rewriting before knowing whether content_identical() has gaps that would create loops under the new LWW regime.
  - **Impact:** If the investigation finds gaps, the spec says to add *"an additional task"* — but the existing tasks are already in progress or complete. The remediation task has no defined home, no estimate, no acceptance criteria, and no mechanism to block phase completion. The investigation's value as a gate is entirely lost.
  - **Fix:** Either: (a) mark `Rewrite Layer 1 LWW comparison` as `Depends on: Validate Layer 2 round-trip determinism` so the investigation must complete (and any gap tasks be added) before the core rewrite is finalised; or (b) explicitly accept the investigation as a post-hoc validation and add a Phase 1 gate task whose sole acceptance criterion is *"investigation complete, zero unhandled gap types OR follow-on spec created and linked"*.

- **[Risk]** Two risk rows are near-duplicates and should be consolidated, and a third missing risk warrants inclusion
  - **Description:** Risk row 1 (*"Layer 2 lossy-mapping loop"*) and Risk row 2 (*"Phase 1 investigation reveals widespread gaps"*) describe the same failure mode (content_identical() gaps causing loops) with the same mitigation (time-box investigation, extract to follow-on). They differ only in framing (cause vs. discovery mechanism) and both list medium/medium. Additionally, there is an unaddressed risk: Phases 1 and 2 touch separate modules that must be deployed together — if `build_vtodo_from_tw()` starts writing LAST-MODIFIED=TW.modified (Phase 2) while the old LWW code still reads LAST-SYNC (which will now be absent), the LWW comparison falls through to `None` and TW always wins until Phase 1 is deployed.
  - **Impact:** Duplicate rows inflate the risk table without adding signal. The partial-deployment gap could cause temporary data consistency issues on a live server if someone applies commits from Phase 2 before Phase 1.
  - **Fix:** Merge rows 1 and 2 into a single row. Add a new row: *"Phases 1 and 2 deployed separately"* | low | medium | *"Phase 2 must not reach production without Phase 1; document in PR description; add compile-time coupling (e.g., single PR) or a runtime guard"*.

- **[Completeness]** Clock-skew deployment note is promised in Risks but no task records it
  - **Description:** The clock-skew risk mitigation says *"Document in deployment notes"*, but no task in any phase has this as a deliverable. There is no deployment notes file mentioned anywhere in the spec.
  - **Impact:** The mitigation will not be implemented — a reader of the merged code or future operator will have no guidance on the NTP dependency.
  - **Fix:** Add a `documentation` `low` task to Phase 3 or Phase 4 titled *"Add deployment note for NTP clock-skew dependency"* with acceptance criterion: *"README or ops docs contain a note that TW host and CalDAV server must have synchronised clocks (NTP); LWW correctness degrades under skew > a few seconds when third-party CalDAV clients are in use"*.

---

## Minor Suggestions
Smaller refinements.

- **[Clarity]** "or confirmed unused" hedge in Phase 1 acceptance criterion creates unnecessary ambiguity
  - **Description:** The acceptance criterion for *"Remove get_last_sync() helper"* reads: *"`LAST_SYNC_PROP` constant is removed (or confirmed unused)"*. This allows the constant to remain in the file as long as it is unused, but Phase 3 verification requires `grep … returns zero results`. The hedge will cause Phase 1 to pass while Phase 3 later fails.
  - **Fix:** Remove the hedge. Write: *"`LAST_SYNC_PROP` constant is deleted from lww.rs"*.

- **[Completeness]** Phase 4 RF test task lacks a pre-scan step that would bound its effort upfront
  - **Description:** The task tells the implementer to *"look for"* RF tests referencing X-CALDAWARRIOR-LAST-SYNC, but provides no grep command for this (unlike Phase 1/3 verification sections). Given the pre-existing 7-test baseline of failures, it would be valuable to know upfront how many RF tests are potentially affected.
  - **Fix:** Add to the task description: *"Before running the suite, run `grep -r 'LAST-SYNC\|LAST_SYNC' tests/robot/` to enumerate affected test files. If zero matches, RF changes are likely not needed."*

- **[Clarity]** Phase 2 verification section's grep overlaps with Phase 3's, making it appear like Phase 2 alone clears all references
  - **Description:** Phase 2 verification ends with: *"`grep -r 'X-CALDAWARRIOR-LAST-SYNC\|LAST_SYNC_PROP' src/` returns zero results after Phase 1+2"*. This is correct only if both phases are done, but the note "(after Phase 1+2)" is easy to overlook; a solo Phase 2 verifier might run this and incorrectly pass it.
  - **Fix:** Change Phase 2 verification to scope it: *"After Phase 2: `grep 'X-CALDAWARRIOR-LAST-SYNC' src/sync/writeback.rs` returns zero results. (Full cross-file check is gated on Phase 3.)"*

- **[Sequencing]** Phase 3 task dependency on `Rewrite Layer 1 LWW comparison` is artificial
  - **Description:** *"Confirm and remove dedicated LAST-SYNC handling in ical.rs"* depends on the LWW rewrite, but the ical.rs cleanup (removing a parse arm or constant) has no logical dependency on the LWW logic being correct. The dependency appears to exist only to serialise phases.
  - **Fix:** Change the dependency to: `Depends on: Remove get_last_sync() helper` — this is the task that actually removes `LAST_SYNC_PROP` in lww.rs, after which the grep in Phase 3 makes sense. Or make it `Depends on: none` with a note that it must be merged in the same PR.

---

## Questions
Clarifications needed before proceeding.

- **[Architecture]** How does LAST-MODIFIED precision interact with the equality tie-breaker in practice?
  - **Context:** The plan notes the equality case handles *"a sub-second TW edit at the same second as the last write"*. RFC 5545 UTC timestamps (`YYYYMMDDThhmmssZ`) have one-second resolution. TW.modified is a Unix timestamp, also typically second-resolution. If both are second-precision, equality is far more common than "sub-second" implies — it will occur on every sync cycle in the steady state (no changes on either side, but Layer 2 fires first). The concern is: are there CalDAV clients or servers that use sub-second LAST-MODIFIED values? If so, `>=` means TW could win even when the CalDAV client edited 0.5 seconds after the TW write.
  - **Needed:** Confirm whether Radicale (and any other supported CalDAV servers) expose sub-second precision in LAST-MODIFIED, and whether the Rust `DateTime<Utc>` comparison truncates to seconds before comparing or uses full nanosecond precision.

- **[Architecture]** What is the intended outcome when both TW.modified and TW.entry are unavailable?
  - **Context:** The spec states *"TWTask.entry is typed as `DateTime<Utc>` (non-Optional — confirmed in types.rs)"*, making `tw.modified.unwrap_or(tw.entry)` always defined. However, the spec does not address the case where a TW task arrives from an import with a malformed or epoch-zero entry. Is `DateTime<Utc>` guaranteed to be a meaningful timestamp, or could it be `0` in edge cases?
  - **Needed:** Confirm whether TW guarantees `entry` is always a valid non-epoch timestamp on import, or whether a floor/guard is needed before the comparison.

- **[Risk]** Are there Robot Framework tests that exercise the equality tie-breaker path specifically?
  - **Context:** The equality case (TW.modified == LAST-MODIFIED) is the steady-state behaviour for a no-change sync cycle. If RF tests were written under the old X-LAST-SYNC regime, they may never have exercised this path. If the tie-breaker behaviour changes between the old and new implementations, this could silently affect test outcomes.
  - **Needed:** A quick grep of `tests/robot/` for tests that set up "no change on either side" scenarios, to confirm they rely on Layer 2 (content check) rather than the LWW tie-breaker.

---

## Praise
What the plan does well.

- **[Completeness]** Assumptions are exceptionally precise and verifiable
  - **Why:** Each assumption cites a specific file and line number (e.g., *"writeback.rs line 101"*, *"types.rs line 211-230"*, *"confirmed in types.rs"*). This eliminates the ambiguity that typically plagues assumption sections and means an implementer can immediately validate each assumption rather than treating them as axiomatic. The distinction between `Option<DateTime<Utc>>` and `DateTime<Utc>` fields is explicitly called out — this is exactly the kind of type-level detail that prevents off-by-one fallback bugs.

- **[Architecture]** Self-stabilisation argument is fully traced through the system
  - **Why:** The plan explicitly traces the steady-state loop: TW writes → LAST-MODIFIED = TW.modified → next sync equality → Layer 2 fires first → Skip. This is not hand-waved; it follows the data through two sync cycles. This level of reasoning is rare in implementation plans and will prevent future maintainers from "fixing" the equality tie-breaker in a way that breaks the loop.

- **[Completeness]** Unit test coverage is thorough and precisely enumerated
  - **Why:** The seven test scenarios (a)-(g) in the Phase 1 task description cover all meaningful branches: TW wins (strict), TW wins (equality), CalDAV wins, two absent-timestamp fallbacks, TW.modified=None fallback to entry, and a regression test for Layer 2 firing before Layer 1. Specifying these upfront means test coverage is a first-class deliverable, not an afterthought.

- **[Risk]** Risk table is honest about accepted operational risk
  - **Why:** The clock-skew row explicitly acknowledges *"Code has no mechanism to enforce this"* rather than inventing a mitigation that doesn't exist. This transparency is more useful than a false mitigation and correctly scopes the risk as a deployment concern rather than a code concern.

- **[Sequencing]** Phase boundary verification commands are concrete and executable
  - **Why:** Each phase ends with exact `cargo test` module filters and `grep` patterns that serve as unambiguous phase completion gates. This is far superior to vague *"verify the implementation"* instructions and makes the verification reproducible across implementers and CI environments.