# Review Summary

## Critical Blockers
Issues that MUST be fixed before this becomes a spec.

- **[Risk]** Unacceptable mitigation for content check gaps
  - **Description:** The plan states: "If Phase 1 investigation reveals widespread content check gaps... Proceed with native LWW treating known gaps as accepted risk." Without `X-CALDAWARRIOR-LAST-SYNC`, the `content_identical()` check (Layer 2) is the *only* mechanism preventing infinite sync loops. If it fails, the sync engine will enter a continuous ping-pong loop or an infinite push loop.
  - **Impact:** An infinite sync loop will thrash the CalDAV server, consume CPU/bandwidth on every run, and constantly mutate `TW.modified` timestamps. This is fatal for a bidirectional sync tool.
  - **Fix:** Update the mitigation strategy in the Risks table and Phase 1 investigation task. It must state: "If content check gaps are found and cannot be fixed within this scope, the spec MUST BE ABORTED/PAUSED until Layer 2 is made 100% deterministic." A known sync loop cannot be treated as an accepted risk.

- **[Architecture]** Missing `DTSTAMP` violates RFC 5545
  - **Description:** The plan's mission is to make VTODOs "100% standard-compliant". However, the assumptions note that `build_vtodo_from_tw()` writes `dtstamp: None`. RFC 5545 (Section 3.6.2) explicitly requires `DTSTAMP` on all `VTODO` components.
  - **Impact:** Omitting a REQUIRED property means the output is not strictly standard-compliant and risks rejection by stricter CalDAV servers or parsing errors in third-party clients.
  - **Fix:** In Phase 2's `build_vtodo_from_tw()` task, add an instruction to set `dtstamp: Some(now)` (since the `now` parameter is already passed into the function) to achieve true standard compliance.

## Major Suggestions
Significant improvements to strengthen the plan.

- **[Sequencing]** Test helper signature changes span multiple phases
  - **Description:** Both Phase 1 and Phase 2 mention updating test helpers (like `make_vtodo`) to drop the `last_sync` parameter. Since these are likely shared helpers used across multiple test modules, changing the signature in Phase 1 will immediately break tests in `writeback.rs` before Phase 2 begins.
  - **Impact:** Compilation failures between phases, blocking iterative development and breaking the build.
  - **Fix:** Specify in Phase 1 that when changing the `make_vtodo` signature, all call sites across the entire `sync` module (including `writeback.rs`) must be updated simultaneously (e.g., passing a hardcoded `None` or refactoring the calls) to ensure the build remains green after Phase 1. Phase 2 can then clean up the remaining test logic.

## Minor Suggestions
Smaller refinements.

- **[Architecture]** Clarify `TW.entry` fallback behavior
  - **Description:** Using `TW.entry` as a fallback when `TW.modified` is missing is technically correct for initial LWW logic, but `entry` is an immutable creation timestamp. 
  - **Fix:** Add a brief note in the assumptions confirming that modern TaskWarrior reliably updates `modified` on every change, meaning this fallback only practically applies to brand new tasks (where `entry` == creation time, which is correct for LWW) or corrupted databases.

## Questions
Clarifications needed before proceeding.

- **[Architecture]** Server overrides of LAST-MODIFIED
  - **Context:** The architecture relies on the CalDAV server accepting `LAST-MODIFIED = TW.modified`. The Phase 1 investigation verifies Radicale's behavior, but what if a stricter server (like Apple Calendar Server or Nextcloud) forcefully overwrites `LAST-MODIFIED` with the server's wall-clock time on PUT?
  - **Needed:** Clarification on the fallback strategy. If the server overrides the timestamp, `TW.modified` will appear older than CalDAV's `LAST-MODIFIED` on the next sync, meaning CalDAV would "win" unless Layer 2 perfectly skips it. Does the plan consider Layer 2 robust enough to handle this scenario indefinitely?

## Praise
What the plan does well.

- **[Clarity]** LWW Decision Tree Documentation
  - **Why:** The explicit mapping of the LWW decision tree (cases a-g) in Phase 1's testing task is exceptionally clear and ensures test-driven completeness for the new logic.

- **[Architecture]** Elimination of Custom State
  - **Why:** Moving away from `X-CALDAWARRIOR-LAST-SYNC` is a strong architectural choice that simplifies the system, shifting from stateful sync tracking to true stateless LWW based on standard properties.