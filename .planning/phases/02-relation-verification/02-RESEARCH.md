# Phase 2: Relation Verification - Research

**Researched:** 2026-03-19
**Domain:** CalDAV RELATED-TO / DEPENDS-ON relation mapping, cycle detection, E2E testing
**Confidence:** HIGH

## Summary

Phase 2 proves that caldawarrior's dependency relation mapping works end-to-end with real servers. The core implementation already exists: `resolve_dependencies()` in `deps.rs` handles UUID-to-UID resolution and cycle detection via iterative DFS, `ical.rs` handles RELATED-TO parsing/serialization, and `mapper/fields.rs` handles bidirectional depends mapping. The main code change required is modifying how cyclic entries are handled -- currently they are fully skipped, but the user decision mandates syncing all fields EXCEPT RELATED-TO for cyclic tasks.

Existing E2E tests (S-40, S-41, S-42) cover the basic scenarios but S-42 needs updated assertions per the user decision, and new tests are needed for 3-node cycles, dependency removal, and blocks verification. The tasks.org/DAVx5 compatibility requirement (REL-03) is addressed through source code research and documentation rather than physical device testing.

**Primary recommendation:** Change the cyclic entry handling from full skip to "sync without RELATED-TO," update S-42 assertions, add new E2E tests, and create the compatibility documentation.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Cyclic tasks sync all non-dependency fields normally -- only RELATED-TO properties are omitted for cyclic entries
- Current S-42 E2E test assertions must be updated: cyclic tasks SHOULD appear on CalDAV (with SUMMARY, STATUS, etc.) but WITHOUT RELATED-TO
- Test both 2-node cycles (A->B->A) and 3-node cycles (A->B->C->A) in E2E tests
- Keep current warning format: `CyclicEntry: task '{description}' is part of a dependency cycle` -- no chain listing
- Cycle detection stays TW-side only (reads `tw_task.depends` graph in `resolve_dependencies`)
- TW `blocks` is a computed inverse of `depends` -- not a stored field. caldawarrior does NOT write a separate RELATED-TO for the blocks direction
- Only the `depends` direction produces `RELATED-TO;RELTYPE=DEPENDS-ON` on the dependent task's VTODO
- REL-04 verification: set A depends B, sync, verify only A's VTODO has RELATED-TO, then verify B's TW JSON export shows `blocks` containing A's UUID
- Document limitation with evidence rather than manual device testing for tasks.org/DAVx5 compatibility
- Research tasks.org and DAVx5 handling of RELATED-TO properties (source code, issues, documentation)
- Create `docs/compatibility/tasks-org.md` with findings

### Claude's Discretion
- Dep removal sync behavior (whether removing TW depends clears CalDAV RELATED-TO on next sync)
- Internal code changes needed to support "sync everything except deps" for cyclic entries (may require writeback changes)
- Exact fixture data for new E2E tests
- Test naming and RF suite organization

### Deferred Ideas (OUT OF SCOPE)
- IR-level unified cycle detection (covering both TW and CalDAV directions in one pass)
- RELTYPE=CHILD / RELTYPE=PARENT support -- caldawarrior only handles DEPENDS-ON
- Real device testing with tasks.org + DAVx5
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| REL-01 | DEPENDS-ON relation syncs end-to-end with real Radicale server -- TW depends UUIDs map to CalDAV RELATED-TO UIDs and back | Existing S-40 (forward) and S-41 (reverse) tests cover this; both already pass. May need assertion hardening. |
| REL-02 | Cycle detection works end-to-end -- circular dependencies are detected, warned, and skipped without data loss | Requires code change: cyclic entries currently fully skipped, must change to "sync without RELATED-TO". S-42 assertions need update. New 3-node cycle test needed. |
| REL-03 | tasks.org compatibility verified -- DEPENDS-ON properties preserved through tasks.org+DAVx5 round-trip (or documented limitation) | Research complete: tasks.org uses RELATED-TO;RELTYPE=PARENT for subtasks, has documented bugs with subtask sync. DEPENDS-ON is RFC 9253 extension, not widely supported. Create docs/compatibility/tasks-org.md. |
| REL-04 | blocks (inverse depends) mapping verified -- TW blocks correctly maps to RELATED-TO in reverse direction | New E2E test needed: set A depends B, sync, verify A's VTODO has RELATED-TO, B's VTODO does not, B's TW export shows blocks containing A's UUID. |
</phase_requirements>

