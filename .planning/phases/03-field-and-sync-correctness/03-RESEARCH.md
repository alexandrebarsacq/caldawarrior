# Phase 3: Field and Sync Correctness - Research

**Researched:** 2026-03-19
**Domain:** Bidirectional field mapping E2E verification, status transition testing, deletion propagation, sync idempotency
**Confidence:** HIGH

## Summary

Phase 3 is a verification-and-fix phase: writing comprehensive E2E tests for all mapped fields, fixing the CalDAV CANCELLED-to-TW-deletion asymmetry, and proving sync idempotency. The codebase is mature with 163 unit tests, 6 integration tests, and 18 integration scenario tests. The existing Robot Framework infrastructure (CalDAVLibrary, TaskWarriorLibrary, common.robot) provides all keywords needed for new tests, with two gaps that need addressing: the `modify_vtodo_status` keyword does not bump `LAST-MODIFIED` and does not clear the `COMPLETED` timestamp on reopen.

The main code change is in `src/sync/writeback.rs` lines 249-262, where the `decide_op` function currently SKIPs when CalDAV status is CANCELLED (instead of propagating deletion to TW). This is a targeted fix -- the `PlannedOp::Skip` with `SkipReason::Cancelled` needs to become a `PlannedOp::ResolveConflict` with CalDAV winning when TW is still active. The rest of the phase is E2E test expansion.

**Primary recommendation:** Fix the CANCELLED propagation bug first, then write E2E tests organized by test-first approach for known bugs and comprehensive coverage of every mapped field's create/update/clear lifecycle.

<user_constraints>

## User Constraints (from CONTEXT.md)

### Locked Decisions
- Bidirectional clear: clearing a field on either side propagates to the other side via LWW
- No tags -> no CATEGORIES line in VTODO (remove property entirely, not empty value)
- No annotations -> no DESCRIPTION property in VTODO (remove, not empty)
- X-TASKWARRIOR-WAIT is TW-authoritative only: TW clear -> remove X-prop from CalDAV. CalDAV-side clear of X-TASKWARRIOR-WAIT is not meaningful
- TW delete -> CalDAV STATUS:CANCELLED (verify existing behavior)
- CalDAV orphan (VTODO gone) -> TW task deleted (verify existing behavior, already tested S-20/S-21)
- CalDAV CANCELLED -> TW deleted (FIX: currently skipped, creates asymmetry)
- Add explicit ghost task test: CANCELLED VTODO on CalDAV with no TW pair -> no TW task created
- Both sides deleted/cancelled -> skip (verify existing AlreadyDeleted behavior)
- Test all status transition paths bidirectionally with comprehensive E2E coverage
- Reopen path: completed -> pending in both directions
- Dedicated idempotency test suite (not per-test boilerplate)
- Fix bugs in-phase: when E2E tests reveal incorrect behavior, fix the code
- Test-first approach for known fixes: write E2E test with skip-unimplemented tag, implement fix, un-skip
- Philosophy: "Better add E2E tests than to try to find the minimal subset"

### Claude's Discretion
- Exact RF test naming and suite organization for new tests
- Whether to extend existing suites (04, 07) or create new ones
- Which field clear operations need unit tests vs E2E-only
- COMPLETED timestamp test strategy (dedicated vs embedded in reopen tests)
- Specific fixture data for new E2E tests

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope

</user_constraints>

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| FIELD-01 | All mapped fields have E2E tests covering create, update, and clear operations | Field mapping analysis (10 fields identified), existing test gap analysis, CalDAVLibrary keyword inventory |
| FIELD-02 | All status transitions tested E2E -- pending<->completed, pending->deleted->CANCELLED, and reverse paths | Status transition matrix, writeback.rs decide_op analysis, CalDAVLibrary keyword gaps identified |
| FIELD-03 | Deletion propagation tested both directions -- TW delete->CalDAV CANCELLED, CalDAV orphan->TW handling | Bug identified in writeback.rs:249-262 (CANCELLED skip), fix approach documented |
| FIELD-04 | Idempotent sync verified -- re-running sync after any operation produces no changes | content_identical check analysis in lww.rs, existing S-32 pattern, idempotency suite design |

