# Review Summary

## Critical Blockers

- **[Architecture]** LWW sync-epoch gate description contradicts acceptance criteria
  - **Description:** The `lww.rs` task description says "only trigger LWW write if the modified timestamp is strictly newer than LAST-SYNC" and the Layer 1 explanation says "on next sync TW.modified == LAST-SYNC → no LWW evaluation." Both phrases imply the *entire* LWW function is skipped when TW.modified ≤ LAST-SYNC. However, the acceptance criteria immediately clarify "(TW-wins path only)." A developer reading the description before the AC would implement a version where CalDAV-originated changes from third-party clients are silently dropped whenever TW.modified equals LAST-SYNC.
  - **Impact:** If implemented per the description (not the AC), changes made by any other CalDAV client to VTODOs would be silently ignored any time the corresponding TW task has not been modified since last sync — breaking the CalDAV→TW sync direction entirely.
  - **Fix:** Rewrite the LWW description explicitly: "The LAST-SYNC gate only suppresses the TW-wins path. Specifically: if TW.modified ≤ LAST-SYNC, `resolve_lww` skips the TW-wins check and proceeds to compare CalDAV.LAST-MODIFIED against TW.modified to determine whether CalDAV wins. The gate does NOT skip the entire function." Update the Layer 1/Layer 2 bullets to use this language and remove the phrase "no LWW evaluation."

- **[Architecture]** ETag retry ownership is contradictory within `run_sync`
  - **Description:** `run_sync()` description says "On `CaldaWarriorError::EtagConflict`... update the IR entry's `caldav_data` with the refetched VTODO, re-run `resolve_lww()`, and retry the write — max 3 attempts before returning a `SyncConflict` error for that entry." But `run_sync()` acceptance criteria immediately states "ETag retry logic is owned exclusively by `apply_writeback()` (not duplicated here)." These cannot both be correct: `apply_writeback()` also describes owning the retry loop.
  - **Impact:** A developer who reads the `run_sync` description literally will implement retry logic there as well, resulting in double retries (effectively 9 attempts), inconsistent `SyncResult.errors` population, or divergent behavior depending on which paragraph is followed.
  - **Fix:** Remove the ETag retry paragraph from `run_sync()`'s description entirely. Replace with: "`run_sync()` delegates all ETag conflict handling to `apply_writeback()`. When an entry exhausts its 3 retry attempts, `apply_writeback()` records a `SyncConflict` in `SyncResult.errors` for that entry and continues. `run_sync()` propagates `SyncResult` without modification."

- **[Architecture]** `build_ir()` has no acceptance criterion for the "no `default` calendar configured" path
  - **Description:** The Constraints section states: "If no `default` calendar is configured, the task is skipped with this warning." However, `build_ir()` acceptance criteria say unmapped projects are "assigned to `default` calendar URL" — but there is no criterion handling the case where `default` itself is absent from config. An `IREntry` would be constructed with an empty or `None` `calendar_url`, and the writeback layer would issue a PUT to a malformed URL or panic on `unwrap()`.
  - **Impact:** Runtime panic or silent data corruption when a user configures project-specific calendars but omits a `default` calendar entry — a plausible and common configuration.
  - **Fix:** Add to `build_ir()` acceptance criteria: "TW tasks whose project maps to no config entry AND no `default` calendar URL is configured are excluded from IR construction with a `SkipReason` (or early `UnmappedProject` warning) and do not produce an `IREntry`." Add a corresponding unit test: "TW task with unmapped project, no `default` calendar → excluded from IR with warning."

---

## Major Suggestions

