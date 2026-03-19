# Phase 4: Compatibility - Research

**Researched:** 2026-03-19
**Domain:** iCalendar DATE/TZID parsing, X-property preservation, CalDAV XML edge cases (Radicale)
**Confidence:** HIGH

## Summary

Phase 4 addresses four compatibility requirements focused on making caldawarrior handle real-world CalDAV data without data loss. The scope has been narrowed to Radicale-only for COMPAT-01 (Nextcloud/Baikal deferred to v2), but the other three requirements (DATE-only preservation, TZID/DST handling, X-property round-trip) apply universally to iCalendar parsing and serialization.

The primary implementation challenge is DATE-only preservation (COMPAT-02). Currently, the VTODO struct stores dates as `DateTime<Utc>`, losing the distinction between `DUE;VALUE=DATE:20260315` (date-only) and `DUE:20260315T000000Z` (midnight UTC datetime). The serializer (`format_datetime`) always emits UTC DATE-TIME format. This needs a tracking mechanism so that dates originating as DATE-only from CalDAV are written back as DATE-only. The TZID/DST work (COMPAT-03) is mostly already handled by `parse_datetime_with_params` using chrono-tz, but has a critical gap: ambiguous DST fall-back times return `None` from `.single()`, silently dropping the datetime. X-property preservation (COMPAT-04) is already implemented at the unit-test level but needs E2E verification through real Radicale.

**Primary recommendation:** Add a `date_only_fields` tracking mechanism (e.g., `HashSet<String>` on VTODO or per-field bool) to distinguish DATE-only origins, modify the serializer to conditionally emit `DUE;VALUE=DATE:YYYYMMDD`, handle ambiguous DST times with `.latest()` fallback, and add E2E tests for X-property round-trip and edge-case XML parsing.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- DATE-only round-trip (COMPAT-02): Preserve DATE-only format on CalDAV round-trip; TW-originated tasks always write DATE-TIME; only preserve DATE-only when the date came from CalDAV originally; applies to both DUE and DTSTART
- Timezone/DST handling (COMPAT-03): Rely on chrono-tz only (no VTIMEZONE parsing); always output datetimes in UTC format; TZID is not round-tripped; add unit tests for America/New_York (spring forward + fall back), Europe/Paris (summer + winter), ambiguous times
- X-property preservation (COMPAT-04): E2E round-trip test through real Radicale with X-APPLE-SORT-ORDER, X-OC-HIDESUBTASKS, X-CUSTOM-FOO; verify X-TASKWARRIOR-WAIT coexistence does not disturb other X-properties
- Radicale XML edge cases (COMPAT-01): Radicale only for v1; add targeted edge-case tests: large responses, special characters, empty calendars; use existing Docker infrastructure

### Claude's Discretion
- DATE-only tracking mechanism (how to detect that a date was originally DATE-only for preservation during serialization)
- Specific Radicale XML edge cases to test
- Exact DST test dates and expected UTC conversions
- Unit test vs E2E test split for timezone tests
- X-property fixture data (specific values for test properties)

### Deferred Ideas (OUT OF SCOPE)
- Nextcloud CalDAV compatibility testing (v2, SERV-01)
- Baikal CalDAV compatibility testing (v2, SERV-02)
- VTIMEZONE component parsing
- Synthetic XML fixtures for non-Radicale servers
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| COMPAT-01 | XML parser handles Radicale response formats without data loss | Existing NsReader parser is already namespace-aware; needs edge-case tests for large responses, special chars, empty calendars |
| COMPAT-02 | DATE-only DUE values (YYYYMMDD) parse and round-trip correctly | Requires new tracking mechanism in VTODO struct + conditional serialization; current code parses DATE-only but always serializes as DATE-TIME |
| COMPAT-03 | TZID datetime handling works for common timezones including DST transitions | chrono-tz handles IANA timezones; `.single()` drops ambiguous DST times -- needs `.latest()` or `.earliest()` fallback |
| COMPAT-04 | Non-standard properties (X-props) from other clients survive round-trip sync | Parsing/serialization already works (unit test exists); needs E2E test through real Radicale + coexistence test with X-TASKWARRIOR-WAIT |
</phase_requirements>