## Architecture Patterns

### Current Sync Pipeline (Fixed Order)
```
build_ir() -> resolve_dependencies() -> apply_writeback()
```

This order is established in `src/sync/mod.rs:57-63` and must not change. `resolve_dependencies` populates `entry.resolved_depends` (CalDAV UIDs) and marks `entry.cyclic = true` for cycle nodes. `apply_writeback` consumes these fields.

### Key Data Flow for Dependencies

```
TW -> CalDAV direction:
  TWTask.depends: Vec<Uuid>
    -> resolve_dependencies() maps UUID -> CalDAV UID
    -> IREntry.resolved_depends: Vec<String>
    -> build_vtodo_from_tw() uses resolved_depends (NOT raw UUIDs)
    -> VTODO.depends: Vec<(RelType::DependsOn, String)>
    -> ical::to_icalendar_string() -> "RELATED-TO;RELTYPE=DEPENDS-ON:{uid}"

CalDAV -> TW direction:
  VTODO.depends: Vec<(RelType::DependsOn, String)>
    -> caldav_to_tw_fields() filters DEPENDS-ON, parses UID as Uuid
    -> CalDavTwFields.depends: Vec<Uuid>
    -> build_tw_task_from_caldav() uses caldav_uid_to_tw_uuid reverse index
    -> TWTask.depends: Vec<Uuid>
```

### Critical Code Locations

| File | Lines | Function | Relevance |
|------|-------|----------|-----------|
| `src/sync/deps.rs` | 19-149 | `resolve_dependencies()` | UUID->UID resolution + DFS cycle detection |
| `src/sync/writeback.rs` | 82-141 | `build_vtodo_from_tw()` | Uses `entry.resolved_depends` for RELATED-TO |
| `src/sync/writeback.rs` | 153-211 | `build_tw_task_from_caldav()` | Reverse maps CalDAV UIDs to TW UUIDs |
| `src/sync/writeback.rs` | 217-338 | `decide_op()` | Line 233: cyclic skip -- THIS MUST CHANGE |
| `src/ical.rs` | 80-92 | `from_icalendar_string()` | Parses RELATED-TO with RELTYPE |
| `src/ical.rs` | 180-186 | `to_icalendar_string()` | Serializes RELATED-TO;RELTYPE= |
| `src/mapper/fields.rs` | 62-119 | `tw_to_caldav_fields()` | Maps depends UUIDs (raw, for field mapping) |
| `src/mapper/fields.rs` | 130-179 | `caldav_to_tw_fields()` | Filters DEPENDS-ON, parses UIDs |

### Required Code Change: Cyclic Entry Handling

**Current behavior** (writeback.rs line 232-238):
```rust
// Cyclic entries are unsafe to write-back; skip them.
if entry.cyclic {
    return Some(PlannedOp::Skip {
        tw_uuid: Some(tw_uuid),
        reason: SkipReason::Cyclic,
    });
}
```

**Required behavior:** Cyclic entries MUST proceed through normal sync logic, but with `resolved_depends` cleared so RELATED-TO properties are NOT emitted. Two approaches:

**Approach A (Recommended): Clear resolved_depends before decision tree.**
In `apply_entry()` or at the start of the main loop, if `entry.cyclic`, set `entry.resolved_depends = vec![]`. Then remove the cyclic skip from `decide_op()`. The entry proceeds through normal LWW/push/pull logic, and `build_vtodo_from_tw()` already uses `entry.resolved_depends` (which is now empty), so no RELATED-TO is emitted.

**Approach B: Add a new PlannedOp variant.**
Create `PlannedOp::PushWithoutDeps` or similar. More complex, more code, unnecessary.

**Approach A is cleaner** because it requires only:
1. Remove the cyclic skip from `decide_op()` paired branch
2. Clear `entry.resolved_depends` for cyclic entries before entering the decision tree
3. Handle cyclic TW-only entries (currently no cyclic check in TW-only branch)
4. Update the `cyclic_entry_skipped` test

**Additional gap:** TW-only entries (line ~296-316) do NOT check `entry.cyclic`. A cyclic TW-only task would be pushed to CalDAV WITH RELATED-TO. The fix is the same: clear `resolved_depends` before the decision tree for all cyclic entries.