- **[Architecture]** Loop prevention architecture over-relies on content-identical check without acknowledging it explicitly
  - **Description:** The LAST-SYNC gate (Layer 1, TW-wins path) works correctly only if `task import` does NOT mutate `modified`. The Risk table rates this as HIGH likelihood. If Phase 0 confirms that `task import` bumps `modified` to `T2 > LAST-SYNC = T1`, then `T2 > T1` — the gate does not suppress the phantom TW victory. In that scenario, the content-identical check (Layer 2) becomes the *sole* loop-prevention mechanism for *both* directions. The spec acknowledges this as "a hard dependency for loop prevention on the CalDAV-wins path" but does not articulate the same dependency for the TW-wins path fallback.
  - **Impact:** If content-identical normalization has any subtle bug — e.g., TZID handling, trailing whitespace in DESCRIPTION, RELATED-TO sort order — and `task import` mutates modified, the result is a perpetual re-write loop. This is the tool's highest-impact operational failure mode.
  - **Fix:** In Phase 0 AC, add a go/no-go gate: "If item (1) confirms `task import` mutates `modified`, verify loop-prevention stability for BOTH paths: (a) TW wins → sync again → assert `written_caldav == 0`; (b) CalDAV wins → sync again → assert `written_tw == 0`." In `lww.rs` description, add: "Note: when `task import` mutates `modified`, the LAST-SYNC gate is insufficient for the TW-wins path fallback. Loop prevention in that path also depends on the content-identical check in the writeback decision tree."

- **[Risk]** CalDAV REPORT query may not return COMPLETED/CANCELLED VTODOs on all servers
  - **Description:** The plan uses `REPORT` with `comp-filter` to list all VTODOs. Some CalDAV server implementations return only `NEEDS-ACTION` items from a basic VCALENDAR `comp-filter` query, treating other statuses as filtered out by default. If completed or cancelled VTODOs are not returned, the IR builder treats them as absent — causing completed TW tasks within the cutoff window to be re-created as `COMPLETED` VTODOs on every sync run.
  - **Impact:** Repeated creation of already-completed VTODO duplicates in CalDAV for any server that omits completed items from REPORT responses.
  - **Fix:** Add to the Risk table: "CalDAV server returns only NEEDS-ACTION from REPORT query | medium | high | Use a comp-filter that does not filter by STATUS, or issue a PROPFIND on the calendar collection instead. Validate in Phase 0 Radicale test: PUT a COMPLETED VTODO, then REPORT — verify it is returned." Also add to Phase 5 deletion/CANCELLED integration test: verify that REPORT returns CANCELLED VTODOs.

- **[Architecture]** `apply_writeback` mutable IR iteration pattern is unspecified but has Rust borrowing implications
  - **Description:** `apply_writeback()` takes `ir: &mut Vec<IREntry>` and must update `ir[i].caldav_data` during ETag conflict retries while iterating. In Rust, this requires index-based iteration (`for i in 0..ir.len()`) rather than iterator-based (`for entry in ir.iter_mut()`), since updating a field on one entry while holding a mutable reference through the iterator violates borrow rules. The spec is silent on this.
  - **Impact:** Implementers will hit a compiler error and may reach for `RefCell`, cloning, or unsafe code rather than the simple index-based solution.
  - **Fix:** Add a note in `apply_writeback` description: "Iterate using `for i in 0..ir.len()` (index-based) rather than `for entry in ir.iter_mut()` to allow updating `ir[i].caldav_data` during ETag retry without borrow conflicts."

- **[Risk]** No handling or documentation for CalDAV calendar URL non-existence
  - **Description:** The plan assumes all configured calendar URLs already exist on the CalDAV server. If a URL points to a non-existent collection, `put_vtodo()` will receive HTTP 404 or 405. This is not mentioned in error handling, prerequisites, or known limitations.
  - **Impact:** Users get a `CaldaWarriorError::CalDav { status: 404 }` with no actionable guidance. Since calendars must be pre-created, this is a near-certain friction point for first-time users.
  - **Fix:** In the CalDAV adapter task, add: "On HTTP 404 for a PUT or REPORT, return `Err(CaldaWarriorError::CalDav { status: 404, body: 'Calendar not found at {url}: verify the calendar exists on the server and the URL in config is correct' })`. Add to README quick-start prerequisites: 'CalDAV calendars must be pre-created on the server; caldawarrior does not auto-create calendars.'"