</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Robot Framework | 7.x (Docker image) | E2E blackbox testing | Already in use, all infrastructure exists |
| cargo test | (Rust toolchain) | Unit + integration tests | Already in use, 163+ tests passing |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| icalendar (Python) | existing | CalDAV VTODO manipulation in RF keywords | Used by CalDAVLibrary.py for property mutation |
| requests (Python) | existing | HTTP CalDAV operations | Used by CalDAVLibrary.py for PUT/GET/DELETE |

No new dependencies needed for this phase. All work uses existing infrastructure.

## Architecture Patterns

### Existing Test Structure
```
tests/robot/
  suites/
    03_orphan.robot         # S-20 to S-22 (deletion/orphan)
    04_status_mapping.robot # S-30 to S-33 (status transitions)
    05_dependencies.robot   # S-40 to S-45 (RELATED-TO)
    07_field_mapping.robot  # S-60 to S-68 (field create/update)
  resources/
    CalDAVLibrary.py        # CalDAV keywords
    TaskWarriorLibrary.py   # TW keywords
    common.robot            # Shared setup/teardown/keywords
  docs/
    CATALOG.md              # Scenario catalog (single source of truth)
```

### Recommended Suite Organization for Phase 3

**Extend existing suites** for tests that naturally belong to their category:
- Extend `04_status_mapping.robot` with S-34+ for reopen, CANCELLED propagation, and transition tests
- Extend `07_field_mapping.robot` with S-69+ for field update and clear operations

**Create new suite** for idempotency:
- New `08_idempotency.robot` for dedicated idempotency suite (FIELD-04)
- Use S-90 to S-99 range (or define a new range in the catalog)

Rationale: extending existing suites preserves the current organizational pattern. The idempotency tests are a distinct concern worthy of their own suite file.

### RF Test Pattern (established)
```robot
Test Case Name
    [Documentation]    S-XX: User story narrative...
    [Tags]    category-tag
    # Setup: create tasks
    ${uuid} =    TW.Add TW Task    Description
    Run Caldawarrior Sync
    Exit Code Should Be    0
    # Get CalDAV UID for assertions
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    # Mutation
    TW.Modify TW Task    ${uuid}    field=new_value
    Run Caldawarrior Sync
    Exit Code Should Be    0
    # Assertions
    CalDAV.VTODO Should Have Property    ${COLLECTION_URL}    ${caldav_uid}    PROPERTY    expected
```

### Anti-Patterns to Avoid
- **Embedding idempotency assertions in every test:** Use a dedicated suite instead. The CONTEXT.md explicitly says "not per-test boilerplate."
- **Asserting sync counts in idempotency tests:** Use `Sync Should Produce Zero Writes` keyword, not manual count parsing.
- **Skipping LAST-MODIFIED when mutating CalDAV:** The CalDAVLibrary's `modify_vtodo_status` does NOT bump LAST-MODIFIED, which means CalDAV may not win LWW. Tests relying on CalDAV-initiated status changes may need to use a different approach or fix the keyword.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| CalDAV VTODO property assertions | Custom grep on raw iCal | `CalDAV.VTODO Should Have Property` keyword | Already handles icalendar parsing, property extraction |
| TW task field assertions | Custom JSON parsing | `TW.TW Task Should Have Field` keyword | Already handles export + field extraction |
| Zero-write verification | Manual stdout parsing | `Sync Should Produce Zero Writes` keyword | Pattern already defined in common.robot |
| CalDAV VTODO creation | Hand-built iCal strings | `CalDAV.Put VTODO` keyword variants | Handles DTSTAMP, UID, proper formatting |

## Common Pitfalls