## Standard Stack

### Core (already in Cargo.toml)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| chrono | 0.4.44 | Date/time types, UTC conversion | Already used throughout; `DateTime<Utc>`, `NaiveDate`, `NaiveDateTime` |
| chrono-tz | 0.10.4 | IANA timezone database (TZID resolution) | Already used for TZID parsing in `parse_datetime_with_params` |
| quick-xml | 0.39.2 | Namespace-aware XML parsing (NsReader) | Already used for CalDAV REPORT response parsing |

### Supporting (already in Cargo.toml)
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| serde/serde_json | 1.x | JSON serialization for TW tasks | Existing; VTODO struct derives Serialize/Deserialize |

### No New Dependencies Needed

All Phase 4 work is achievable with the existing dependency set. No new crates are required.

## Architecture Patterns

### Current Date Flow (the problem)

```
CalDAV iCal text                  VTODO struct              iCal text output
  DUE;VALUE=DATE:20260315  -->  due: DateTime<Utc>  -->  DUE:20260315T000000Z
  (DATE-only)                   (midnight UTC)            (DATE-TIME -- WRONG)
```

### Recommended DATE-Only Tracking Pattern

**Recommendation (Claude's discretion area):** Add per-field boolean flags to VTODO struct to track DATE-only origin.

```rust
// In src/types.rs - add to VTODO struct:
#[serde(default)]
pub due_is_date_only: bool,
#[serde(default)]
pub dtstart_is_date_only: bool,
```

**Why per-field booleans over alternatives:**
- A `HashSet<String>` of field names adds indirection and allocation for what is always 0-2 items
- Per-field booleans are zero-cost when false (serde skip_serializing_if), explicit, and impossible to mis-key
- The VTODO struct already has per-field Option types, so this follows the existing pattern
- Only DUE and DTSTART can be DATE-only per RFC 5545 (COMPLETED, LAST-MODIFIED, DTSTAMP are always DATE-TIME)

### Required Changes by File

```
src/types.rs
  └── VTODO: add due_is_date_only, dtstart_is_date_only bools

src/ical.rs (parsing)
  └── from_icalendar_string(): detect VALUE=DATE param, set _is_date_only flags
  └── parse_datetime_with_params(): already handles DATE-only parsing (no change needed)

src/ical.rs (serialization)
  └── to_icalendar_string(): check _is_date_only flags, emit DUE;VALUE=DATE:YYYYMMDD or DUE:YYYYMMDDTHHMMSSZ
  └── New helper: format_date_only(dt: DateTime<Utc>) -> String  (YYYYMMDD format)

src/ical.rs (DST fix)
  └── parse_datetime_with_params(): replace .single()? with .latest() fallback for ambiguous times

src/sync/writeback.rs
  └── build_vtodo_from_tw(): propagate _is_date_only from fetched_vtodo to rebuilt VTODO
  └── TW-originated tasks: always set _is_date_only = false

src/mapper/fields.rs
  └── No changes needed -- field mapper deals with DateTime<Utc>, not format metadata

tests/robot/resources/CalDAVLibrary.py
  └── New keyword: put_vtodo_raw_ical(collection_url, uid, ical_text) -- PUT arbitrary iCal content
  └── This enables E2E tests with DATE-only values, X-properties, TZID params

tests/robot/suites/09_compatibility.robot (new)
  └── DATE-only round-trip E2E tests
  └── X-property preservation E2E tests
  └── Large response / special character / empty calendar edge cases
```

### Anti-Patterns to Avoid
- **Storing format metadata in extra_props:** The `_is_date_only` flag must be part of the struct, not smuggled as a synthetic IcalProp, because extra_props are round-tripped to the server and this is internal metadata.
- **Parsing VALUE=DATE in the serializer:** The serializer should not inspect the DateTime value to guess if it was date-only (e.g., checking if time is midnight) -- that would incorrectly flag midnight-UTC datetimes as date-only.
- **Round-tripping TZID:** Per locked decisions, TZID is not round-tripped. Always output UTC. Do not store the original TZID for re-emission.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| IANA timezone resolution | Custom TZ offset tables | chrono-tz `Tz::parse()` | Covers all IANA zones, handles DST rules, updated with tz database |
| DST transition rules | Manual spring-forward/fall-back date tables | chrono-tz `from_local_datetime()` | Rules change by legislation; the tz database tracks these |
| iCal line folding/unfolding | Custom string splitting | Existing `fold_line()`/`unfold_lines()` in ical.rs | Already correct and tested |
| XML namespace resolution | Regex or string matching | quick-xml NsReader | Already handles arbitrary prefixes (D:, ns0:, bare xmlns) |

**Key insight:** The iCalendar format is deceptively simple -- the hard parts (DST rules, line folding, comma escaping, parameter parsing) are already solved in the codebase. Phase 4 is mostly about adding tracking metadata and edge-case tests, not rewriting parsers.

## Common Pitfalls

### Pitfall 1: Ambiguous DST Times Silently Dropped
**What goes wrong:** `parse_datetime_with_params` calls `.single()` on the `MappedLocalTime` result. For fall-back DST transitions (e.g., 2026-11-01 01:30 America/New_York), the local time maps to two UTC instants. `.single()` returns `None`, and the entire datetime is silently dropped.
**Why it happens:** `.single()` is the conservative choice -- it only succeeds for unambiguous mappings. But for calendar data, silently losing a date is worse than picking one interpretation.
**How to avoid:** Use `.latest()` as the fallback. This picks the second (standard-time) occurrence during fall-back, which is the more intuitive interpretation. Chain: `.single().or_else(|| result.latest())`.
**Warning signs:** Any VTODO with a TZID datetime during a DST fall-back hour will have its date field parsed as `None`. This would manifest as missing DUE/DTSTART on synced tasks.

### Pitfall 2: DATE-Only Flag Not Propagated Through TW-to-CalDAV Path
**What goes wrong:** When caldawarrior updates a CalDAV VTODO from TW data, `build_vtodo_from_tw` rebuilds the VTODO from scratch. If it does not copy `_is_date_only` from the existing fetched VTODO, the date reverts to DATE-TIME format.
**Why it happens:** The rebuild pattern in `build_vtodo_from_tw` constructs a new VTODO struct with fields from TW. The `_is_date_only` metadata exists only on the CalDAV-fetched VTODO, not on the TW task.
**How to avoid:** In `build_vtodo_from_tw`, when `entry.fetched_vtodo` exists, copy the `_is_date_only` flags from the fetched VTODO to the new VTODO. When there is no existing VTODO (new TW-originated task), always set flags to `false`.
**Warning signs:** A CalDAV VTODO that originally had `DUE;VALUE=DATE:20260315` will change to `DUE:20260315T000000Z` after one sync cycle.

### Pitfall 3: VTODO Struct Changes Break Existing Tests
**What goes wrong:** Adding `due_is_date_only` and `dtstart_is_date_only` fields to the VTODO struct requires updating every test that constructs a VTODO directly (e.g., `make_basic_vtodo()` in ical.rs tests).
**Why it happens:** Rust structs with named fields require all fields at construction.
**How to avoid:** Use `#[serde(default)]` and set `..Default::default()` or add them with default `false` to existing test helper functions. Consider deriving Default on VTODO if not already done.
**Warning signs:** Compiler errors in test files after adding struct fields.

### Pitfall 4: CalDAVLibrary.py put_vtodo_with_fields Cannot Handle DATE-Only or X-Props
**What goes wrong:** The existing `put_vtodo_with_fields` keyword builds iCal content with hardcoded line patterns. It cannot produce `DUE;VALUE=DATE:YYYYMMDD` or arbitrary X-properties.
**Why it happens:** The keyword was designed for Phase 3 field mapping tests, not compatibility edge cases.
**How to avoid:** Add a new `put_vtodo_raw_ical` keyword that accepts complete iCal text, enabling tests to PUT exact iCal content with VALUE=DATE parameters, TZID parameters, and arbitrary X-properties.
**Warning signs:** Tests cannot create the input VTODO formats needed for verification.

### Pitfall 5: Spring-Forward Gap Times
**What goes wrong:** A VTODO with `DTSTART;TZID=America/New_York:20260308T020000` (2:00 AM on spring-forward day) specifies a local time that does not exist. `from_local_datetime` returns `MappedLocalTime::None`, and `.single()` returns `None`, silently dropping the datetime.
**Why it happens:** Clocks jump from 1:59 AM to 3:00 AM; 2:00 AM never exists.
**How to avoid:** After `.single()` fails, try `.latest()` (returns `None` for gaps). For true gap times, fall back to a sensible interpretation: parse as if the clock had not sprung forward (i.e., treat as the post-transition time). The simplest approach: if `.single()` returns `None` and `.latest()` also returns `None`, re-parse the naive datetime as UTC. This matches the "floating time treated as UTC" fallback already in the code. Alternatively, use `.earliest()` which for a gap returns the first valid moment after the gap.
**Warning signs:** VTODOs with gap-time TZID datetimes lose their date fields entirely.

## Code Examples

Verified patterns from the existing codebase:

### DATE-Only Detection in Parser (New Code)
```rust
// In from_icalendar_string(), inside the DUE match arm:
"DUE" => {
    due = parse_datetime_with_params(&value, &params);
    // Check for VALUE=DATE parameter (RFC 5545 Section 3.2.20)
    due_is_date_only = params.iter().any(|(k, v)| {
        k.eq_ignore_ascii_case("VALUE") && v.eq_ignore_ascii_case("DATE")
    }) || (value.len() == 8 && !value.contains('T'));
    // Second condition: implicit date-only (no VALUE param but YYYYMMDD format)
}
```

### Conditional DATE-Only Serialization (New Code)
```rust
// In to_icalendar_string():
if let Some(due) = vtodo.due {
    if vtodo.due_is_date_only {
        lines.push(format!("DUE;VALUE=DATE:{}", due.format("%Y%m%d")));
    } else {
        lines.push(format!("DUE:{}", format_datetime(due)));
    }
}
```

### DST Ambiguity Fix (Modified Code)
```rust
// In parse_datetime_with_params(), replace the TZID branch's last two lines:
// OLD:
//   let dt = tz.from_local_datetime(&naive).single()?;
//   return Some(dt.to_utc());
// NEW:
let mapped = tz.from_local_datetime(&naive);
let dt = mapped.single()
    .or_else(|| mapped.latest())   // fall-back: pick standard-time occurrence
    .or_else(|| mapped.earliest()) // gap: pick first valid moment after gap
    ?;
return Some(dt.to_utc());
```

### RF E2E: put_vtodo_raw_ical Keyword (New CalDAVLibrary.py method)
```python
def put_vtodo_raw_ical(self, collection_url, uid, ical_text):
    """PUT raw iCalendar text directly, for compatibility edge-case tests."""
    url = f"{collection_url}{uid}.ics"
    response = self._session.put(
        url,
        data=ical_text.encode('utf-8'),
        headers={'Content-Type': 'text/calendar; charset=utf-8'},
    )
    self._check_response(response)
```

### DST Unit Test: America/New_York Fall-Back
```rust
#[test]
fn test_tzid_fall_back_ambiguous() {
    // 2026-11-01 01:30 ET is ambiguous: could be EDT (UTC-4) or EST (UTC-5).
    // We expect the latest interpretation (EST = UTC-5), so 01:30 EST = 06:30 UTC.
    let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\n\
        UID:dst-fall-001\r\n\
        DUE;TZID=America/New_York:20261101T013000\r\n\
        END:VTODO\r\nEND:VCALENDAR\r\n";
    let vtodo = from_icalendar_string(ical).expect("should parse ambiguous DST time");
    let due = vtodo.due.expect("DUE should not be None for ambiguous time");
    assert_eq!(due.hour(), 6); // 01:30 EST = 06:30 UTC
    assert_eq!(due.minute(), 30);
}
```

### DST Unit Test: America/New_York Spring-Forward Gap
```rust
#[test]
fn test_tzid_spring_forward_gap() {
    // 2026-03-08 02:30 ET does not exist (clocks jump from 1:59 to 3:00 EDT).
    // We expect earliest valid time after gap: 03:00 EDT = 07:00 UTC.
    let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\n\
        UID:dst-spring-001\r\n\
        DUE;TZID=America/New_York:20260308T023000\r\n\
        END:VTODO\r\nEND:VCALENDAR\r\n";
    let vtodo = from_icalendar_string(ical).expect("should parse gap DST time");
    let due = vtodo.due.expect("DUE should not be None for gap time");
    // 03:00 EDT = 07:00 UTC (earliest valid moment after the gap)
    assert_eq!(due.hour(), 7);
    assert_eq!(due.minute(), 0);
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| DATE-only parsed but always serialized as DATE-TIME | Need conditional serialization based on origin | This phase | Prevents spurious time components on CalDAV clients |
| `.single()` only for TZID resolution | Need `.single()` with `.latest()` / `.earliest()` fallback | This phase | Prevents silent datetime loss during DST transitions |
| X-property preservation tested only at unit level | Need E2E test through real Radicale | This phase | Validates full round-trip including server-side storage |

**Already correct:**
- XML parser (quick-xml NsReader) is namespace-aware -- handles D:, ns0:, bare xmlns
- extra_props parsing preserves arbitrary properties including params
- extra_props serialization includes params
- X-TASKWARRIOR-WAIT filtering does not disturb other X-props

## Open Questions

1. **MappedLocalTime::earliest() behavior for gaps**
   - What we know: `MappedLocalTime::Ambiguous(early, late)` is for fall-back; `.latest()` returns `late`. For spring-forward gaps, the result is `MappedLocalTime::None` -- `.latest()` returns `None`.
   - What's unclear: Does chrono 0.4.44's `MappedLocalTime` have `.earliest()` that returns the first valid moment after a gap? The docs say `None` for gaps returns `None` from both `.earliest()` and `.latest()`.
   - Recommendation: For gap times, after `.single()` and `.latest()` both return `None`, fall back to treating the naive datetime as UTC (matching the existing floating-time fallback). Add a unit test confirming this behavior. This is acceptable because gap times in real-world iCal data are rare and the "treat as UTC" interpretation is reasonable.

2. **VTODO Default derive**
   - What we know: VTODO currently does not derive `Default`.
   - What's unclear: Whether adding `#[derive(Default)]` would break any existing code or test patterns.
   - Recommendation: Add `Default` derive to VTODO (all fields are Option or Vec, which have natural defaults). This simplifies test construction with struct update syntax (`VTODO { uid: "..".into(), ..Default::default() }`).

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust unit + integration) + Robot Framework (E2E) |
| Config file | `Cargo.toml` (test harness) + `tests/robot/docker-compose.yml` (RF) |
| Quick run command | `cargo test` |
| Full suite command | `cargo test && CURRENT_UID=$(id -u) CURRENT_GID=$(id -g) docker compose -f tests/robot/docker-compose.yml run --rm robot` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| COMPAT-01 | Large Radicale response (many VTODOs) parsed | unit | `cargo test caldav_adapter::tests::test_parse_multistatus_large -x` | Wave 0 |
| COMPAT-01 | Special characters in VTODO content parsed | unit | `cargo test caldav_adapter::tests::test_parse_multistatus_special_chars -x` | Wave 0 |
| COMPAT-01 | Empty calendar response parsed (0 VTODOs) | unit | `cargo test caldav_adapter::tests::test_parse_multistatus_empty -x` | Wave 0 |
| COMPAT-02 | DATE-only DUE parsed and flag set | unit | `cargo test ical::tests::test_date_only_due_parsed -x` | Wave 0 |
| COMPAT-02 | DATE-only DUE serialized as VALUE=DATE | unit | `cargo test ical::tests::test_date_only_due_serialized -x` | Wave 0 |
| COMPAT-02 | DATE-only DUE round-trip via Radicale | E2E | RF suite 09_compatibility.robot | Wave 0 |
| COMPAT-02 | TW-originated task serializes DUE as DATE-TIME | unit | `cargo test ical::tests::test_tw_originated_due_datetime -x` | Wave 0 |
| COMPAT-03 | America/New_York EST (winter) TZID parsed | unit | existing `ical::tests::test_tzid_conversion` | Exists |
| COMPAT-03 | America/New_York spring-forward gap handled | unit | `cargo test ical::tests::test_tzid_spring_forward_gap -x` | Wave 0 |
| COMPAT-03 | America/New_York fall-back ambiguous handled | unit | `cargo test ical::tests::test_tzid_fall_back_ambiguous -x` | Wave 0 |
| COMPAT-03 | Europe/Paris summer (UTC+2) TZID parsed | unit | `cargo test ical::tests::test_tzid_paris_summer -x` | Wave 0 |
| COMPAT-03 | Europe/Paris winter (UTC+1) TZID parsed | unit | `cargo test ical::tests::test_tzid_paris_winter -x` | Wave 0 |
| COMPAT-04 | X-properties survive Radicale round-trip | E2E | RF suite 09_compatibility.robot | Wave 0 |
| COMPAT-04 | X-TASKWARRIOR-WAIT coexists with other X-props | E2E | RF suite 09_compatibility.robot | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test`
- **Per wave merge:** `cargo test && CURRENT_UID=$(id -u) CURRENT_GID=$(id -g) docker compose -f tests/robot/docker-compose.yml run --rm robot`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `tests/robot/suites/09_compatibility.robot` -- DATE-only E2E, X-property E2E, edge-case tests
- [ ] New CalDAVLibrary.py keyword `put_vtodo_raw_ical` -- enables custom iCal content in E2E tests
- [ ] Unit tests for COMPAT-01 edge cases (large, special chars, empty) in `src/caldav_adapter.rs`
- [ ] Unit tests for COMPAT-02 DATE-only parsing/serialization in `src/ical.rs`
- [ ] Unit tests for COMPAT-03 DST edge cases (spring-forward, fall-back, Paris) in `src/ical.rs`

## Sources

### Primary (HIGH confidence)
- Codebase: `src/ical.rs` lines 426-479 -- parse_datetime_with_params, format_datetime (verified by reading source)
- Codebase: `src/types.rs` lines 214-246 -- VTODO and IcalProp struct definitions (verified by reading source)
- Codebase: `src/sync/writeback.rs` lines 82-141 -- build_vtodo_from_tw extra_props handling (verified by reading source)
- Codebase: `src/caldav_adapter.rs` lines 340-469 -- parse_multistatus_vtodos NsReader parser (verified by reading source)
- Codebase: `tests/robot/resources/CalDAVLibrary.py` -- all RF CalDAV keywords (verified by reading source)
- [iCalendar.org - DUE property specification](https://icalendar.org/iCalendar-RFC-5545/3-8-2-3-date-time-due.html) -- VALUE=DATE option for DUE
- [RFC 5545](https://datatracker.ietf.org/doc/html/rfc5545) -- iCalendar core specification
- [chrono MappedLocalTime docs](https://docs.rs/chrono/latest/chrono/offset/type.MappedLocalTime.html) -- Single/Ambiguous/None variants, .single()/.latest()/.earliest() behavior

### Secondary (MEDIUM confidence)
- [chrono-tz GitHub](https://github.com/chronotope/chrono-tz) -- IANA timezone database coverage
- [chrono issue #1153](https://github.com/chronotope/chrono/issues/1153) -- ambiguous TZ string handling discussion

### Tertiary (LOW confidence)
- MappedLocalTime `.earliest()` for gap times: based on enum documentation stating None variant means `.earliest()` and `.latest()` both return None for gaps. Needs unit test validation.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all dependencies already in Cargo.toml, versions verified via `cargo tree`
- Architecture (DATE-only tracking): HIGH - clear from reading VTODO struct and serialization code; per-field bool is the simplest correct approach
- Architecture (DST fix): HIGH - `.single()` behavior documented; `.latest()` fallback is standard chrono pattern
- Pitfalls: HIGH - all identified from direct source code analysis, not speculation
- Validation: HIGH - existing test infrastructure thoroughly reviewed; RF keywords and Docker setup confirmed working

**Research date:** 2026-03-19
**Valid until:** 2026-04-19 (stable domain -- RFC 5545 and chrono API are mature)
