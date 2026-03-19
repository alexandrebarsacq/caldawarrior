# Phase 3: Field and Sync Correctness - Context

**Gathered:** 2026-03-19
**Status:** Ready for planning

<domain>
## Phase Boundary

Verify every mapped field creates, updates, clears, and round-trips correctly, and sync is idempotent. Fix bugs discovered during testing. Mapped fields: SUMMARY, DESCRIPTION, STATUS, PRIORITY, DUE, DTSTART, COMPLETED, CATEGORIES, RELATED-TO, X-TASKWARRIOR-WAIT.

</domain>

<decisions>
## Implementation Decisions

### Field clear semantics
- Bidirectional clear: clearing a field on either side propagates to the other side via LWW
- No tags → no CATEGORIES line in VTODO (remove property entirely, not empty value)
- No annotations → no DESCRIPTION property in VTODO (remove, not empty)
- X-TASKWARRIOR-WAIT is TW-authoritative only: TW clear → remove X-prop from CalDAV. CalDAV-side clear of X-TASKWARRIOR-WAIT is not meaningful (CalDAV clients don't know about this property)

### Deletion propagation
- Verify existing behavior: TW delete → CalDAV STATUS:CANCELLED (already implemented)
- Verify existing behavior: CalDAV orphan (VTODO gone) → TW task deleted (already tested S-20/S-21)
- **FIX**: CalDAV CANCELLED → TW deleted (currently skipped, creates asymmetry). Fix so CalDAV CANCELLED propagates to TW deletion when CalDAV wins LWW
- Add explicit ghost task test: CANCELLED VTODO on CalDAV with no TW pair → no TW task created
- Both sides deleted/cancelled → skip (verify existing AlreadyDeleted behavior)

### Status transitions
- Test all paths bidirectionally with comprehensive E2E coverage (philosophy: more tests > minimal tests)
- Reopen path: completed → pending in both directions (TW reopen → CalDAV NEEDS-ACTION; CalDAV NEEDS-ACTION on COMPLETED → TW pending)
- TW delete → CalDAV CANCELLED (E2E, existing unit-tested behavior)
- CalDAV CANCELLED → TW deleted (E2E, after fix)

### COMPLETED timestamp side-effects
- Claude's Discretion: determine if COMPLETED timestamp set/cleared needs dedicated tests or is already covered by existing assertions

### Idempotency testing (FIELD-04)
- Dedicated idempotency test suite: create tasks with various fields, sync, sync again, assert zero writes on second run
- Not per-test boilerplate — one comprehensive focused suite

### Bug discovery policy
- Fix bugs in-phase: when E2E tests reveal incorrect behavior, fix the code in this phase
- Test-first approach for known fixes: write E2E test expecting correct behavior (skip-unimplemented tag), implement fix, un-skip
- Applies to the CANCELLED→TW fix and any other bugs discovered during E2E expansion

### Claude's Discretion
- Exact RF test naming and suite organization for new tests
- Whether to extend existing suites (04, 07) or create new ones
- Which field clear operations need unit tests vs E2E-only
- COMPLETED timestamp test strategy (dedicated vs embedded in reopen tests)
- Specific fixture data for new E2E tests

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` — FIELD-01 through FIELD-04 define the specific verification requirements
- `.planning/ROADMAP.md` — Phase 3 success criteria (4 conditions that must be TRUE)

### Existing E2E tests (extend, don't duplicate)
- `tests/robot/suites/04_status_mapping.robot` — S-30 through S-33: pending↔completed, stable pending, cutoff
- `tests/robot/suites/07_field_mapping.robot` — S-60 through S-68: SUMMARY, DUE, PRIORITY, CATEGORIES, annotations, project mapping
- `tests/robot/suites/03_orphan.robot` — S-20, S-21: CalDAV orphan → TW deletion
- `tests/robot/suites/05_dependencies.robot` — S-40 through S-45: RELATED-TO create/verify

### Core implementation (where fixes will land)
- `src/sync/writeback.rs` — Deletion/CANCELLED handling logic (lines 232-305), field update application
- `src/mapper/status.rs` — TW status → CalDAV status mapping
- `src/mapper/fields.rs` — Bidirectional field mapping (TW↔CalDAV)
- `src/sync/lww.rs` — LWW comparison including status normalization (CANCELLED → "deleted" at line 34)
- `src/ical.rs` — iCal parsing/serialization for all VTODO properties

### Test infrastructure
- `tests/robot/resources/CalDAVLibrary.py` — CalDAV keywords (VTODO manipulation, property assertions)
- `tests/robot/resources/TaskWarriorLibrary.py` — TW keywords (add, modify, complete, delete, field assertions)
- `tests/robot/resources/common.robot` — Shared setup/teardown, sync keywords
- `tests/robot/docs/CATALOG.md` — Scenario catalog (update with new scenarios)

### Prior phase context
- `.planning/phases/01-code-audit-and-bug-fixes/01-CONTEXT.md` — Test philosophy: spec-oriented, E2E mandatory, no backward compat
- `.planning/phases/02-relation-verification/02-CONTEXT.md` — RELATED-TO testing patterns, skip-unimplemented tag usage

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `CalDAV.Modify VTODO Status` RF keyword — Changes VTODO STATUS (used in S-30)
- `CalDAV.VTODO Should Have Property` RF keyword — Asserts specific property value
- `CalDAV.Get VTODO Raw` RF keyword — Gets raw iCal for detailed property inspection
- `TW.TW Task Should Have Status` RF keyword — Asserts TW task status
- `TW.Delete TW Task` RF keyword — Runs `task <uuid> delete`
- `TW.Modify TW Task` RF keyword — Modifies any TW field (can clear fields)
- `Sync Should Produce Zero Writes` RF keyword — Asserts idempotent sync (used in S-32)
- `skip-unimplemented` RF tag pattern — For tests that need code changes before passing

### Established Patterns
- Suite 07 tests: create task → sync → assert CalDAV property (and reverse direction)
- Suite 04 tests: modify status → sync → assert status propagated
- Orphan tests: delete from one side → sync → assert propagated to other side
- LWW field comparison at second precision in `src/sync/lww.rs`

### Integration Points
- `src/sync/writeback.rs:249-253` — CANCELLED skip logic (needs fix for CalDAV CANCELLED → TW deletion)
- `src/sync/writeback.rs:408` — CalDAV-only terminal entries skip logic (ghost task prevention)
- `src/mapper/fields.rs:62-100` — TW → CalDAV field mapping (clear = None propagation)

</code_context>

<specifics>
## Specific Ideas

- Philosophy: "Better add E2E tests than to try to find the minimal subset of tests that covers everything" — comprehensive over minimal
- Test-first for known fixes: write failing E2E test with skip-unimplemented, then implement fix, then un-skip
- The CANCELLED → TW deletion fix is a known asymmetry that should be fixed in this phase
- Existing S-32 (stable pending) already tests basic idempotency; dedicated suite extends to all operation types

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 03-field-and-sync-correctness*
*Context gathered: 2026-03-19*