### Dependency Removal Sync (Claude's Discretion)

When a TW `depends` is removed (via `task modify depends:` to remove), the next sync should:
1. The TW task's `depends` field no longer contains the removed UUID
2. `resolve_dependencies()` produces empty `resolved_depends` for that entry
3. `build_vtodo_from_tw()` generates a VTODO with no RELATED-TO
4. The CalDAV PUT replaces the existing VTODO, effectively removing the RELATED-TO

This already works with the existing code because `build_vtodo_from_tw()` rebuilds the VTODO from scratch using `resolved_depends`, not by merging with existing CalDAV data. The VTODO is fully replaced on PUT. **No code changes needed for dep removal.**

### Anti-Patterns to Avoid
- **Do NOT add cyclic handling in multiple places.** Clear `resolved_depends` in ONE location, before the decision tree.
- **Do NOT create a separate code path for cyclic entries.** The whole point is that cyclic entries follow the normal path, just without RELATED-TO.
- **Do NOT modify `resolve_dependencies()` to skip cycle marking.** The warning output and `entry.cyclic` flag are still needed for logging.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Cycle detection | Custom graph algorithms | Existing `resolve_dependencies()` DFS | Already correct, tested, handles multi-node cycles |
| RELATED-TO parsing | Manual string splitting | Existing `ical.rs` parser with `parse_property_line` | Handles RELTYPE params, quoted values, line folding |
| UUID <-> UID mapping | Custom lookup | Existing `build_caldav_index()` reverse map | Already built per-writeback, O(1) lookups |
| RF CalDAV operations | Raw HTTP calls | Existing `CalDAVLibrary.py` keywords | `Add Vtodo Related To`, `Get VTODO Raw`, etc. already work |

## Common Pitfalls

### Pitfall 1: Testing blocks with TW 3.x
**What goes wrong:** TW 3.x uses filter-before-command syntax. `task export <uuid>` works but `task <uuid> _show` or `task <uuid> info` might not expose `blocks` in JSON.
**Why it happens:** `blocks` is a computed field, not stored. TW computes it from the reverse of `depends` across all tasks.
**How to avoid:** Use `task export` and check the JSON output. The `blocks` field appears in TW's export as a derived field when another task depends on this one.
**Warning signs:** Test passes locally but fails in Docker (different TW version).

### Pitfall 2: S-42 Assertion Change
**What goes wrong:** S-42 currently asserts `Should Be Equal As Integers ${count} 0` (no VTODOs). After the change, cyclic tasks WILL produce VTODOs.
**Why it happens:** The behavior change means cyclic tasks are no longer skipped.
**How to avoid:** Update S-42 to assert: (1) VTODOs exist (count=2 for 2-node), (2) VTODOs have SUMMARY/STATUS, (3) VTODOs do NOT have RELATED-TO, (4) stderr still contains CyclicEntry warnings.

### Pitfall 3: Cyclic TW-Only Entry Gap
**What goes wrong:** A TW-only task in a cycle gets pushed to CalDAV WITH RELATED-TO because the TW-only branch doesn't check `entry.cyclic`.
**Why it happens:** The cyclic check was only in the paired branch.
**How to avoid:** Apply the `resolved_depends` clearing universally before the decision tree, not conditionally per branch.

### Pitfall 4: Sync Summary Counts
**What goes wrong:** After changing cyclic behavior, the sync output counts change. S-42 asserts `Synced: 0 created, 0 updated in CalDAV`.
**Why it happens:** Cyclic entries were skipped, now they will be pushed/updated.
**How to avoid:** Update S-42 stdout assertions to match new behavior (e.g., `Synced: 2 created, 0 updated in CalDAV`).

### Pitfall 5: TW blocks Field Visibility
**What goes wrong:** `blocks` may not appear in `task export` for older TW versions or when no task depends on the target.
**Why it happens:** `blocks` is only computed when there exists a task with `depends` pointing to this task's UUID.
**How to avoid:** Always verify `blocks` AFTER setting `depends` on another task and exporting both. The RF keyword `TW Task Should Depend On` checks `depends`, not `blocks` -- a new keyword may be needed.

## Code Examples

