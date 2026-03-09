# Review Summary

## Critical Blockers
Issues that MUST be fixed before this becomes a spec.

*None identified.* The plan is fundamentally sound with well-reasoned root-cause analysis and appropriate fixes for each failure.

---

## Major Suggestions
Significant improvements to strengthen the plan.

- **[Architecture]** `build_ir` access to `config` is implicit but not confirmed
  - **Description:** The Phase 2 task says to add `now: DateTime<Utc>` to `build_ir` and compute `cutoff_dt = now - Duration::days(config.completed_cutoff_days as i64)`. This implies `build_ir` already receives a `config` parameter (or has it in scope). The plan never states this explicitly — it only describes adding `now`. If `config` is not currently in scope inside `build_ir`, the fix requires adding two parameters, not one.
  - **Impact:** If the implementer starts coding and discovers `config` isn't available in `build_ir`, the task description is incomplete and needs re-scoping mid-task.
  - **Fix:** Add one sentence to the task description: "Confirm whether `build_ir` already receives a `Config` argument; if not, add `config: &Config` as a second new parameter and update all call sites accordingly." Add an acceptance criterion: "`build_ir` has access to `config.completed_cutoff_days` (existing or newly added parameter)."

- **[Risk]** Timestamp collision risk is misrated — flaky tests have medium CI impact, not low
  - **Description:** The risk table rates "S-11/S-62 still fail if TW.modified is set at the same second as LAST-MODIFIED" as low impact. But a test that passes 99% of the time and fails 1% is a flaky test, which is a medium-impact CI concern — it erodes trust in the suite and leads to spurious reruns.
  - **Impact:** If this race condition occurs in CI even occasionally, it will cause confusion and potentially mask real regressions. The plan would pass review and ship with a latent flakiness risk.
  - **Fix:** Upgrade impact to **medium**. Add an optional mitigation: "If flakiness is observed in practice, add a `time.sleep(1)` inside `modify_vtodo_summary` after the PUT (or before returning) to guarantee at least one second separates TW.modified from LAST-MODIFIED." This mitigation costs ~1 second per relevant test call and eliminates the race entirely.

- **[Clarity/Architecture]** GET-after-PUT verification placement and semantics are ambiguous
  - **Description:** The `modify_vtodo_summary` acceptance criteria says "After the PUT, a GET on the same resource confirms `LAST-MODIFIED` is present." It's not clear whether this GET happens (a) inside the Python method itself (permanent overhead on every call), or (b) as a separate RF keyword assertion added to S-11/S-62 only. Additionally, the description says this "verifies the Radicale assumption that PUT-supplied LAST-MODIFIED is preserved," but checking *presence* only confirms LAST-MODIFIED exists — it does not verify the *value* matches what was set. If Radicale overrides with server time, presence would pass but the value would differ from T2.
  - **Impact:** (a) If the GET is inside the method, every future call to `modify_vtodo_summary` permanently pays an extra round-trip — coupling a verification concern into a helper method. (b) The stated verification goal (value preserved) is not achievable by checking presence alone; this mismatch could give false confidence.
  - **Fix:** Clarify placement: "Add a postcondition GET inside the method body; this is a deliberate one-time empirical check — once the Radicale assumption is confirmed to hold in practice, the GET can be removed." Update the acceptance criterion to be precise: "Returned VTODO contains LAST-MODIFIED with a value ≥ T2 (the datetime set before PUT)" — this correctly handles both the preserve-exact-value and override-with-server-time cases, and it's the property that actually matters for LWW correctness.

---

## Minor Suggestions
Smaller refinements.

- **[Completeness]** S-41 fix task lacks specific location in the file
  - **Description:** The task says "on the failing line in `05_dependencies.robot`" but doesn't identify which test case or keyword call contains `CalDAV.Add VTODO Related-To`. If the suite file is long and multiple callers exist, the implementer must search.
  - **Fix:** Add: "Locate via RF error output — the failing keyword will be named in the `No keyword found` error message. Expected: one occurrence in the `S-41` test case body."

- **[Clarity]** Phase 3 verification command omits `cargo build --release` prerequisite
  - **Description:** Phase 3 runs `docker compose build && docker compose run --rm robot`. If the Dockerfile copies the compiled binary from the host (rather than compiling inside Docker), `cargo build --release` must be run before `docker compose build`, or the image will bundle a stale binary. The Dockerfile's build strategy is not mentioned.
  - **Fix:** Add a parenthetical in the Phase 3 verification step: "(If Dockerfile copies a pre-compiled host binary, run `cargo build --release` before `docker compose build`.)" Alternatively, add this as an explicit first step in the verification command.

- **[Architecture]** Deleted tasks may not have `end` set — silent no-op for that status
  - **Description:** The cutoff filter targets `status == "completed" || status == "deleted"` and uses `task.end.map(|e| e < cutoff_dt).unwrap_or(false)`. In TaskWarrior, deleted tasks do set `end` (the deletion timestamp), but it's worth confirming: if any deleted task lacks `end`, it will silently pass through the filter regardless of age. For S-33 specifically this is irrelevant (S-33 appears to test completed tasks), but the acceptance criteria don't call this out.
  - **Fix:** Add acceptance criterion: "Deleted tasks without an `end` field are treated as within cutoff (included) — this is consistent with the `unwrap_or(false)` behavior and avoids silent loss of unsynced deleted tasks."