- **[Sequencing]** Phase 0 empirical research task is labeled `small` but is clearly `medium` or `large`
  - **Description:** The empirical research task covers 9 distinct behavioral verification items, each requiring a write-test-verify cycle, plus two ADR documents (`tw-import-timestamp.md`, `tw-field-clearing.md`) with structured content. Item #9 alone covers 5 field-clearing cases plus 3 status transition methods. Realistically this is 3–5 days of work.
  - **Impact:** Underestimation leads to phase slippage. Phase 3 LWW design, `task modify` vs `task import` strategy, and `update()` implementation are all gated on these findings.
  - **Fix:** Change complexity to `medium`. Split into two tasks: (a) "TW import/timestamp/UUID behavior research (items 1–4)" and (b) "TW field-clearing, filter syntax, and status transition research (items 5–9)." This clarifies scope and allows concurrent progress once Radicale is running.

- **[Architecture]** No transient-error retry or backoff in CalDAV adapter
  - **Description:** `RealCalDavClient` handles HTTP 412 with an orchestrator-level retry but has no retry for transient network errors: HTTP 503, 429 (rate limiting), connection timeout, or DNS failure. A single network hiccup during `list_vtodos()` fails the entire sync with no recovery path.
  - **Impact:** Unreliable syncs on flaky networks or against rate-limiting servers. Given that `list_vtodos()` is called once per calendar and is the first I/O operation in a sync run, this is a common failure point.
  - **Fix:** Add to the CalDAV adapter task: "Transient errors (HTTP 503, 429, `reqwest` timeout/connect errors) are retried with exponential backoff: 3 attempts at 1s, 2s, 4s intervals. Non-transient errors (4xx except 412, parse errors) are not retried. Add `caldav_max_retries` config key (default: 3)." If deferred to v2, document as Known Limitation #15.

- **[Completeness]** TW `tags` and `priority` fields are not mentioned anywhere in the plan
  - **Description:** TaskWarrior has a `tags` field (arbitrary strings) and `priority` (H/M/L). CalDAV VTODOs have `CATEGORIES` and `PRIORITY` (0–9). The plan does not mention these in the field mapper, known limitations, or v2 roadmap. Additionally, `tags` and `priority` are absent from the content-identical check — meaning CalDAV-wins LWW overwrites TW priority/tags silently.
  - **Impact:** Users who rely on TW priority or tags will lose them silently on any CalDAV-wins round-trip. No warning is emitted. This is a high-visibility gap given how commonly TW priority is used.
  - **Fix:** Add to v1 Known Limitations: "TW `tags` and `priority` fields are not synced in v1. Values on the TW side are not propagated to CalDAV. On CalDAV-wins updates, TW tags and priority are preserved (not overwritten), since they are not part of the content-identical check or `caldav_to_tw_fields()` mapping." Add tags/priority → CATEGORIES/PRIORITY mapping to `docs/v2-roadmap.md`.

---

## Minor Suggestions

- **[Clarity]** "7 fields" in Phase 0 acceptance criteria vs. 8 fields in actual definition
  - **Description:** Phase 0 empirical research AC says "verify content-identical check (all 7 fields) produces stable results." The actual definition lists 8: SUMMARY, STATUS, DUE, DTSTART, COMPLETED, DESCRIPTION, RELATED-TO[DEPENDS-ON], X-TASKWARRIOR-WAIT. The off-by-one may cause Phase 0 validation to omit X-TASKWARRIOR-WAIT normalization verification.
  - **Fix:** Change "all 7 fields" to "all 8 fields." Explicitly include a waiting-status task (one with X-TASKWARRIOR-WAIT) in the stability test.

- **[Architecture]** RFC 5545 line fold-continuation should handle TAB as well as SPACE
  - **Description:** RFC 5545 §3.1 specifies CRLF + SPACE for folding, but some CalDAV server implementations use CRLF + TAB as a fold-continuation indicator. The iCalendar parser AC only mentions SPACE.
  - **Fix:** Add to `src/ical.rs` acceptance criteria: "fold-continuation lines starting with either SPACE (U+0020) or HTAB (U+0009) are recombined on parse." Add a unit test for TAB-folded input.

