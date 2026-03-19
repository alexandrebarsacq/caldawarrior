# Phase 7: README Accuracy Fixes - Research

**Researched:** 2026-03-19
**Domain:** Documentation accuracy (Markdown editing)
**Confidence:** HIGH

## Summary

Phase 7 is a documentation-only gap closure phase that addresses two findings from the v1.0 milestone audit: MISS-01 (README contradicts implementation regarding annotation/DESCRIPTION sync) and FLOW-01 (new user flow is partially misleading due to MISS-01). The phase requires four surgical edits to `README.md`, all targeting content that became stale after Phase 3 implemented annotation/DESCRIPTION sync but Phase 6 wrote the README based on earlier assumptions.

The implementation is straightforward: Phase 3 added bidirectional annotation-to-DESCRIPTION mapping (confirmed by `src/mapper/fields.rs` lines 17-18, `src/tw_adapter.rs` annotate/denotate logic, and passing E2E tests S-84/S-85). The README was written during Phase 6 before this was accounted for, resulting in three places where annotation/DESCRIPTION sync is incorrectly described as absent or planned. Additionally, `docs/compatibility/tasks-org.md` exists but is not linked from the README Compatibility section.

**Primary recommendation:** Make exactly four edits to README.md — correct Limitation 12, add Field Mapping row, remove v2 Roadmap entry, add compatibility doc link. No code changes, no test changes, no library dependencies.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| DOC-01 | README covers installation, configuration, usage, and common workflows | Four specific README edits fix documentation inaccuracies identified in v1.0 audit (MISS-01, FLOW-01). All edits are precisely located with line numbers. |
</phase_requirements>

## Standard Stack

Not applicable. This phase modifies only `README.md` (Markdown). No libraries, build tools, or runtime dependencies are involved.

## Architecture Patterns

Not applicable for a documentation-only phase. The only "pattern" is the existing README structure, documented below for the planner's reference.

### README Structure (Current)

```
README.md (493 lines)
├── Features
├── Installation
├── Quick Start
├── Config Reference
├── CLI Reference
├── Scheduling
├── Field Mapping          ← Edit 2: add annotations row
│   └── Status Mapping
├── Compatibility          ← Edit 4: add tasks-org.md link
│   ├── Servers
│   └── Clients
├── v1 Known Limitations
│   ├── 1-11 (unchanged)
│   ├── 12. No description... ← Edit 1: rewrite this entry
│   ├── 13-15 (unchanged)
├── Testing
├── v2 Roadmap             ← Edit 3: remove annotation row
└── License
```

## Don't Hand-Roll

Not applicable for documentation edits.

## Exact Edits Required

All four edits are precisely located. Confidence: HIGH (verified by reading the current README and cross-referencing with source code and E2E tests).

### Edit 1: Correct Limitation 12 (README lines 390-396)

**Current text (WRONG):**
```markdown
### 12. No description or annotation sync

TaskWarrior annotations are not mapped to any CalDAV field. The CalDAV `DESCRIPTION` property
is not imported into TaskWarrior. Only the `SUMMARY` field (mapped to `description`) is synced.

**Workaround:** Keep extended notes in one system only. Neither side will overwrite or delete
the other's extended notes; they simply remain invisible across the sync boundary.
```

**Replacement:**
The limitation title should change (e.g., "Annotation sync limited to first annotation"). The body must reflect reality: the first TW annotation syncs bidirectionally with CalDAV DESCRIPTION. Only the first annotation is mapped (confirmed: `fields.rs` line 72 uses `.first()`). Multiple annotations are not supported. The workaround should explain this single-annotation limitation.

**Evidence:**
- `src/mapper/fields.rs:72` — `task.annotations.first().map(|a| a.description.clone())`
- `src/mapper/fields.rs:138` — `vtodo.description` maps to `annotations_text`
- `src/tw_adapter.rs:342-363` — annotate/denotate diff logic
- E2E tests S-84 (TW annotation -> CalDAV DESCRIPTION) and S-85 (CalDAV DESCRIPTION -> TW annotation) both pass

### Edit 2: Add Field Mapping Row (README line 220)

**Current table** is missing an `annotations` row. Insert after the `description -> SUMMARY` row:

```markdown
| `annotations[0]` | `DESCRIPTION`                  | First annotation only        |
```

The `[0]` notation communicates the single-annotation limitation clearly.

### Edit 3: Remove v2 Roadmap Entry (README line 488)

**Current entry:**
```markdown
| **Annotation / DESCRIPTION sync** | Map TaskWarrior annotations to CalDAV `DESCRIPTION` and vice versa |
```

This row must be removed entirely since the feature is already implemented. The remaining 6 roadmap items stay.

