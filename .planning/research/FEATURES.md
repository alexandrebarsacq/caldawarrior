# Feature Landscape

**Domain:** CalDAV/VTODO bidirectional sync tool (TaskWarrior <-> CalDAV)
**Researched:** 2026-03-18
**Mode:** Hardening milestone -- focus on what needs verification/testing, not greenfield features

## Table Stakes

Features users expect from a production-quality CalDAV sync tool. Missing = product feels broken.

### VTODO Field Support

| Feature | Why Expected | Complexity | Status | Notes |
|---------|--------------|------------|--------|-------|
| UID preservation | RFC 5545 REQUIRED property; identity anchor | Low | DONE | Caldawarrior uses TW UUID as CalDAV UID via caldavuid UDA |
| DTSTAMP emission | RFC 5545 REQUIRED; many servers reject without it | Low | DONE | Emitted on serialize in ical.rs |
| SUMMARY round-trip | Primary task title; every client shows this | Low | DONE | Maps to TW description |
| DESCRIPTION round-trip | Notes/body text; used by all major clients | Low | DONE | Maps to TW first annotation |
| STATUS mapping (NEEDS-ACTION, COMPLETED, CANCELLED) | Core task lifecycle; IN-PROCESS also common | Low | DONE | Four CalDAV statuses mapped; unknown falls back to NEEDS-ACTION |
| PRIORITY mapping | Standard 1-9 scale; tasks.org and Apple Reminders use it | Low | DONE | H=1, M=5, L=9 bidirectional |
| DUE date round-trip | Deadlines are fundamental to task management | Low | DONE | DateTime with UTC and TZID support |
| DTSTART round-trip | Start/scheduled dates; tasks.org shows this | Low | DONE | Maps to TW scheduled |
| COMPLETED timestamp | When a task was completed; required for COMPLETED status | Low | DONE | Maps to TW end |
| CATEGORIES round-trip | Tags/labels; tasks.org, Nextcloud Tasks, Thunderbird all use | Low | DONE | Maps to TW tags |
| LAST-MODIFIED emission | Conflict resolution anchor; most servers set this | Low | DONE | Used for LWW; DTSTAMP fallback |
| ETag-based conditional writes | Prevents data loss on concurrent updates (If-Match header) | Medium | DONE | Retry loop with 3 attempts |
| iCal TEXT escaping (RFC 5545 3.3.11) | Commas, semicolons, backslashes, newlines must be escaped | Low | DONE | escape_text/unescape_text in ical.rs |
| Line folding at 75 octets (RFC 5545 3.1) | Required by spec; servers may reject unfold input | Low | DONE | fold_line/unfold_lines in ical.rs |
| TZID datetime parsing | Clients send timezone-qualified dates; tasks.org does | Medium | DONE | chrono-tz conversion in parse_datetime_with_params |
| DATE-only parsing (YYYYMMDD) | All-day due dates; tasks.org and Thunderbird send these | Low | DONE | Handled in parse_datetime_with_params |
| Non-standard property preservation | Sabre/dav best practice: GET then PUT must preserve unknown X-props | Medium | DONE | extra_props Vec preserved on round-trip |
| Bidirectional sync (not just push) | Users change tasks on phone (tasks.org) and CLI (TW) | High | DONE | Full IR-based pipeline |
| Conflict resolution (LWW) | Both sides can change simultaneously | Medium | DONE | Two-layer: content-identical check + timestamp LWW |
| Loop prevention (stable point) | Sync must not create infinite update cycles | High | DONE | Content-identical check on 8 fields prevents loops |
| Task creation from either side | Users create tasks in TW or in tasks.org | Medium | DONE | PushToCalDav and PullFromCalDav ops |
| Task deletion propagation | Deleting on one side should propagate | Medium | DONE | TW deleted -> CANCELLED on CalDAV; orphaned caldavuid -> TW delete |
| Dry-run mode | Users need to preview sync before committing | Low | DONE | --dry-run with formatted output |

### Sync Correctness

| Feature | Why Expected | Complexity | Status | Notes |
|---------|--------------|------------|--------|-------|
| Idempotent sync (re-run = no-op) | Running sync twice should not change anything | High | DONE | content_identical check; tested in integration |
| No data loss on field clearing | Removing DUE in CalDAV must clear DUE in TW, not keep stale | Medium | DONE | ADR: tw-field-clearing.md documents trailing-colon clearing |
| No task resurrection after deletion | Deleted TW task must not reappear from stale CalDAV VTODO | High | DONE | Orphaned caldavuid detection; ADR: loop-prevention.md |
| Completed task cutoff | Don't sync ancient completed tasks | Low | DONE | completed_cutoff_days config (default 90) |
| Recurring task skip with warning | RRULE VTODOs are complex; skipping is acceptable if warned | Low | DONE | Both TW recurring and CalDAV RRULE skipped with warnings |