- **[Risk]** `clear_vtodos` DELETE loop does not handle 404 on individual DELETE calls
  - **Description:** The teardown method handles a 404 on PROPFIND (empty collection), but a 404 on an individual DELETE call (resource already removed between PROPFIND and DELETE, unlikely but possible) is not mentioned. The current `_check_response` pattern likely raises on 404, which would abort teardown mid-loop.
  - **Fix:** Add one sentence to the task description: "Individual DELETE calls should treat a 404 response as success (already deleted) — do not raise an error."

---

## Questions
Clarifications needed before proceeding.

- **[Architecture]** Does TaskWarrior reliably set `end` on deleted tasks?
  - **Context:** The cutoff filter uses `task.end` for both `completed` and `deleted` statuses. If TW omits `end` for deleted tasks in some edge cases (e.g., tasks deleted before ever being started), the filter silently doesn't apply to them.
  - **Needed:** Confirmation from TW JSON export of a deleted task — or a note in the assumptions that "TW always sets `end` when a task is completed or deleted."

- **[Architecture]** Is the volume-mount assumption confirmed from `docker-compose.yml`, or inferred?
  - **Context:** The Phase 1 claim that "no rebuild needed" is load-bearing — it affects how quickly developers can iterate on Phase 1 fixes. If the compose file actually COPYs resources at image build time, Phase 1 would also require a rebuild.
  - **Needed:** A note in Assumptions confirming this was verified from `tests/robot/docker-compose.yml` (e.g., "confirmed: `./tests/robot/resources` is mounted at `/tests/resources`"). This is the foundation for the Phase 1/Phase 2 split.

- **[Clarity]** For S-11/S-62: what specifically does caldawarrior's LWW compare on the CalDAV side — `LAST-MODIFIED` only, or also `DTSTAMP`?
  - **Context:** The plan correctly identifies that `modify_vtodo_summary` must set `LAST-MODIFIED`. But if the LWW implementation falls back to `DTSTAMP` when `LAST-MODIFIED` is absent, the pre-fix behavior is consistent with the assumption. If it uses only `LAST-MODIFIED`, then any VTODO without `LAST-MODIFIED` would always lose to TW (any value > missing). Knowing which field(s) are compared would sharpen the assumption section.
  - **Needed:** A brief note confirming which CalDAV field the native LWW resolver reads (from `src/` code review), so the assumption is grounded in code, not inference.

---

## Praise
What the plan does well.

- **[Architecture]** Precise root-cause analysis with concrete code references
  - **Why:** Each of the 4 root causes is tied to a specific mechanism (e.g., "DTSTAMP is older than TW.modified because caldavuid writeback bumps modified"). This is the difference between fixing the symptom and fixing the cause — the plan unambiguously does the latter.

- **[Risk]** Radicale LAST-MODIFIED assumption is acknowledged and empirically verified
  - **Why:** Rather than asserting server behavior, the plan builds a verification step directly into the acceptance criteria (GET after PUT). This is excellent defensive engineering — if the assumption is wrong, the failure will be caught immediately with a clear error, not silently manifest as an LWW bug.

- **[Architecture]** `caldavuid.is_none()` guard is correctly identified and central
  - **Why:** Filtering already-synced completed tasks would trigger orphan deletion of their CalDAV entries — a subtle and destructive correctness bug. Catching this during planning (not implementation) is exactly the right time to address it, and it's prominently documented in both the description and acceptance criteria.

- **[Sequencing]** Phase 1/Phase 2 split is grounded in operational reality
  - **Why:** Distinguishing "volume-mounted resources (no rebuild)" from "Rust binary change (rebuild required)" is not just theoretical — it directly affects iteration speed during debugging. A developer fixing Phase 1 can run tests in seconds; knowing this upfront avoids unnecessary rebuilds.

- **[Risk]** `completed_cutoff_days = 0` no-op edge case is explicitly handled
  - **Why:** `0` is a natural default/sentinel value that many users might set. Without this explicit treatment, `now - Duration::days(0)` = `now`, which would filter out every completed task modified today — a silent data loss bug. The plan anticipates this and codifies the safe behavior.

- **[Completeness]** Boundary condition test matrix for the cutoff filter is thorough
  - **Why:** Five distinct boundary cases (filtered, same-second, None end, has caldavuid, zero cutoff) cover the meaningful behavioral boundaries. This is the right level of coverage for a correctness-critical filter without being exhaustive.

- **[Clarity]** Assumptions section is unusually honest about what's inferred vs. known
  - **Why:** Flagging "Radicale preserves LAST-MODIFIED — will be verified empirically" rather than stating it as fact gives future implementers the right level of caution. This kind of epistemic hygiene in a spec prevents "but the spec said so" post-mortems.