### Edit 4: Add tasks-org.md Link (README Compatibility section, after line 251)

**Current state:** The DEPENDS-ON note references RFC 9253 and Known Limitation 15, but does not link to the detailed `docs/compatibility/tasks-org.md` document.

**Add a link** either within the DEPENDS-ON note paragraph or as a new "See also" line. Example placement after the existing DEPENDS-ON note:

```markdown
For detailed compatibility analysis, see [tasks.org / DAVx5 Compatibility](docs/compatibility/tasks-org.md).
```

The target file exists at `docs/compatibility/tasks-org.md` (verified).

## Common Pitfalls

### Pitfall 1: Limitation Renumbering
**What goes wrong:** Removing Limitation 12 entirely and renumbering 13-15 would break the internal anchor link from the Compatibility section (`#15-depends-on-relations-invisible-to-caldav-clients`).
**How to avoid:** Do NOT remove Limitation 12. Rewrite it in place with corrected content reflecting the single-annotation limitation. Keep numbering 1-15 intact.

### Pitfall 2: Overstating Annotation Support
**What goes wrong:** Writing that "annotations sync bidirectionally" without mentioning the first-annotation-only limitation would create a new documentation inaccuracy.
**How to avoid:** Clearly state that only the first annotation maps to DESCRIPTION. Multiple annotations are not synced. This is the actual behavior per `fields.rs:72`.

### Pitfall 3: Stale Anchor References
**What goes wrong:** If Limitation 12's title changes, the heading anchor changes, and any cross-references would break.
**How to avoid:** Check for internal links referencing `#12-no-description-or-annotation-sync`. A quick grep of the README confirms no internal links reference Limitation 12 by anchor, so the title can safely change.

### Pitfall 4: Relative Link Format
**What goes wrong:** Using an absolute path or wrong relative path for the tasks-org.md link.
**How to avoid:** Use relative path from repo root: `docs/compatibility/tasks-org.md`. This works on GitHub and in local Markdown viewers.

## Code Examples

Not applicable. All edits are Markdown text changes.

## State of the Art

Not applicable for documentation-only changes.

## Open Questions

None. All four edits are precisely defined with exact line numbers and verified against source code. There are no ambiguities.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Manual review (documentation-only phase) |
| Config file | N/A |
| Quick run command | `grep -n "annotations\|DESCRIPTION" README.md` |
| Full suite command | Visual inspection of all four edit locations |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DOC-01 (Edit 1) | Limitation 12 reflects annotation sync reality | manual-only | `sed -n '390,396p' README.md` | N/A |
| DOC-01 (Edit 2) | Field Mapping table includes annotations row | manual-only | `grep 'annotations.*DESCRIPTION' README.md` | N/A |
| DOC-01 (Edit 3) | v2 Roadmap does not list annotation sync | manual-only | `grep -c 'Annotation.*DESCRIPTION sync' README.md` (expect 0) | N/A |
| DOC-01 (Edit 4) | Compatibility links to tasks-org.md | manual-only | `grep 'tasks-org.md' README.md` | N/A |

**Manual-only justification:** Documentation accuracy cannot be meaningfully tested with automated test frameworks. The verification is visual/textual and takes seconds.

### Sampling Rate
- **Per task commit:** Verify edit with grep commands above
- **Per wave merge:** Full README review of four edit locations
- **Phase gate:** All four grep checks pass

### Wave 0 Gaps
None -- no test infrastructure needed for documentation-only edits.

## Sources

### Primary (HIGH confidence)
- `README.md` (lines 208-220, 235-251, 390-396, 476-488) -- current state of all edit targets
- `src/mapper/fields.rs` (lines 17-18, 65-72, 131-180) -- annotation/DESCRIPTION field mapping implementation
- `src/tw_adapter.rs` (lines 272-363) -- annotate/denotate diff logic
- `.planning/v1.0-MILESTONE-AUDIT.md` -- MISS-01 and FLOW-01 gap definitions
- `tests/robot/docs/CATALOG.md` -- S-84 and S-85 E2E test results (both pass)
- `docs/compatibility/tasks-org.md` -- target file for Edit 4 (confirmed to exist)

### Secondary (MEDIUM confidence)
None needed.

### Tertiary (LOW confidence)
None.

## Metadata

**Confidence breakdown:**
- Edit locations: HIGH -- all four verified by reading current README line-by-line
- Annotation behavior: HIGH -- verified in source code (fields.rs, tw_adapter.rs) and E2E tests (S-84, S-85)
- Link target: HIGH -- `docs/compatibility/tasks-org.md` confirmed to exist with 85 lines of content

**Research date:** 2026-03-19
**Valid until:** Indefinite (documentation accuracy does not expire)