### Production Quality

| Feature | Why Expected | Complexity | Status | Notes |
|---------|--------------|------------|--------|-------|
| Clear error messages | Users need to diagnose auth failures, network issues | Low | PARTIAL | Auth errors clear; some unwrap_or_default paths swallow context |
| Exit code 1 on failure | Scripts/cron need reliable exit codes | Low | DONE | main.rs exits 1 on error |
| Config file security | Passwords in config; must warn on bad permissions | Low | DONE | 0600 check with [WARN]; env var override |
| TLS support with insecure option | Self-signed certs common for self-hosted Radicale | Low | DONE | allow_insecure_tls config |
| Password env var override | CI/CD and secret managers need this | Low | DONE | CALDAWARRIOR_PASSWORD |

## Differentiators

Features that set caldawarrior apart. Not universally expected, but highly valued.

| Feature | Value Proposition | Complexity | Status | Notes |
|---------|-------------------|------------|--------|-------|
| RELATED-TO;RELTYPE=DEPENDS-ON bidirectional sync | **Only sync tool with dependency support.** twcaldav has zero relation support. syncall has none. This is caldawarrior's unique selling point. | High | DONE (untested E2E with real clients) | TW depends UUIDs -> CalDAV RELATED-TO UIDs; reverse mapping implemented. RFC 9253 defines DEPENDS-ON as standard reltype. |
| Cycle detection in dependency graphs | Prevents broken sync when circular deps exist | Medium | DONE | Three-colour DFS; cyclic tasks skipped with warning |
| Project-to-calendar mapping | Route TW projects to separate CalDAV calendars | Low | DONE | TOML [[calendar]] config with project field |
| Stateless design (no sync database) | Simpler deployment; correlation via caldavuid UDA only | Medium | DONE | No external state file to manage or corrupt |
| Single static binary (Rust) | Easy deployment; no runtime dependencies | Low | DONE | cargo build produces single binary |
| tasks.org compatibility | Most popular open-source Android task app with CalDAV | Medium | PARTIAL | Core VTODO works; X-APPLE-SORT-ORDER and subtask hierarchy not tested |

## Anti-Features