- **[Clarity]** `SkipReason::Completed` name does not convey the "CalDAV-only" qualifier
  - **Description:** `SkipReason::Completed` is documented as "CalDAV-only COMPLETED item; not imported as new TW task." Without the qualifier in the name, a developer might incorrectly apply this variant to the both-exist completed branch.
  - **Fix:** Rename to `SkipReason::CalDavOnlyCompleted` to match the naming pattern of `CalDavDeletedTwTerminal`. Update all references.

- **[Clarity]** `SyncResult.skipped` uses "etc." instead of complete enumeration
  - **Description:** `SyncResult.skipped` is documented as "number of entries skipped (identical, cancelled, recurring, cyclic, etc.)." The "etc." is imprecise for a doc comment on a public struct field.
  - **Fix:** Replace with the complete list: "count of entries resulting in `PlannedOp::Skip` across all `SkipReason` variants: `Cancelled`, `CalDavOnlyCompleted`, `Recurring`, `Cyclic`, `Identical`, `DeletedBeforeSync`, `AlreadyDeleted`, `CalDavDeletedTwTerminal`."

- **[Architecture]** UTF-8 multi-byte characters at the 75-octet line-folding boundary
  - **Description:** RFC 5545 §3.1 specifies folding at 75 **octets**, not characters. A 4-byte emoji at byte position 73 must not be split across a fold boundary, or the resulting file is invalid UTF-8.
  - **Fix:** Add to `src/ical.rs` acceptance criteria: "Line folding inserts fold points only at valid UTF-8 codepoint boundaries at or before the 75-octet limit. Add a unit test for a SUMMARY containing a 4-byte emoji that pushes past 75 octets."

- **[Completeness]** `--version` flag not specified in CLI
  - **Description:** The CLI spec defines `sync`, `--dry-run`, and `--config` but omits `--version`. This is standard for Rust/clap binaries.
  - **Fix:** Add to `src/cli.rs` task: "Configure `clap` with `version(env!(\"CARGO_PKG_VERSION\"))`. Acceptance criterion: `caldawarrior --version` prints the version string."

- **[Clarity]** `build_ir()` project reverse-mapping for CalDAV-only entries is specified in the wrong task
  - **Description:** The reverse-mapping of `IREntry.calendar_url` → TW `project` for CalDAV-only creates is described in `apply_writeback` acceptance criteria but the logic of building `calendar_url` on those entries comes from `FetchedVTODO.calendar_url` in `build_ir`. The reverse lookup belongs in one clearly designated place.
  - **Fix:** Add to `apply_writeback` acceptance criteria: "For new TW tasks (CalDAV-only NEEDS-ACTION), `project` is reverse-mapped from `IREntry.calendar_url` against config `[[calendar]]` entries. If the URL matches `default`, `project = None`. If no config entry matches, emit `UnmappedProject` and set `project = None`."

---

## Questions

- **[Architecture]** Should CalDAV calendar existence be validated at startup?
  - **Context:** Users must pre-create CalDAV calendars before running sync. There is no startup check — a misconfigured URL will only surface as a `CalDav { status: 404 }` error after the first PUT attempt, potentially after several successful reads from other calendars.
  - **Needed:** Should `run_sync` (or `--dry-run`) issue a lightweight `PROPFIND` on each configured calendar URL during setup to validate reachability and return a clear error before any writes begin?

- **[Architecture]** What is the intended behavior when two TW tasks share the same `caldavuid` value?
  - **Context:** This could occur via manual UDA editing, a backup/restore operation, or a TaskWarrior import from a backup. Both TW tasks would match the same VTODO in the IR. The IR builder has no acceptance criterion for this case.
  - **Needed:** Should `build_ir()` detect duplicate `caldavuid` values, retain the higher-`modified` entry, and emit a `Warning::DuplicateCaldavuid`? Or is this documented as undefined behavior requiring manual resolution?

