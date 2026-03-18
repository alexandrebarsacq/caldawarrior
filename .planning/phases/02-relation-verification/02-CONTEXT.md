# Phase 2: Relation Verification - Context

**Gathered:** 2026-03-18
**Status:** Ready for planning

<domain>
## Phase Boundary

Prove that dependency relations — caldawarrior's differentiator — work end-to-end with real servers. TW `depends` UUIDs must map to CalDAV `RELATED-TO;RELTYPE=DEPENDS-ON` UIDs and back. Cycle detection must prevent corrupt data. Third-party client compatibility (tasks.org/DAVx5) must be documented with evidence.

</domain>

<decisions>
## Implementation Decisions

### Cycle handling behavior
- Cyclic tasks sync all non-dependency fields normally — only RELATED-TO properties are omitted for cyclic entries
- Current S-42 E2E test assertions must be updated: cyclic tasks SHOULD appear on CalDAV (with SUMMARY, STATUS, etc.) but WITHOUT RELATED-TO
- Test both 2-node cycles (A→B→A) and 3-node cycles (A→B→C→A) in E2E tests
- Keep current warning format: `CyclicEntry: task '{description}' is part of a dependency cycle` — no chain listing
- Cycle detection stays TW-side only (reads `tw_task.depends` graph in `resolve_dependencies`)
- **Future improvement note:** `resolve_dependencies` could be enhanced to build a unified dependency graph from both `tw_task.depends` and `fetched_vtodo.depends` at the IR level, avoiding duplicate logic if CalDAV-side cycle detection is ever needed. Document this in code comments.

### Blocks (inverse depends)
- TW `blocks` is a computed inverse of `depends` — not a stored field. caldawarrior does NOT write a separate RELATED-TO for the blocks direction
- Only the `depends` direction produces `RELATED-TO;RELTYPE=DEPENDS-ON` on the dependent task's VTODO
- REL-04 verification: set A depends B, sync, verify only A's VTODO has RELATED-TO, then verify B's TW JSON export shows `blocks` containing A's UUID

### tasks.org/DAVx5 compatibility (REL-03)
- Document limitation with evidence rather than manual device testing
- Research tasks.org and DAVx5 handling of RELATED-TO properties (source code, issues, documentation)
- Create `docs/compatibility/tasks-org.md` with findings — confirmed support or documented limitation with evidence
- This doc will be referenced from the Phase 6 compatibility matrix

### Claude's Discretion
- Dep removal sync behavior (whether removing TW depends clears CalDAV RELATED-TO on next sync)
- Internal code changes needed to support "sync everything except deps" for cyclic entries (may require writeback changes)
- Exact fixture data for new E2E tests
- Test naming and RF suite organization

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Core implementation
- `src/sync/deps.rs` — Dependency UUID→UID resolution and cycle detection (DFS). Has 5 unit tests
- `src/ical.rs` lines 80-92, 180-186 — RELATED-TO parsing (with RelType enum) and serialization
- `src/mapper/fields.rs` — Bidirectional depends field mapping (TW→CalDAV and CalDAV→TW) with round-trip tests
- `src/sync/mod.rs` — Pipeline order: build_ir → resolve_dependencies → apply_writeback
- `src/sync/writeback.rs` — Where cyclic entries must be handled (sync fields but skip RELATED-TO)
- `src/ir.rs` — IR construction, IREntry has both `tw_task.depends` and `fetched_vtodo.depends`
- `src/types.rs` — IREntry struct with `resolved_depends`, `cyclic` fields; RelType enum

### Existing E2E tests
- `tests/robot/suites/05_dependencies.robot` — S-40 (forward sync), S-41 (reverse sync), S-42 (cyclic, skip-unimplemented)
- `tests/robot/resources/CalDAVLibrary.py` — `Add Vtodo Related To` keyword
- `tests/robot/resources/TaskWarriorLibrary.py` — `TW Task Should Depend On` keyword

### Test documentation
- `tests/robot/docs/CATALOG.md` — Scenarios S-40, S-41, S-42 documented
- `tests/robot/docs/GAP_ANALYSIS.md` — Section 4: RELATED-TO → depends mapping analysis

### Requirements
- `.planning/REQUIREMENTS.md` — REL-01 through REL-04
- `.planning/ROADMAP.md` — Phase 2 success criteria (4 conditions)

### Prior phase context
- `.planning/phases/01-code-audit-and-bug-fixes/01-CONTEXT.md` — Test philosophy: spec-oriented tests, E2E mandatory, no backward compat concerns

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `resolve_dependencies()` in `src/sync/deps.rs` — Already implements full cycle detection via iterative DFS and UUID→UID resolution
- `RelType` enum in `src/types.rs` — `DependsOn` and `Other(String)` variants, used in parsing and serialization
- `Add Vtodo Related To` RF keyword in `CalDAVLibrary.py:411` — Adds RELATED-TO to a VTODO on the CalDAV server
- `TW Task Should Depend On` RF keyword in `TaskWarriorLibrary.py:418` — Asserts depends field contains a UUID
- `MockCalDavClient` / `MockTaskRunner` — Existing mocks for unit/integration tests

### Established Patterns
- Unit tests in same file (`#[cfg(test)]` modules) — follow this for new deps.rs tests
- RF test structure: `tests/robot/suites/05_dependencies.robot` already organized for dependency tests
- `skip-unimplemented` tag pattern for tests that need code changes before passing
- Round-trip test pattern in `mapper/fields.rs` — `depends_round_trip_tw_to_caldav_to_tw()`

### Integration Points
- `src/sync/writeback.rs` — Must check `entry.cyclic` and omit RELATED-TO while still syncing other fields
- `src/sync/mod.rs:60` — `resolve_dependencies` call site; pipeline order is fixed
- `src/ical.rs:180-186` — RELATED-TO serialization; cyclic entries need this skipped
- `src/output.rs` — Warning output formatting (CyclicEntry warnings go through here)

</code_context>

<specifics>
## Specific Ideas

- S-42 assertions need to change: cyclic tasks SHOULD produce VTODOs (just without RELATED-TO), not 0 VTODOs
- The `blocks` verification test should query TW JSON export and check the computed `blocks` field directly
- tasks.org research should check their GitHub repo (tasks/tasks) for RELATED-TO handling in their CalDAV sync code
- Add a code comment in `resolve_dependencies` noting the potential for IR-level unified graph detection as a future enhancement

</specifics>

<deferred>
## Deferred Ideas

- IR-level unified cycle detection (covering both TW and CalDAV directions in one pass) — future enhancement, noted in code
- RELTYPE=CHILD / RELTYPE=PARENT support — not in scope, caldawarrior only handles DEPENDS-ON
- Real device testing with tasks.org + DAVx5 — documented limitation is sufficient for v1

</deferred>

---

*Phase: 02-relation-verification*
*Context gathered: 2026-03-18*
