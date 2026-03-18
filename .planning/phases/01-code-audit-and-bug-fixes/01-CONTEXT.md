# Phase 1: Code Audit and Bug Fixes - Context

**Gathered:** 2026-03-18
**Status:** Ready for planning

<domain>
## Phase Boundary

Fix 4 known bugs before any test expansion: CATEGORIES comma-escaping, XML parser namespace handling, error message context, and ETag normalization. All subsequent phases depend on these fixes being correct so that tests validate correct behavior.

</domain>

<decisions>
## Implementation Decisions

### Error reporting behavior
- Verbose by default — show warnings and errors with full context during sync
- Full context in error messages: task UUID, field name, CalDAV href, and the actual values that caused the issue
- Errors and warnings to stderr, sync progress/results to stdout (standard Unix convention)
- Keep direct eprintln!/println! — no logging framework. The codebase already uses this pattern and it's sufficient for a run-and-exit sync CLI

### Failure mode policy
- Default: skip failing task and continue syncing the rest. Log error with full context, report all failures at end
- Add --fail-fast flag for users who want abort-on-first-error behavior
- Non-zero exit code when any task fails (including ETag retry exhaustion) — cron/scripts need to know something went wrong
- XML parser: parse what you can, skip unparseable VTODOs with warnings. One malformed entry shouldn't block 99 valid tasks

### Weak ETag handling
- Claude's Discretion — pick the pragmatic approach based on what CalDAV servers actually do in practice

### Corrupted data handling
- No special handling needed — no existing users, no corrupted data in the wild
- Just fix the CATEGORIES escaping bug in the code

### Regression testing
- Each bug fix ships with its own regression tests proving the fix works
- Tests must be spec-oriented: verify behavior ("a tag with a comma survives round-trip sync") not implementation ("escape_text is called")
- Include E2E tests (Robot Framework with real TW+Radicale), not just unit tests
- XML parser fixtures: use real Radicale server response data captured from Docker environment
- Radicale only for Phase 1 — Nextcloud/Baikal fixtures deferred to Phase 4 (Compatibility)

### Claude's Discretion
- XML library choice for parser replacement
- Specific ETag normalization approach (strip W/ prefix vs. skip conditional write)
- Internal error type refactoring to support full-context propagation
- Exact --fail-fast flag implementation (CLI arg parsing approach)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` — AUDIT-01 through AUDIT-04 define the specific bugs to fix

### Project context
- `.planning/PROJECT.md` — Constraints, tech stack, key decisions
- `.planning/ROADMAP.md` — Phase 1 success criteria (4 conditions that must be TRUE)

### Existing specs (completed, for reference patterns)
- `specs/completed/field-mapping-fix-2026-03-09-001/` — Previous field mapping fixes, established test patterns
- `specs/completed/blackbox-integration-tests-2026-03-01-001/` — RF E2E test patterns and Docker setup
- `specs/completed/native-lww-sync-2026-03-02-001/` — LWW sync implementation patterns

No external RFC/spec docs in repo — RFC 5545 (iCalendar) and RFC 7232 (ETags) should be consulted via web for escaping rules and ETag semantics.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `escape_text()`/`unescape_text()` in `src/ical.rs:315-353` — Already implement RFC 5545 text escaping correctly; must be applied to CATEGORIES parsing
- `CaldaWarriorError` enum in `src/error.rs` — Rich error type with context variants; use instead of `unwrap_or_default()`
- `CalDavClient` trait in `src/caldav_adapter.rs:12-31` — Abstract interface enables mock testing
- `MockCalDavClient` / `MockTaskRunner` — Existing mock implementations for unit tests
- Robot Framework Docker setup in `tests/robot/docker-compose.yml` — Real TW+Radicale E2E environment

### Established Patterns
- Unit tests in same file (`#[cfg(test)]` modules)
- Round-trip serialization tests (e.g., `test_round_trip_basic()` in ical.rs)
- ETag retry logic with `MAX_ETAG_RETRIES = 3` in `src/sync/writeback.rs:18`
- TW 3.x filter-before-command syntax for task operations

### Integration Points
- CATEGORIES parsing: `src/ical.rs:68-75` (parse) and `src/ical.rs:169-171` (serialize)
- XML parsing: `src/caldav_adapter.rs:312-420` (extract_tag_content, parse_multistatus_vtodos, extract_calendar_data)
- Error swallowing: ~10 `unwrap_or_default()` calls across `src/caldav_adapter.rs`, `src/sync/writeback.rs`, `src/sync/deps.rs`
- ETag handling: `src/caldav_adapter.rs:93-97` (extraction), `src/caldav_adapter.rs:176` (If-Match header)

### Key Risk
- XML parser replacement is highest-risk item — touches all CalDAV data flow. Must be tested thoroughly.

</code_context>

<specifics>
## Specific Ideas

- Tests should test the spec, not the implementation — verify observable behavior, not internal function calls
- E2E tests are mandatory alongside unit tests, not optional
- No existing users means no backward compatibility concerns — just fix the bugs cleanly

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 01-code-audit-and-bug-fixes*
*Context gathered: 2026-03-18*