### Current RELATED-TO Serialization (ical.rs:180-186)
```rust
// Source: src/ical.rs
for (rel, uid) in &vtodo.depends {
    let reltype_str = match rel {
        RelType::DependsOn => "DEPENDS-ON".to_string(),
        RelType::Other(s) => s.clone(),
    };
    lines.push(format!("RELATED-TO;RELTYPE={}:{}", reltype_str, uid));
}
```

### Current build_vtodo_from_tw Uses resolved_depends (writeback.rs:132-138)
```rust
// Source: src/sync/writeback.rs
depends: entry
    .resolved_depends
    .iter()
    .map(|uid| (RelType::DependsOn, uid.clone()))
    .collect(),
```

### Proposed Fix: Clear resolved_depends for Cyclic Entries
```rust
// In apply_entry() or apply_writeback(), BEFORE decide_op():
if entry.cyclic {
    entry.resolved_depends.clear();
}
// Then remove the cyclic skip from decide_op()
```

### RF Test Pattern: Verify VTODO Has No RELATED-TO
```robot
${raw} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${caldav_uid}
Should Not Contain    ${raw}    RELATED-TO
Should Contain    ${raw}    SUMMARY
```

### RF Test Pattern: Verify TW blocks Field
```robot
# After setting A depends B:
${task_b_json} =    TW.Get TW Task    ${uuid_b}
# TW 3.x: blocks is a list of UUIDs
Should Contain    ${task_b_json}[blocks]    ${uuid_a}
```

## tasks.org / DAVx5 Compatibility Research (REL-03)

### Findings