### Pitfall 1: CalDAVLibrary modify_vtodo_status Does Not Bump LAST-MODIFIED
**What goes wrong:** Tests that modify CalDAV status expect CalDAV to win LWW, but without LAST-MODIFIED being bumped, TW may win the conflict instead.
**Why it happens:** The `modify_vtodo_summary` keyword bumps LAST-MODIFIED by +2 seconds, but `modify_vtodo_status` does not.
**How to avoid:** Either fix `modify_vtodo_status` to bump LAST-MODIFIED (like `modify_vtodo_summary` does), or ensure the Radicale server sets LAST-MODIFIED on PUT (Radicale does update it automatically on write). Verify which approach is needed by examining what Radicale returns after a PUT.
**Warning signs:** CalDAV status change tests pass but TW wins LWW instead of CalDAV, causing no status propagation.
**Confidence:** MEDIUM -- need to verify Radicale's behavior on PUT. If Radicale auto-updates LAST-MODIFIED, the keyword works as-is. S-30 passes (CalDAV COMPLETED -> TW completed), which suggests Radicale does handle this. But the safe approach is to also bump LAST-MODIFIED in the keyword.

### Pitfall 2: CalDAVLibrary modify_vtodo_status Does Not Clear COMPLETED Timestamp on Reopen
**What goes wrong:** When reopening a COMPLETED task back to NEEDS-ACTION, the COMPLETED timestamp property remains on the VTODO, causing content_identical to see a mismatch on subsequent syncs.
**Why it happens:** The keyword only sets COMPLETED when new_status is "COMPLETED", but never removes it for other statuses.
**How to avoid:** Fix the keyword to delete the COMPLETED property when setting status to NEEDS-ACTION or CANCELLED.
**Warning signs:** Reopen tests fail with unexpected writes on second sync, or CalDAV VTODO retains stale COMPLETED timestamp.
**Confidence:** HIGH -- verified by reading CalDAVLibrary.py lines 386-391.

### Pitfall 3: CANCELLED Propagation Fix Must Handle LWW Correctly
**What goes wrong:** Simply changing the CANCELLED skip to a delete could cause TW tasks to be deleted when TW was the more recent editor.
**Why it happens:** The current code at writeback.rs:249-262 unconditionally skips on CANCELLED without checking LWW timestamps.
**How to avoid:** The fix should use LWW resolution: when CalDAV is CANCELLED and CalDAV's LAST-MODIFIED is newer than TW's modified, propagate deletion to TW. When TW is newer, TW wins and marks CalDAV back to the correct status.
**Warning signs:** Edge case: TW modifies task, CalDAV gets CANCELLED from external client nearly simultaneously -- LWW determines winner.
**Confidence:** HIGH -- the fix approach is clear from the codebase architecture.

### Pitfall 4: Field Clear Semantics for CATEGORIES and DESCRIPTION
**What goes wrong:** Clearing tags in TW should result in the CATEGORIES property being removed entirely from the VTODO (not set to empty). Similarly for annotations -> DESCRIPTION.
**Why it happens:** The serializer in ical.rs (line 169) already handles this: `if !vtodo.categories.is_empty()` gates CATEGORIES emission. And line 152: `if let Some(ref description)` gates DESCRIPTION.
**How to avoid:** The code already correctly omits these properties when empty. The E2E test should verify the property is ABSENT from raw iCal, not just empty.
**Warning signs:** Asserting `CalDAV.VTODO Should Have Property ... CATEGORIES ...` on a task with no tags will fail (property absent).
**Confidence:** HIGH -- verified from ical.rs to_icalendar_string implementation.

### Pitfall 5: Ghost Task Prevention for CalDAV-Only CANCELLED
**What goes wrong:** A CANCELLED VTODO on CalDAV with no TW pair could potentially create a TW task with "deleted" status.
**Why it happens:** If the CalDAV-only decision tree doesn't handle CANCELLED properly.
**How to avoid:** Code at writeback.rs:319-321 already handles this correctly: CalDAV-only CANCELLED entries are skipped with `SkipReason::Cancelled`. Verify with E2E test.
**Warning signs:** CalDAV-only CANCELLED VTODO creates a TW task.
**Confidence:** HIGH -- verified from code.

## Code Examples

### The CANCELLED Propagation Bug (lines 249-262 of writeback.rs)
```rust
// Current behavior (BUG): CalDAV CANCELLED → skip entirely
if caldav_status == "CANCELLED" {
    let reason = if tw_status == "completed" {
        SkipReason::CalDavDeletedTwTerminal
    } else {
        SkipReason::Cancelled   // <-- BUG: should delete TW task when CalDAV wins LWW
    };
    return Some(PlannedOp::Skip {
        tw_uuid: Some(tw_uuid),
        reason,
    });
}
```