- **[Architecture]** Is per-calendar `list_vtodos` failure intended to be fatal to the entire sync run?
  - **Context:** `run_sync` fails immediately if any `list_vtodos` call fails. With 5+ calendars, one unreachable server blocks all other calendars' syncs.
  - **Needed:** Is there a preference for "fail fast" (current design) vs. "continue with available calendars and emit a warning for the failed one"? The answer affects whether `Warning::CalendarFetchError` should be added to the `Warning` enum.

- **[Risk]** What are the specific manual recovery steps for Known Limitation #11 (dangling VTODO after crash)?
  - **Context:** Known Limitation #11 says "requires manual cleanup" but the README and constraints don't describe what cleanup looks like. A user who hits this will have a duplicate VTODO in CalDAV with the same SUMMARY as an existing TW task that still has `caldavuid = None`.
  - **Needed:** Document the recovery steps explicitly: (a) identify the duplicate by running `caldawarrior sync --dry-run` and looking for an unexpected CREATE CalDAV for a task that seems to already exist, (b) either delete the dangling VTODO manually or set `caldavuid` on the TW task to the VTODO's UID. Add this to the README troubleshooting section.

---

## Praise

- **[Architecture]** No-sync-database design using `caldavuid` as the sole join key
  - **Why:** Eliminating an intermediate sync database removes a common failure class (database corruption, lock conflicts, schema migration, state divergence from the source systems). The state is inspectable directly: `task export | jq .[].caldavuid` reveals the full sync linkage. Storing the join key in TaskWarrior (where it survives machine restores alongside task data) rather than in a separate file is particularly sound.

- **[Architecture]** Two-invocation `TwAdapter.list_all()` design prevents active-task exclusion
  - **Why:** Using one date-unrestricted export for all active tasks and a separate windowed export for completed/deleted tasks elegantly eliminates the "active task older than cutoff window disappears from IR → duplicate VTODO created" bug. This is a subtle failure mode that many sync tools exhibit for long-lived stable tasks. The deduplication-by-higher-`modified` merge strategy for the edge case of a task transitioning state mid-run is also explicitly specified.

- **[Architecture]** `StatusDecision` enum cleanly separates "skip create" from "delete existing" semantics for CANCELLED
  - **Why:** The distinction between `StatusDecision::SkipCreate` (CalDAV-only CANCELLED → do not import) and `StatusDecision::DeleteExisting` (both-exist CalDAV CANCELLED → delete TW task) is subtle but critical for correctness. Encoding this distinction in the type system forces every caller to explicitly handle both cases, preventing the common error of treating all CANCELLED VTODOs uniformly.

- **[Risk]** Phase 0 empirical research before locking Phase 3 design
  - **Why:** Gating Phase 3's LWW strategy on empirical verification of `task import` timestamp behavior is the right call. The 9-item research scope, combined with the Risk table entry explicitly marking this as HIGH/CRITICAL, ensures the implementation is grounded in verified CLI behavior rather than assumption. The ADR-format documentation requirement provides a clear audit trail when implementation decisions are questioned later.

- **[Completeness]** Constraints section codifies 20+ edge-case behavioral decisions explicitly
  - **Why:** Decisions like DTSTAMP write-vs-read semantics, `task delete` vs purge semantics, TEXT escaping requirements, `completed_cutoff_days` scope, and CalDAV-only CANCELLED import rules are the type of ambiguities that cause expensive mid-implementation rework when left unspecified. Having them in the constraints section — and referenced in task acceptance criteria — reduces the gap between intent and implementation significantly.

- **[Architecture]** ETag-based optimistic concurrency control with orchestrator-level retry
  - **Why:** The adapter correctly returns `EtagConflict { refetched_vtodo }` with fresh data rather than retrying internally. This keeps the adapter stateless and delegates the "what to do with the conflict" decision to the orchestrator, which re-runs LWW with current state. The 3-attempt cap with per-entry error collection (not sync abort) is exactly the right balance between correctness and resilience.