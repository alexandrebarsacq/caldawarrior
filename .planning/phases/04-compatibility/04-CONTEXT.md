# Phase 4: Compatibility - Context

**Gathered:** 2026-03-19
**Status:** Ready for planning

<domain>
## Phase Boundary

Verify caldawarrior handles real-world data formats without data loss: DATE-only values, timezone/DST handling, and non-standard property preservation. **Scoped to Radicale only for v1** — Nextcloud/Baikal compatibility deferred to v2. Includes edge-case XML parsing tests for Radicale.

</domain>

<decisions>
## Implementation Decisions

### DATE-only round-trip (COMPAT-02)
- Preserve DATE-only format on CalDAV round-trip: if the original VTODO had `DUE;VALUE=DATE:YYYYMMDD`, write it back as DATE-only (not DATE-TIME)
- Applies to both DUE and DTSTART properties
- TW-originated tasks always write DATE-TIME to CalDAV (TW stores full timestamps internally)
- Only preserve DATE-only when the date came from CalDAV originally
- TW side always stores full datetime (due:YYYY-MM-DDTHH:MM:SSZ) regardless of CalDAV format

### Timezone/DST handling (COMPAT-03)
- Rely on chrono-tz only — no VTIMEZONE component parsing
- chrono-tz covers all IANA timezones including DST rules
- Always output datetimes in UTC format (YYYYMMDDTHHMMSSZ) — TZID is not round-tripped
- Add unit tests for common timezones with DST transitions:
  - America/New_York: spring forward and fall back
  - Europe/Paris: summer (UTC+2) and winter (UTC+1)
  - Ambiguous times during DST transitions

### X-property preservation (COMPAT-04)
- E2E round-trip test through real Radicale: VTODO with X-APPLE-SORT-ORDER, X-OC-HIDESUBTASKS, X-CUSTOM-FOO
- Verify all X-properties survive a full sync cycle (PUT → sync → sync → GET)
- Include test variant with X-TASKWARRIOR-WAIT coexisting alongside other X-properties — verify caldawarrior manages its own X-prop without disturbing others

### Radicale XML edge cases (COMPAT-01)
- Radicale only for v1 — no Nextcloud/Baikal fixtures
- Add targeted edge-case tests beyond existing Phase 1-3 coverage: large responses (many VTODOs), special characters in VTODO content, empty calendars
- Use existing Radicale Docker infrastructure (no new containers)

### Claude's Discretion
- DATE-only tracking mechanism (how to detect that a date was originally DATE-only for preservation during serialization)
- Specific Radicale XML edge cases to test
- Exact DST test dates and expected UTC conversions
- Unit test vs E2E test split for timezone tests
- X-property fixture data (specific values for test properties)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` — COMPAT-01 through COMPAT-04 (note: scope narrowed to Radicale-only for COMPAT-01)
- `.planning/ROADMAP.md` — Phase 4 success criteria (4 conditions, adjusted for Radicale-only scope)

### Core implementation (where changes will land)
- `src/ical.rs` lines 431-479 — Date/time parsing (`parse_datetime_with_params`), DATE-only handling at lines 466-470, TZID resolution at lines 442-456
- `src/ical.rs` lines 427-429 — Date serialization (currently always UTC DATE-TIME, needs DATE-only support)
- `src/ical.rs` lines 99-106 — Extra property parsing (IcalProp with params)
- `src/ical.rs` lines 190-201 — Extra property serialization
- `src/sync/writeback.rs` lines 101-114 — extra_props preservation logic, X-TASKWARRIOR-WAIT filtering
- `src/caldav_adapter.rs` lines 340-469 — XML parsing with NsReader (already namespace-aware)

### Existing test coverage (extend, don't duplicate)
- `src/caldav_adapter.rs` lines 624-809 — XML parsing unit tests (Radicale format, custom namespaces, CDATA, malformed skipping)
- `src/ical.rs` line 614 — TZID conversion test (America/New_York)
- `src/ical.rs` line 657 — X-property round-trip unit test
- `tests/robot/` — Existing RF E2E infrastructure with real Radicale Docker

### Prior phase context
- `.planning/phases/01-code-audit-and-bug-fixes/01-CONTEXT.md` — Test philosophy: spec-oriented, E2E mandatory, Radicale-only for Phase 1 with Nextcloud/Baikal deferred to Phase 4 (now further deferred to v2)
- `.planning/phases/03-field-and-sync-correctness/03-CONTEXT.md` — Field clear semantics, X-TASKWARRIOR-WAIT handling

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `parse_datetime_with_params()` in `src/ical.rs:433` — Already handles TZID and DATE-only parsing; needs modification to track DATE-only origin
- `IcalProp` struct with `params: Vec<(String, String)>` — Can carry `VALUE=DATE` parameter for DATE-only tracking
- `extra_props: Vec<IcalProp>` in VTODO struct — Already preserves non-standard properties through sync
- `parse_multistatus_vtodos()` in `src/caldav_adapter.rs:350` — NsReader-based XML parser, already handles namespace variations
- Radicale Docker setup in `tests/robot/docker-compose.yml` — Ready for new E2E tests
- `CalDAV.Get VTODO Raw` RF keyword — Gets raw iCal for property inspection in E2E tests

### Established Patterns
- Unit tests in same file (`#[cfg(test)]` modules) — DST tests go in ical.rs
- RF E2E test structure in `tests/robot/suites/` — X-property round-trip test follows existing patterns
- `skip-unimplemented` RF tag for tests needing code changes
- Round-trip test pattern: create → sync → verify → sync → verify-no-change

### Integration Points
- `src/ical.rs` serialization functions — Need DATE-only output path alongside existing DATE-TIME
- `src/mapper/fields.rs` — Field mapping may need awareness of DATE-only for proper TW↔CalDAV transformation
- `src/sync/writeback.rs` build_vtodo_from_tw — Where date format decision happens during CalDAV write

</code_context>

<specifics>
## Specific Ideas

- DATE-only preservation is the key correctness concern — clients like tasks.org may show "midnight" instead of "all day" if a spurious time component is added
- Radicale-only scope for v1 is intentional — no point testing servers we can't run in Docker CI
- chrono-tz is pragmatically sufficient for IANA timezones; VTIMEZONE parsing would be overengineering for the VTODO use case
- The X-TASKWARRIOR-WAIT coexistence test verifies that caldawarrior's property management doesn't have side effects on other clients' X-properties

</specifics>

<deferred>
## Deferred Ideas

- Nextcloud CalDAV compatibility testing — v2 (SERV-01 in REQUIREMENTS.md)
- Baikal CalDAV compatibility testing — v2 (SERV-02 in REQUIREMENTS.md)
- VTIMEZONE component parsing — not needed for IANA timezones, reconsider if proprietary TZID issues arise
- Synthetic XML fixtures for non-Radicale servers — deferred with the server testing itself

</deferred>

---

*Phase: 04-compatibility*
*Context gathered: 2026-03-19*