The fix should replace the `SkipReason::Cancelled` branch (when TW is NOT completed) with deletion propagation. When CalDAV is CANCELLED and TW is pending/waiting, this means the task was cancelled from the CalDAV side and should propagate to TW.

**Recommended fix approach:**
```rust
if caldav_status == "CANCELLED" {
    if tw_status == "completed" {
        // Both terminal — skip
        return Some(PlannedOp::Skip {
            tw_uuid: Some(tw_uuid),
            reason: SkipReason::CalDavDeletedTwTerminal,
        });
    }
    // CalDAV CANCELLED + TW active → propagate deletion to TW
    return Some(PlannedOp::DeleteFromTw(entry.clone()));
}
```

Note: This is simpler than full LWW because CANCELLED is an explicit terminal state. The `AlreadyDeleted` case (both deleted+cancelled) is handled above this block. The `TwDeletedMarkCancelled` case (TW deleted first) is also handled above. So this branch only fires when CalDAV is CANCELLED but TW is still active -- deletion should always propagate.

### Existing CalDAV Modification Pattern (for reference)
```python
# CalDAVLibrary.py - modify_vtodo_summary sets LAST-MODIFIED
component['SUMMARY'] = new_summary
component['LAST-MODIFIED'] = vDatetime(
    datetime.now(tz=timezone.utc) + timedelta(seconds=2)
)
```

### CalDAVLibrary.py Fix Needed for modify_vtodo_status
```python
# Need to add LAST-MODIFIED bump and COMPLETED clearing:
def modify_vtodo_status(self, collection_url, uid, new_status):
    raw = self.get_vtodo_raw(collection_url, uid)
    cal = Calendar.from_ical(raw)
    for component in cal.walk():
        if component.name == 'VTODO':
            component['STATUS'] = new_status
            component['LAST-MODIFIED'] = vDatetime(
                datetime.now(tz=timezone.utc) + timedelta(seconds=2)
            )
            if new_status == 'COMPLETED':
                component['COMPLETED'] = vDatetime(
                    datetime.now(tz=timezone.utc)
                )
            elif 'COMPLETED' in component:
                del component['COMPLETED']
            break
    # ... PUT back
```

### content_identical Check (8 Fields) — lww.rs
The fields checked for idempotency (content_identical function):
1. SUMMARY vs TW description
2. DESCRIPTION vs TW first annotation text
3. STATUS (normalized: NEEDS-ACTION=pending, COMPLETED=completed, CANCELLED=deleted)
4. DUE (second precision)
5. DTSTART vs TW scheduled (second precision)
6. COMPLETED vs TW end (second precision)
7. RELATED-TO[DEPENDS-ON] vs resolved_depends (sorted)
8. X-TASKWARRIOR-WAIT (second precision, expired TW wait collapsed to None)

Note: PRIORITY and CATEGORIES are NOT in the content_identical check. This means changing only priority or categories will NOT trigger `Skip(Identical)` -- it will fall through to LWW timestamp comparison. This is potentially relevant for idempotency testing.