**tasks.org (tasks/tasks on GitHub):**
- Uses `RELATED-TO;RELTYPE=PARENT` for subtask hierarchies (PARENT is the RFC 5545 default)
- Has documented bugs with subtask sync ordering (GitHub issue #3023): tasks initially import correctly but PUT requests can break hierarchy
- **Does NOT use RELTYPE=DEPENDS-ON.** DEPENDS-ON is defined in RFC 9253 (2022), a relatively new extension. tasks.org implements parent-child relationships only.
- When tasks.org encounters unknown RELTYPE values, behavior is undocumented -- it likely preserves the raw property (since it stores the full iCal text) but does not render it in the UI
- Confidence: MEDIUM (based on issue tracker analysis, not source code audit)

**DAVx5:**
- Acts as a transparent sync proxy between CalDAV server and task apps
- Does not interpret VTODO semantics -- it passes through iCalendar data to the task provider (tasks.org, jtx Board, OpenTasks)
- RELATED-TO properties should survive DAVx5 sync since DAVx5 transfers raw VCALENDAR data
- Confidence: MEDIUM (based on documentation review)

**jtx Board:**
- Explicitly supports RELATED-TO for cross-linking tasks, notes, journals
- Most standards-compliant of the Android task apps
- May support DEPENDS-ON -- but no explicit confirmation found
- Confidence: LOW (would need source code verification)

**Radicale (CalDAV server):**
- File-based storage: stores .ics files as-is
- ALL iCalendar properties are preserved verbatim -- Radicale does not parse or filter RELATED-TO
- RELATED-TO;RELTYPE=DEPENDS-ON will survive round-trips through Radicale
- Confidence: HIGH (architecture guarantees preservation)

### Recommendation for docs/compatibility/tasks-org.md

Document:
1. DEPENDS-ON is RFC 9253, distinct from RFC 5545 PARENT/CHILD
2. tasks.org supports PARENT only, not DEPENDS-ON
3. DAVx5 is transparent -- properties pass through
4. Radicale preserves all properties
5. DEPENDS-ON will survive server round-trips but will NOT be rendered as dependencies in tasks.org (it will be invisible)
6. Link to tasks/tasks#3023 as evidence of subtask sync issues

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust cargo test (unit/integration) + Robot Framework (E2E) |
| Config file | `Cargo.toml` (Rust), `tests/robot/` (RF) |
| Quick run command | `cargo test --lib -q` |
| Full suite command | `cargo test && CURRENT_UID=$(id -u) CURRENT_GID=$(id -g) docker compose -f tests/robot/docker-compose.yml run --rm robot` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| REL-01 | DEPENDS-ON forward sync | E2E | RF: `05_dependencies.robot::TW Depends Syncs To CalDAV Related-To` | Yes (S-40) |
| REL-01 | DEPENDS-ON reverse sync | E2E | RF: `05_dependencies.robot::CalDAV Related-To Syncs To TW Depends` | Yes (S-41) |
| REL-02 | 2-node cycle detection | E2E | RF: `05_dependencies.robot::Cyclic Dependency...` (S-42, needs assertion update) | Yes (needs fix) |
| REL-02 | 3-node cycle detection | E2E | RF: `05_dependencies.robot::<new test>` | No -- Wave 0 |
| REL-02 | Cyclic sync-without-deps | unit | `cargo test cyclic` | Yes (needs update) |
| REL-03 | tasks.org compatibility doc | manual-only | Review `docs/compatibility/tasks-org.md` | No -- new file |
| REL-04 | blocks inverse mapping | E2E | RF: `05_dependencies.robot::<new test>` | No -- Wave 0 |
| REL-04 | Only dependent has RELATED-TO | E2E | RF: included in blocks test | No -- Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test --lib -q`
- **Per wave merge:** `cargo test` (full unit + integration)
- **Phase gate:** Full suite green + RF E2E suite green

### Wave 0 Gaps
- [ ] Update `cyclic_entry_skipped` unit test in `writeback.rs` -- must verify cyclic entries ARE written (not skipped)
- [ ] New unit test: cyclic entry produces VTODO without RELATED-TO
- [ ] New RF test: 3-node cycle in `05_dependencies.robot`
- [ ] New RF test: blocks verification in `05_dependencies.robot`
- [ ] Update S-42 assertions in `05_dependencies.robot` -- remove `skip-unimplemented` tag
- [ ] New RF keyword in `TaskWarriorLibrary.py`: `TW Task Should Have Blocks` or similar for checking computed `blocks` field
- [ ] Create `docs/compatibility/` directory and `tasks-org.md`

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| RFC 5545 RELATED-TO (PARENT/CHILD only) | RFC 9253 adds DEPENDS-ON, REFID, LINK | 2022 (RFC 9253 published) | DEPENDS-ON is caldawarrior's key differentiator but few clients implement it |
| Skip cyclic entries entirely | Sync all fields, omit RELATED-TO | Phase 2 decision | Better data preservation for cyclic tasks |

**Deprecated/outdated:**
- tasks.org PARENT subtask sync has known bugs (#3023) -- do not assume stable behavior

## Open Questions

1. **TW blocks field in JSON export -- is it always present?**
   - What we know: `blocks` is computed by TW, appears when another task depends on this UUID
   - What's unclear: Does TW 3.x always include it in `task export` output? Format? (comma-separated string vs array)
   - Recommendation: Implement the test and handle both formats (the existing `tw_depends` serde module already handles both)

2. **tasks.org behavior when encountering RELTYPE=DEPENDS-ON**
   - What we know: tasks.org only implements PARENT relationships. DEPENDS-ON is likely preserved in raw iCal but invisible in UI
   - What's unclear: Does tasks.org strip unknown RELATED-TO properties on sync-back?
   - Recommendation: Document as "likely preserved but not rendered" with MEDIUM confidence. Link to GitHub issues as evidence. Note this is acceptable for v1 per user decision.

## Sources

### Primary (HIGH confidence)
- RFC 9253: https://datatracker.ietf.org/doc/html/rfc9253 -- DEPENDS-ON RELTYPE definition
- Source code analysis: `src/sync/deps.rs`, `src/sync/writeback.rs`, `src/ical.rs`, `src/mapper/fields.rs`
- Existing tests: `tests/robot/suites/05_dependencies.robot` (S-40, S-41, S-42)

### Secondary (MEDIUM confidence)
- tasks/tasks#3023: https://github.com/tasks/tasks/issues/3023 -- RELATED-TO handling bugs in tasks.org
- DAVx5 documentation: https://manual.davx5.com/tasks_notes.html -- transparent VTODO sync
- tasks.org CalDAV docs: https://tasks.org/docs/caldav_intro.html

### Tertiary (LOW confidence)
- jtx Board RELATED-TO support -- stated in marketing materials but DEPENDS-ON not confirmed

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Rust + existing codebase, no new dependencies needed
- Architecture: HIGH - code change is small and well-scoped (clear resolved_depends for cyclic entries)
- Pitfalls: HIGH - identified from direct source code analysis
- Compatibility: MEDIUM - tasks.org behavior inferred from issues, not verified with source code

**Research date:** 2026-03-19
**Valid until:** 2026-04-19 (stable domain, RFC-based)