Features to explicitly NOT build. These are out of scope by design.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Parent/child subtask hierarchy (RELATED-TO;RELTYPE=PARENT/CHILD) | TW has no native subtask model. Mapping would be lossy and confusing. tasks.org's own subtask sync has documented bugs (issue #3023, #932). The complexity-to-value ratio is terrible. | Only support DEPENDS-ON relations (flat dependency graph). Document limitation. If a CalDAV client sends PARENT/CHILD RELATED-TO, preserve as extra_props but do not map to TW. |
| PERCENT-COMPLETE field mapping | TW has no percent-complete concept. Mapping STATUS + PERCENT-COMPLETE bidirectionally is a known source of sync bugs (Outlook CalDAV Synchronizer had multiple bug reports). Ambiguous interaction with STATUS field. | Preserve PERCENT-COMPLETE as extra_prop on round-trip. Do not attempt to map to TW. |
| VALARM (reminder/alarm) sync | TW has no alarm concept. Alarms are client-specific (tasks.org stores in X-props). | Preserve as extra_props. Do not map. |
| Recurring VTODO sync (RRULE) | Completing a recurring VTODO creates complex split semantics (one COMPLETED instance + one new NEEDS-ACTION). TW recurring tasks have different semantics. Known interop nightmare (tasks.org issue #1261). | Skip with warning (current behavior). Document limitation. |
| DURATION property (alternative to DUE) | RFC 5545 says DUE and DURATION are mutually exclusive. TW has no duration concept. Adding DURATION mapping adds complexity for minimal value. | If a VTODO has DURATION instead of DUE, preserve as extra_prop. Compute DUE from DTSTART+DURATION only if needed (future enhancement). |
| Daemon/scheduler mode | Caldawarrior is a sync binary. Users control invocation via cron, systemd timer, or TW hooks. Building a scheduler adds complexity without value. | Document cron/systemd examples. |
| Real-time push notifications | CalDAV doesn't have push; it's polling-based. WebSocket/XMPP calendar push is non-standard. | Document recommended sync frequency. |
| GUI or web interface | CLI tool; different user demographic. | Keep CLI-only. |
| Multi-user/multi-account sync | Adds authentication complexity, calendar permission model, conflict resolution across users. | Single-account design. Run separate instances for multiple accounts. |
| X-APPLE-SORT-ORDER emission | tasks.org uses this for "My order" sorting (non-standard). caldawarrior should not invent sort orders. | Preserve as extra_prop if received. Do not generate. |
| SEQUENCE property management | RFC says increment on each update. But many servers (OwnCloud, Radicale) don't enforce it, and it adds bookkeeping complexity. | Do not emit SEQUENCE. If received, preserve as extra_prop. |
| VTIMEZONE component embedding | Caldawarrior stores all times as UTC. Embedding VTIMEZONE is only needed for floating time or local time output. All major servers handle UTC. | Always emit UTC (Z suffix). Parse TZID from incoming. Never embed VTIMEZONE. |
| ATTENDEE/ORGANIZER (scheduling) | iTIP scheduling (RFC 6638) is a separate protocol. Task assignment is enterprise-only. | Preserve as extra_props. Do not map. |

## Feature Dependencies

```
ETag-based writes --> Conflict resolution (LWW)
                  --> Loop prevention (stable point)

caldavuid UDA --> Task identity/pairing
             --> Dependency UUID -> CalDAV UID resolution
             --> Orphan detection (deletion propagation)

RELATED-TO parsing --> Dependency resolution
                   --> Cycle detection

Status mapping --> Completed timestamp (COMPLETED property)
              --> Deletion propagation (CANCELLED status)
              --> Recurring task skip

iCal parser (ical.rs) --> All field mapping
                      --> Non-standard property preservation
                      --> TZID handling

TW adapter --> Field clearing (trailing colon)
           --> Task import (modify preservation)
           --> UDA registration
```

## Hardening-Specific Feature Verification Matrix

These are features that are implemented but need verification/testing for production quality.

| Feature Area | What Needs Verification | Priority | Current Test Coverage |
|--------------|------------------------|----------|---------------------|
| DEPENDS-ON E2E with tasks.org | Does tasks.org display RELATED-TO;RELTYPE=DEPENDS-ON? Does it generate them? | HIGH | Unit tests + 1 integration test. No real client test. |
| Field clearing round-trip | When CalDAV removes DUE, does TW.due get cleared? When TW removes priority, does VTODO PRIORITY get removed? | HIGH | ADR documents behavior. No E2E test for clearing. |
| DATE-only DUE values | tasks.org may send DUE;VALUE=DATE:20260315. Does parsing work? Does round-trip preserve? | HIGH | parse_datetime_with_params handles YYYYMMDD. No E2E test. |
| TZID datetime handling | tasks.org sends TZID-qualified dates. Do they convert to UTC correctly? DST edge cases? | MEDIUM | Unit test for America/New_York. No DST transition test. |
| Long DESCRIPTION with special chars | DESCRIPTION with newlines, commas, semicolons, backslashes. Does escaping survive round-trip? | MEDIUM | Unit test for escape_text. No E2E test with real server. |
| Multi-annotation sync | TW tasks with 2+ annotations: only first maps to DESCRIPTION. Is this documented? Are extra annotations preserved? | LOW | Only first annotation mapped (documented in fields.rs). |
| Status transitions E2E | pending->completed->pending, pending->deleted->CANCELLED. All paths. | HIGH | 1 integration test (completed). No CANCELLED E2E. |
| ETag conflict under concurrent access | Two CalDAV clients editing same VTODO during sync. | MEDIUM | 1 integration test. Real concurrency untested. |
| Empty/whitespace SUMMARY | VTODO with no SUMMARY or SUMMARY:. Does "(no title)" sentinel work? | LOW | Unit test. No E2E. |
| Large task count performance | 500+ tasks with dependencies. Does sync complete in reasonable time? | LOW | 100-task integration test exists. |
| PRIORITY=0 semantics | RFC says 0 = undefined. Caldawarrior treats as None. tasks.org may send PRIORITY:0. | MEDIUM | Unit test confirms 0->None. No client compatibility test. |
| extra_props preservation | PUT a VTODO with X-CUSTOM props. Re-GET. Are they preserved? | MEDIUM | Unit test. No E2E with real server. |
| Expired wait collapse | TW wait in past: should drop X-TASKWARRIOR-WAIT from CalDAV. | LOW | Unit test. No E2E. |

## MVP Recommendation (Hardening Milestone)

This is a hardening milestone, not a feature-building one. Prioritize verification over new features.

### Must Verify (blocks "production-quality" claim):
1. **DEPENDS-ON relation E2E** - This is the unique selling point. Must prove it works with real Radicale server, round-trips correctly, and ideally with tasks.org.
2. **All field mapping correctness** - Every mapped field (SUMMARY, DESCRIPTION, STATUS, PRIORITY, DUE, DTSTART, COMPLETED, CATEGORIES, RELATED-TO, X-TASKWARRIOR-WAIT) must have E2E tests covering create, update, and clear operations.
3. **Status transition completeness** - All TW statuses (pending, waiting, completed, deleted, recurring) must be tested E2E with correct CalDAV status mapping.
4. **Deletion propagation** - Both directions: TW delete -> CalDAV CANCELLED, CalDAV delete -> TW orphan handling.
5. **Idempotent sync verification** - Every sync operation must reach stable point on re-run. This is already partially tested but needs comprehensive coverage.

### Should Verify (important for reliability):
6. **DATE-only value handling** - Verify tasks.org date-only DUE values parse and round-trip correctly.
7. **TZID timezone handling** - Test with common timezones; verify DST edge case doesn't corrupt dates.
8. **Error recovery** - Auth failure, network timeout, malformed VTODO. Verify graceful degradation.
9. **Non-standard property preservation** - X-props from other clients survive round-trip.

### Defer (not needed for hardening):
- X-APPLE-SORT-ORDER support
- DURATION computation
- WebDAV-Sync (sync-token) for efficient fetching
- Custom field mapping configuration
- Offline queue

## Sources

- [RFC 5545 - VTODO Component](https://icalendar.org/iCalendar-RFC-5545/3-6-2-to-do-component.html) -- VTODO property cardinality (HIGH confidence)
- [RFC 9253 - iCalendar Relationships](https://datatracker.ietf.org/doc/html/rfc9253) -- DEPENDS-ON, FINISHTOSTART, GAP parameter, LINK property (HIGH confidence)
- [sabre/dav Building a CalDAV Client](https://sabre.io/dav/building-a-caldav-client/) -- Best practices: property preservation, ETag handling, sync strategies (HIGH confidence)
- [tasks.org CalDAV Docs](https://tasks.org/docs/caldav_intro.html) -- Server compatibility, basic setup (MEDIUM confidence)
- [tasks.org Manual Sort Mode](https://tasks.org/docs/manual_sort_mode/) -- X-APPLE-SORT-ORDER usage (MEDIUM confidence)
- [tasks.org subtask sync issues](https://github.com/tasks/tasks/issues/3023) -- RELATED-TO;RELTYPE=PARENT bugs in tasks.org (MEDIUM confidence)
- [tasks.org subtask disappears](https://github.com/tasks/tasks/issues/932) -- Subtask hierarchy breaks on sync (MEDIUM confidence)
- [tasks.org recurring completion bug](https://github.com/tasks/tasks/issues/1261) -- Completing recurring VTODO creates duplicates (MEDIUM confidence)
- [Outlook CalDAV Sync PERCENT-COMPLETE issues](https://sourceforge.net/p/outlookcaldavsynchronizer/tickets/1211/) -- STATUS vs PERCENT-COMPLETE interaction bugs (MEDIUM confidence)
- [opentasks RELATED-TO subtasks](https://github.com/dmfs/opentasks/issues/341) -- Cross-client RELATED-TO;RELTYPE=PARENT implementation differences (MEDIUM confidence)
- [CalConnect Task Introduction](https://devguide.calconnect.org/Tasks/Tasks-Introduction/) -- Task extensions to iCalendar standards overview (HIGH confidence)
- [RFC 5545 SEQUENCE Property](https://icalendar.org/iCalendar-RFC-5545/3-8-7-4-sequence-number.html) -- SEQUENCE increment rules (HIGH confidence)
- [syncall tw-caldav](https://github.com/eigenmannmartin/syncall/blob/master/readme-tw-caldav.md) -- Competing TW-CalDAV sync; no relation support, no waiting task support (MEDIUM confidence)
- Internal: docs/adr/tw-field-clearing.md -- Empirical TW behavior research (HIGH confidence)
- Internal: docs/adr/loop-prevention.md -- Two-layer sync loop prevention design (HIGH confidence)
- Internal: .planning/codebase/CONCERNS.md -- Known codebase issues and gaps (HIGH confidence)