**IMPORTANT FINDING:** PRIORITY and CATEGORIES are missing from `content_identical`. After a sync that updates these fields, the content_identical check will see matching content on the other 8 fields and return Identical -- but only if PRIORITY and CATEGORIES were NOT the only changed fields. If they WERE the only changed fields, content will appear identical (because those fields aren't checked) and sync will be a no-op. This is correct behavior for idempotency (no writes on second run) but means changes to only priority/categories might not propagate correctly if timestamps don't trigger LWW.

Wait -- looking more carefully: `build_vtodo_from_tw` at line 131 sets `priority: fields.priority` and line 129 sets `categories: tw.tags.clone().unwrap_or_default()`. So TW->CalDAV write DOES include priority and categories. The content_identical check's omission means that after TW writes priority to CalDAV, the next sync won't see a difference (which is correct -- nothing changed). But if CalDAV changes priority and CalDAV wins LWW, the TW task gets updated. Then next sync, content_identical fires because the 8 tracked fields match. Priority is on both sides. This should work correctly.

**Confidence:** HIGH -- the 8-field content_identical check is sufficient because the LWW layer handles the initial propagation, and content_identical prevents infinite loops.

## Field Coverage Gap Analysis

### Current E2E Coverage
| Field | Create TW->CalDAV | Create CalDAV->TW | Update TW->CalDAV | Update CalDAV->TW | Clear |
|-------|:-:|:-:|:-:|:-:|:-:|
| SUMMARY (description) | S-60 | S-64 | -- | S-62 | -- |
| DESCRIPTION (annotations) | S-66 | S-65 | -- | -- | -- |
| STATUS | S-31 (completed) | S-30 (completed) | -- | -- | -- |
| PRIORITY | S-67 (CalDAV->TW) | S-67 | -- | -- | -- |
| DUE | S-61 | -- | -- | -- | -- |
| DTSTART (scheduled) | -- | -- | -- | -- | -- |
| COMPLETED (timestamp) | implicit in S-31 | implicit in S-30 | -- | -- | -- |
| CATEGORIES (tags) | S-68 (tags) | -- | -- | -- | -- |
| RELATED-TO (depends) | S-40 | S-41 | -- | -- | S-45 |
| X-TASKWARRIOR-WAIT | -- | -- | -- | -- | -- |

### Needed E2E Tests (Gaps)
| Field | Missing Operations | Priority |
|-------|-------------------|----------|
| SUMMARY | update TW->CalDAV, clear both directions | HIGH |
| DESCRIPTION | update both directions, clear both directions | HIGH |
| STATUS | reopen (completed->pending) both dirs, CANCELLED->TW delete | HIGH (includes bug fix) |
| PRIORITY | create TW->CalDAV (existing S-67 is CalDAV->TW only for 3 levels), update both dirs, clear both dirs | HIGH |
| DUE | create CalDAV->TW, update both directions, clear both directions | HIGH |
| DTSTART | create both directions, update both directions, clear both directions | HIGH (zero coverage) |
| COMPLETED | explicit timestamp check on complete, cleared on reopen | MEDIUM (may embed in reopen tests) |
| CATEGORIES | create CalDAV->TW, update both directions, clear (tags removed -> no CATEGORIES line) | HIGH |
| RELATED-TO | update (add second dep), clear CalDAV->TW | MEDIUM (S-40/41/45 cover basics) |
| X-TASKWARRIOR-WAIT | create, update, clear (TW-authoritative) | HIGH (zero coverage) |
| Deletion | TW delete->CalDAV CANCELLED (verify), CalDAV CANCELLED->TW delete (fix+test), ghost prevention | HIGH |
| Idempotency | Dedicated suite: multiple field combinations, all operations | HIGH |

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Robot Framework (Docker) + cargo test |
| Config file | `tests/robot/resources/common.robot` (RF), `Cargo.toml` (Rust) |
| Quick run command | `cargo test --lib` |
| Full suite command | `cargo test && CURRENT_UID=$(id -u) CURRENT_GID=$(id -g) docker compose -f tests/robot/docker-compose.yml run --rm robot` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| FIELD-01 | All mapped fields create/update/clear E2E | E2E (RF) | `docker compose -f tests/robot/docker-compose.yml run --rm robot --include field-mapping` | Partial (07_field_mapping.robot exists, needs expansion) |
| FIELD-02 | All status transitions E2E | E2E (RF) | `docker compose -f tests/robot/docker-compose.yml run --rm robot --include status-mapping` | Partial (04_status_mapping.robot exists, needs expansion) |
| FIELD-03 | Deletion propagation both directions | E2E (RF) + unit | `cargo test cancelled && docker compose ... --include deletion` | Partial (unit tests exist, E2E for CANCELLED->TW missing) |
| FIELD-04 | Idempotent sync verification | E2E (RF) | `docker compose ... --include idempotency` | No (new suite needed) |

### Sampling Rate
- **Per task commit:** `cargo test --lib` (fast, unit tests only)
- **Per wave merge:** `cargo test` (full Rust suite including integration)
- **Phase gate:** Full suite: `cargo test && docker compose RF suite` -- all tests green

### Wave 0 Gaps
- [ ] `tests/robot/suites/08_idempotency.robot` -- new suite for FIELD-04
- [ ] Fix `CalDAVLibrary.py modify_vtodo_status` -- add LAST-MODIFIED bump and COMPLETED clearing
- [ ] New CalDAV keyword: `Modify VTODO Field` or `Modify VTODO DUE` (for generic field update tests)
- [ ] New CalDAV keyword: `VTODO Should Not Have Property` (for field clear assertions)
- [ ] Update CATALOG.md with new scenario IDs

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| CalDAV CANCELLED skipped entirely | Should propagate to TW deletion | This phase (fix) | Symmetric bidirectional deletion |
| 8-field content_identical | 8-field content_identical (unchanged) | Phase 1 | PRIORITY and CATEGORIES not in check (acceptable) |
| modify_vtodo_status without LAST-MODIFIED | Should bump LAST-MODIFIED | This phase (fix) | CalDAV status changes reliably win LWW |

## Open Questions

1. **Does Radicale auto-bump LAST-MODIFIED on PUT?**
   - What we know: S-30 passes (CalDAV COMPLETED -> TW completed), suggesting CalDAV status changes propagate correctly even without explicit LAST-MODIFIED bump in the keyword.
   - What's unclear: Whether Radicale sets LAST-MODIFIED on every PUT, or if the icalendar library's serialization produces a LAST-MODIFIED that Radicale preserves.
   - Recommendation: Fix the keyword to explicitly set LAST-MODIFIED anyway (defensive). Also verify with a quick manual test if needed. LOW risk -- S-30 already works.

2. **Should PRIORITY and CATEGORIES be added to content_identical?**
   - What we know: They are omitted from the 8-field check. The LWW timestamp layer handles propagation. The content check prevents infinite loops for the 8 tracked fields.
   - What's unclear: Whether there's a scenario where only priority/categories differ but timestamps don't trigger LWW, causing the change to be permanently lost.
   - Recommendation: This is out of scope for Phase 3 per CONTEXT.md. Document as a potential Phase 4 item if discovered during testing.

3. **COMPLETED timestamp: dedicated tests or embedded?**
   - What we know: User marked this as Claude's Discretion.
   - Recommendation: Embed COMPLETED timestamp assertions in the reopen tests (completed->pending) and the complete tests (pending->completed). Add one explicit assertion per direction: when completing, verify COMPLETED property exists with a timestamp; when reopening, verify COMPLETED property is removed. This avoids a separate test while ensuring coverage.

## Sources

### Primary (HIGH confidence)
- `src/sync/writeback.rs` -- decide_op function (lines 221-328), CANCELLED handling (lines 249-262), build_vtodo_from_tw (lines 84-141), build_tw_task_from_caldav (lines 153-211)
- `src/sync/lww.rs` -- content_identical function (lines 68-139), 8-field check
- `src/mapper/fields.rs` -- bidirectional field mapping (tw_to_caldav_fields, caldav_to_tw_fields)
- `src/mapper/status.rs` -- TW status -> CalDAV status enum
- `src/ical.rs` -- iCal serialization/deserialization, to_icalendar_string field emission logic
- `tests/robot/resources/CalDAVLibrary.py` -- modify_vtodo_status keyword analysis
- `tests/robot/resources/TaskWarriorLibrary.py` -- available TW keywords
- `tests/robot/suites/04_status_mapping.robot` -- existing S-30 to S-33
- `tests/robot/suites/07_field_mapping.robot` -- existing S-60 to S-68
- `tests/robot/suites/03_orphan.robot` -- existing S-20 to S-22
- `tests/robot/docs/CATALOG.md` -- scenario ID ranges and conventions

### Secondary (MEDIUM confidence)
- Radicale LAST-MODIFIED behavior inferred from S-30 test passing

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies, all infrastructure exists
- Architecture: HIGH -- extending established patterns with clear gaps identified
- Pitfalls: HIGH -- verified from source code, specific line numbers cited
- Bug fix approach: HIGH -- single code path identified with clear fix

**Research date:** 2026-03-19
**Valid until:** 2026-04-19 (stable codebase, no external dependency changes expected)
