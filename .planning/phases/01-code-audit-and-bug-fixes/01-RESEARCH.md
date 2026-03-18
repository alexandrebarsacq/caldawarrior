# Phase 1: Code Audit and Bug Fixes - Research

**Researched:** 2026-03-18
**Domain:** iCalendar RFC 5545 compliance, XML parsing, HTTP ETag handling, Rust error propagation
**Confidence:** HIGH

## Summary

Phase 1 addresses four confirmed bugs in the caldawarrior sync engine that must be fixed before any test expansion. Each bug is isolated to a specific subsystem, with clear root causes identified through code analysis.

**AUDIT-01 (CATEGORIES comma-escaping):** The parser at `src/ical.rs:68-75` splits CATEGORIES on raw commas without unescaping first, so a tag like "Smith, John" becomes two separate tags. The serializer at `src/ical.rs:169-171` joins categories with raw commas without escaping commas inside individual tag values. Both `escape_text()` and `unescape_text()` already exist in the same file and correctly handle RFC 5545 TEXT escaping -- they just are not applied to CATEGORIES values. Additionally, the TW tags-to-CATEGORIES mapping is incomplete: `build_vtodo_from_tw()` copies categories from the existing CalDAV VTODO (`base.map(|v| v.categories.clone()).unwrap_or_default()`) instead of mapping from `tw.tags`.

**AUDIT-02 (XML parser namespace handling):** The custom string-based XML parser in `src/caldav_adapter.rs:312-420` hardcodes exactly two namespace prefixes ("D:" and "C:") plus bare (no prefix). Servers like Nextcloud/Baikal may use arbitrary prefixes (e.g., "d:", "ns0:", or default xmlns). The fix is to replace this hand-rolled parser with `quick-xml` (v0.39.2), which provides `NsReader` for proper namespace-aware parsing.

**AUDIT-03 (Error context swallowing):** There are ~10 `unwrap_or_default()` calls in `src/caldav_adapter.rs` (lines 98, 114, 148, 155, 200, 229) and `src/sync/writeback.rs` that silently produce empty strings when HTTP response bodies fail to read. The error type `CaldaWarriorError` already has rich context variants -- the issue is that callers swallow errors instead of propagating them. Additionally, the ETag retry exhaustion error at `writeback.rs:477-480` only includes the CalDAV UID but not the task UUID, field, or server URL.

**AUDIT-04 (Weak ETag normalization):** The ETag extraction at `src/caldav_adapter.rs:93-97` stores ETags as-is from the server. The `put_vtodo` method at line 176 wraps the ETag in double quotes for the `If-Match` header. Weak ETags (`W/"abc"`) need the `W/` prefix stripped before use in `If-Match` because RFC 7232 requires strong comparison for `If-Match`. Currently, a weak ETag `W/"abc"` would be sent as `If-Match: "W/"abc""` (malformed), causing 412 Precondition Failed.

**Primary recommendation:** Fix all four bugs in order of risk (AUDIT-02 XML parser first as highest-risk, then AUDIT-01, AUDIT-04, AUDIT-03), with regression tests for each fix.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **Error reporting behavior**: Verbose by default with full context (task UUID, field name, CalDAV href, actual values). Errors/warnings to stderr, sync progress to stdout. Keep direct eprintln!/println! -- no logging framework.
- **Failure mode policy**: Default skip-and-continue; add --fail-fast flag; non-zero exit code on any failure; XML parser parse-what-you-can with warnings.
- **Corrupted data handling**: No special handling needed -- no existing users, just fix the bugs cleanly.
- **Regression testing**: Each bug fix ships with regression tests. Tests must be spec-oriented (verify behavior, not implementation). Include E2E tests (Robot Framework with real TW+Radicale). Use real Radicale server response data for XML parser fixtures. Radicale only for Phase 1.

### Claude's Discretion
- XML library choice for parser replacement
- Specific ETag normalization approach (strip W/ prefix vs. skip conditional write)
- Internal error type refactoring to support full-context propagation
- Exact --fail-fast flag implementation (CLI arg parsing approach)

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| AUDIT-01 | CATEGORIES comma-escaping bug fixed -- tags containing commas no longer silently split | RFC 5545 TEXT escaping rules confirmed; existing `escape_text()`/`unescape_text()` functions identified; parsing fix at ical.rs:68-75 and serialization fix at ical.rs:169-171; TW tags mapping gap identified in writeback.rs:128 |
| AUDIT-02 | XML parser replaced with proper XML library -- CalDAV responses from non-Radicale servers parse correctly | `quick-xml` v0.39.2 identified as the standard Rust XML library with `NsReader` for namespace-aware parsing; current hand-rolled parser at caldav_adapter.rs:312-420 hardcodes D:/C: prefixes |
| AUDIT-03 | Error messages improved -- no swallowed context from unwrap_or_default paths | 10 `unwrap_or_default()` sites catalogued; `CaldaWarriorError` already has context variants; error propagation pattern identified; --fail-fast requires clap modification (already using derive API) |
| AUDIT-04 | ETag normalization handles weak ETags -- no 412 loops on Nextcloud/Baikal | RFC 7232 If-Match requires strong comparison; weak ETags must strip W/ prefix; ETag extraction at caldav_adapter.rs:93-97 needs normalization function |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| quick-xml | 0.39.2 | XML parsing with namespace support | De facto Rust XML library; NsReader handles arbitrary namespace prefixes; streaming API keeps memory low |
| thiserror | 2 | Error derive macro | Already in Cargo.toml; provides `#[error]` derive for CaldaWarriorError |
| clap | 4 | CLI argument parsing | Already in Cargo.toml with derive feature; add --fail-fast flag |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| chrono | 0.4 | Date/time handling | Already in Cargo.toml; used throughout |
| reqwest | 0.12 | HTTP client | Already in Cargo.toml; CalDAV requests |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| quick-xml | fast-dav-rs | Full CalDAV client; much heavier dependency, forces async, overkill for just fixing XML parsing |
| quick-xml | webdav-xml | WebDAV-specific XML types; too opinionated, would require restructuring adapter layer |
| quick-xml NsReader | quick-xml Reader (no namespace) | Simpler but still namespace-blind; defeats the purpose of AUDIT-02 |

**Installation:**
```bash
cargo add quick-xml@0.39
```

**Version verification:** `quick-xml` 0.39.2 confirmed via `cargo search quick-xml` on 2026-03-18.

## Architecture Patterns

### Affected File Structure
```
src/
  ical.rs              # AUDIT-01: CATEGORIES escaping fix (parse + serialize)
  caldav_adapter.rs    # AUDIT-02: XML parser replacement; AUDIT-04: ETag normalization
  error.rs             # AUDIT-03: May need additional error variants
  sync/
    writeback.rs       # AUDIT-01: TW tags->CATEGORIES mapping; AUDIT-03: error context; AUDIT-04: ETag usage
  main.rs              # AUDIT-03: --fail-fast flag addition
  output.rs            # AUDIT-03: Error formatting (already handles [ERROR] prefix)
```

### Pattern 1: RFC 5545 CATEGORIES Escaping (AUDIT-01)

**What:** CATEGORIES is a comma-separated list of TEXT values. Each individual category value must have commas escaped per RFC 5545 section 3.3.11 (TEXT type). The comma separator is a raw comma; commas inside category names are escaped as `\,`.

**When to use:** Parsing and serializing CATEGORIES property.

**Parse fix (ical.rs:68-75):**
```rust
// BEFORE (buggy):
"CATEGORIES" => {
    for cat in value.split(',') {
        let cat = cat.trim().to_string();
        if !cat.is_empty() { categories.push(cat); }
    }
}

// AFTER (correct):
"CATEGORIES" => {
    // Split on unescaped commas only, then unescape each value
    for cat in split_on_unescaped_commas(&value) {
        let cat = unescape_text(cat.trim());
        if !cat.is_empty() { categories.push(cat); }
    }
}
```

**Serialize fix (ical.rs:169-171):**
```rust
// BEFORE (buggy):
if !vtodo.categories.is_empty() {
    lines.push(format!("CATEGORIES:{}", vtodo.categories.join(",")));
}

// AFTER (correct):
if !vtodo.categories.is_empty() {
    let escaped: Vec<String> = vtodo.categories.iter().map(|c| escape_text(c)).collect();
    lines.push(format!("CATEGORIES:{}", escaped.join(",")));
}
```

**New helper needed:**
```rust
/// Split a string on unescaped commas (commas not preceded by backslash).
fn split_on_unescaped_commas(s: &str) -> Vec<&str> { ... }
```

**TW tags mapping fix (writeback.rs:128):**
```rust
// BEFORE (buggy -- uses CalDAV categories, ignoring TW tags):
categories: base.map(|v| v.categories.clone()).unwrap_or_default(),

// AFTER (correct -- maps TW tags to categories):
categories: tw.tags.clone().unwrap_or_default(),
```

### Pattern 2: Namespace-Aware XML Parsing (AUDIT-02)

**What:** Replace the hand-rolled string-matching XML parser with quick-xml's `NsReader` to handle arbitrary namespace prefixes.

**When to use:** Parsing CalDAV multistatus REPORT responses.

**Approach:**
```rust
use quick_xml::events::Event;
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;

const DAV_NS: &[u8] = b"DAV:";
const CALDAV_NS: &[u8] = b"urn:ietf:params:xml:ns:caldav";

fn parse_multistatus_vtodos(xml: &str) -> Vec<FetchedVTODO> {
    let mut reader = NsReader::from_str(xml);
    reader.config_mut().trim_text(true);
    // Use namespace-resolved events to match elements regardless of prefix
    // Match on (namespace_uri, local_name) tuples, not prefixed tag names
}
```

**Key design decisions:**
- Use `NsReader::from_str()` (synchronous, no async needed)
- Match on namespace URI + local name, never on prefix
- DAV namespace: `DAV:` for `response`, `href`, `getetag`, `propstat`, `prop`, `status`
- CalDAV namespace: `urn:ietf:params:xml:ns:caldav` for `calendar-data`
- Parse-what-you-can: skip unparseable entries with warnings, do not abort

### Pattern 3: ETag Normalization (AUDIT-04)

**What:** Strip the `W/` weak indicator prefix from ETags before storage and use in `If-Match` headers. RFC 7232 section 2.3.2 specifies that `If-Match` uses strong comparison, and weak ETags cannot satisfy strong comparison.

**Approach -- strip W/ prefix on extraction:**
```rust
/// Normalize an ETag value: strip W/ weak prefix and ensure double-quote wrapping.
/// This converts weak ETags to strong for use in If-Match headers.
fn normalize_etag(raw: &str) -> String {
    let s = raw.trim();
    // Strip weak indicator
    let s = if s.starts_with("W/") || s.starts_with("w/") {
        &s[2..]
    } else {
        s
    };
    // Ensure double-quote wrapping
    let s = s.trim_matches('"');
    format!("\"{}\"", s)
}
```

**Apply at extraction point (caldav_adapter.rs:93-97):**
```rust
let etag = resp.headers()
    .get("etag")
    .and_then(|v| v.to_str().ok())
    .map(|s| normalize_etag(s));
```

**This also simplifies the If-Match header construction (caldav_adapter.rs:176):**
```rust
// BEFORE: manually wraps in quotes (double-wrapping risk)
req = req.header("If-Match", format!("\"{}\"", e.trim_matches('"')));

// AFTER: etag is already normalized with quotes
req = req.header("If-Match", e);
```

### Pattern 4: Error Context Propagation (AUDIT-03)

**What:** Replace `unwrap_or_default()` on HTTP response bodies with proper error propagation.

**Approach:** For `resp.text().unwrap_or_default()` calls that feed error messages, use `.unwrap_or_else(|e| format!("<body unreadable: {}>", e))` to preserve the failure reason. For success-path calls, propagate the error properly.

**ETag retry exhaustion context enrichment (writeback.rs:477-480):**
```rust
// BEFORE:
result.errors.push(format!(
    "SyncConflict: ETag conflict unresolved after {} attempts (uid={:?})",
    MAX_ETAG_RETRIES, entry.caldav_uid
));

// AFTER:
result.errors.push(format!(
    "SyncConflict: ETag conflict unresolved after {} attempts \
     (tw_uuid={:?}, caldav_uid={:?}, href={:?})",
    MAX_ETAG_RETRIES,
    entry.tw_uuid,
    entry.caldav_uid,
    entry.fetched_vtodo.as_ref().map(|fv| &fv.href),
));
```

### Anti-Patterns to Avoid
- **Swallowing errors with unwrap_or_default():** Use `?` operator or explicit error messages that include the failure context.
- **Matching XML elements by prefixed name:** Always match on (namespace_uri, local_name) pair. Never match on "D:response" or "C:calendar-data" strings.
- **Raw comma splitting for iCal lists:** Always account for backslash-escaped commas in TEXT value lists.
- **Storing ETags without normalization:** Normalize on extraction so all downstream code sees consistent format.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| XML parsing with namespace support | String-based tag matching (current `extract_tag_content`) | `quick-xml::NsReader` | Arbitrary namespace prefixes, attributes on tags, nested CDATA, XML entities -- all handled correctly |
| RFC 5545 TEXT escaping | Manual char-by-char escape (already exists) | Existing `escape_text()`/`unescape_text()` in ical.rs | Already correct and tested; just apply to CATEGORIES |
| CLI argument parsing | Manual argv parsing for --fail-fast | `clap` derive macro | Already used; adding a field to the struct is trivial |

**Key insight:** The XML parser is the only place where a new dependency is justified. The other three bugs are fixed by correctly using existing code and patterns.

## Common Pitfalls

### Pitfall 1: CATEGORIES Round-Trip Through TW Tags
**What goes wrong:** Fixing the CATEGORIES escaping in ical.rs is necessary but not sufficient. The `build_vtodo_from_tw()` function at `writeback.rs:128` currently copies categories from the *existing CalDAV VTODO* (`base.map(|v| v.categories.clone())`), completely ignoring TW tags. After a TW-wins conflict, modified tags on the TW side would be lost.
**Why it happens:** The tags-to-CATEGORIES mapping was never implemented in the TW-to-CalDAV direction. Categories only flow CalDAV-to-CalDAV (preserved from existing VTODO).
**How to avoid:** Map `tw.tags` (a `Vec<String>` from `TWTask.tags`) to `VTODO.categories` in `build_vtodo_from_tw()`.
**Warning signs:** A test that modifies TW tags and syncs would show the old CalDAV categories unchanged.

### Pitfall 2: XML Parser State Machine Complexity
**What goes wrong:** The CalDAV multistatus response is nested: `multistatus > response > propstat > prop > (getetag|calendar-data)`. A flat string search works for simple cases but breaks when responses contain nested XML (e.g., error descriptions containing XML fragments).
**Why it happens:** String-based XML parsing cannot track nesting depth.
**How to avoid:** Use quick-xml's event-based parser with explicit depth tracking. Process events in a state machine: track which `<response>` block you are in, accumulate href/etag/calendar-data, emit a FetchedVTODO when `</response>` is reached.
**Warning signs:** Parse returns fewer VTODOs than expected, or returns corrupted data.

### Pitfall 3: ETag Double-Quoting
**What goes wrong:** The current code at `caldav_adapter.rs:176` does `format!("\"{}\"", e.trim_matches('"'))`. If the ETag is already correctly quoted (e.g., `"abc123"`), this works. But if it is weak (`W/"abc123"`), `trim_matches('"')` produces `W/"abc123` (only trims the trailing quote), and the format produces `"W/"abc123"` which is malformed.
**Why it happens:** The weak ETag prefix `W/` sits outside the quotes. Simple trim operations do not handle it.
**How to avoid:** Normalize ETags at the extraction point (strip W/, ensure quotes). Then downstream code can use the ETag directly without additional formatting.
**Warning signs:** 412 Precondition Failed errors in integration tests, especially with non-Radicale servers.

### Pitfall 4: XML CDATA in calendar-data
**What goes wrong:** Some CalDAV servers wrap the VTODO content in XML CDATA sections (`<![CDATA[BEGIN:VCALENDAR...]]>`). The current string-based parser does not handle CDATA.
**Why it happens:** String searching for `<C:calendar-data>` and `</C:calendar-data>` boundaries works only when the content is plain text between tags.
**How to avoid:** quick-xml's `Event::CData` and `Event::Text` events both provide content; concatenate text events between the calendar-data open and close tags.
**Warning signs:** Empty or truncated VTODO content from servers that use CDATA.

### Pitfall 5: Error Context in Async/Retry Paths
**What goes wrong:** When an error occurs during ETag retry, the error message at `writeback.rs:486` just calls `e.to_string()`. If `e` is a `CaldaWarriorError::CalDav { status, body }` with an empty body (from `unwrap_or_default()`), the error message is useless.
**Why it happens:** Error context is lost at two levels: first when the HTTP body fails to read (swallowed), then when the error is formatted for the user.
**How to avoid:** Fix both levels: preserve body-read failures AND include entry context when accumulating errors in SyncResult.
**Warning signs:** Error messages like "CalDAV request failed with status 412: " (empty body after the colon).

## Code Examples

### Verified: RFC 5545 TEXT Escaping Rules
```
Source: RFC 5545 Section 3.3.11 (https://icalendar.org/iCalendar-RFC-5545/3-3-11-text.html)

ESCAPED-CHAR = ("\\" / "\;" / "\," / "\N" / "\n")
; \\ encodes \
; \; encodes ;
; \, encodes ,
; \N or \n encodes newline
; \: is NOT used -- colons are not escaped
```

### Verified: RFC 5545 CATEGORIES ABNF
```
Source: RFC 5545 Section 3.8.1.2 (https://icalendar.org/iCalendar-RFC-5545/3-8-1-2-categories.html)

categories = "CATEGORIES" catparam ":" text *("," text) CRLF
; Multiple categories separated by COMMA
; Each category value is a TEXT value (escaping applies)
```

### Verified: RFC 7232 ETag Comparison for If-Match
```
Source: RFC 7232 Section 2.3 (https://www.rfc-editor.org/rfc/rfc7232.html)

If-Match uses STRONG comparison:
  - Both ETags must NOT be weak
  - Opaque-tags must match character-by-character

Weak ETag: W/"xyzzy"
Strong ETag: "xyzzy"

For If-Match: strip W/ prefix, use only the strong form.
```

### Existing: escape_text / unescape_text (ical.rs:315-353)
```rust
// Already correctly implements RFC 5545 TEXT escaping.
// Must be applied to CATEGORIES values during parse and serialize.
fn escape_text(s: &str) -> String { /* escapes \, ;, ,, \n */ }
fn unescape_text(s: &str) -> String { /* reverses escaping */ }
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Hand-rolled XML string matching | quick-xml NsReader | quick-xml 0.39.x (2024+) | NsReader provides proper namespace resolution; no more prefix guessing |
| Ignoring weak ETags | Strip W/ prefix for If-Match | RFC 7232 (since 2014) | Prevents 412 loops on Nextcloud/Baikal which may return weak ETags |

**Deprecated/outdated:**
- The legacy `parse_vtodo_from_ical()` function in `caldav_adapter.rs:428-505` is already dead code (`#[allow(dead_code)]`). Consider removing it during cleanup to reduce confusion.

## Open Questions

1. **Does Radicale ever return weak ETags?**
   - What we know: Radicale documentation says it returns strong ETags. RFC 4791 requires strong ETags for calendar objects.
   - What's unclear: Whether Radicale's actual responses always comply, especially through reverse proxies that may downgrade to weak.
   - Recommendation: Implement normalization regardless -- it is defensive and costs nothing. Test with Radicale in Phase 1; verify with Nextcloud/Baikal fixtures in Phase 4.

2. **Do any CalDAV servers use CDATA for calendar-data?**
   - What we know: Radicale returns plain text between tags. Baikal/Nextcloud (SabreDAV) may differ.
   - What's unclear: Exact SabreDAV output format for calendar-data content.
   - Recommendation: Handle both plain text and CDATA in the quick-xml parser. Low cost to implement, prevents future breakage.

3. **Tags round-trip: should CalDAV categories overwrite TW tags or merge?**
   - What we know: Currently TW tags are ignored on TW-to-CalDAV sync. CalDAV-to-TW direction does not map categories to tags at all (the `tags` field in `build_tw_task_from_caldav()` at writeback.rs:199 copies from base TW task, ignoring VTODO categories).
   - What's unclear: Whether the user expects bidirectional tags/categories mapping or just correct serialization.
   - Recommendation: For Phase 1, fix the serialization bug (escape commas correctly) and map TW tags to CATEGORIES on TW-to-CalDAV. The CalDAV-to-TW direction (categories->tags) can be addressed in Phase 3 (FIELD-01) which covers all field mappings.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust built-in) + Robot Framework 7.x (E2E) |
| Config file | `Cargo.toml` (test section) + `tests/robot/docker-compose.yml` |
| Quick run command | `cargo test` |
| Full suite command | `cargo test && CURRENT_UID=$(id -u) CURRENT_GID=$(id -g) docker compose -f tests/robot/docker-compose.yml run --rm robot` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| AUDIT-01 | Tag with comma survives round-trip parse/serialize | unit | `cargo test --lib ical::tests::test_categories_comma_roundtrip` | Wave 0 |
| AUDIT-01 | Tag with comma survives full sync round-trip (TW -> CalDAV -> TW) | E2E | RF suite: `tests/robot/suites/07_field_mapping.robot` (new test case) | Wave 0 |
| AUDIT-02 | XML with arbitrary namespace prefix parses correctly | unit | `cargo test --lib caldav_adapter::tests::test_parse_multistatus_custom_ns` | Wave 0 |
| AUDIT-02 | XML with bare (no prefix) tags parses correctly | unit | `cargo test --lib caldav_adapter::tests::test_parse_multistatus_bare_ns` | Wave 0 |
| AUDIT-03 | Error messages include task UUID and CalDAV href | unit | `cargo test --lib sync::writeback::tests::test_error_context_includes_uuid` | Wave 0 |
| AUDIT-03 | --fail-fast flag parsed by CLI | unit | `cargo test --bin caldawarrior tests::sync_fail_fast_flag` | Wave 0 |
| AUDIT-04 | Weak ETag W/ prefix stripped on extraction | unit | `cargo test --lib caldav_adapter::tests::test_normalize_etag_weak` | Wave 0 |
| AUDIT-04 | Strong ETag preserved unchanged | unit | `cargo test --lib caldav_adapter::tests::test_normalize_etag_strong` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test` (unit + integration, ~170 tests in ~120s)
- **Per wave merge:** Full suite including RF E2E
- **Phase gate:** Full suite green before verification

### Wave 0 Gaps
- [ ] `quick-xml` dependency not yet in Cargo.toml -- add via `cargo add quick-xml@0.39`
- [ ] Unit tests for CATEGORIES comma escaping -- new tests needed in `src/ical.rs` tests module
- [ ] Unit tests for namespace-aware XML parsing -- new tests needed in `src/caldav_adapter.rs` tests module
- [ ] Unit tests for ETag normalization -- new tests in `src/caldav_adapter.rs` tests module
- [ ] Unit tests for enriched error context -- new tests in `src/sync/writeback.rs` tests module
- [ ] E2E test for tags-with-commas round-trip -- new test case in RF `07_field_mapping.robot`
- [ ] RF CalDAVLibrary.py needs `put_vtodo_with_categories` keyword for E2E testing

## Sources

### Primary (HIGH confidence)
- RFC 5545 Section 3.3.11 (TEXT escaping) - https://icalendar.org/iCalendar-RFC-5545/3-3-11-text.html
- RFC 5545 Section 3.8.1.2 (CATEGORIES property) - https://icalendar.org/iCalendar-RFC-5545/3-8-1-2-categories.html
- RFC 7232 (Conditional Requests, ETag comparison) - https://www.rfc-editor.org/rfc/rfc7232.html
- RFC 4791 Section 5.3.4 (CalDAV requires strong ETags) - https://www.rfc-editor.org/rfc/rfc4791.html
- quick-xml docs - https://docs.rs/quick-xml/latest/quick_xml/
- quick-xml GitHub - https://github.com/tafia/quick-xml
- Source code analysis: src/ical.rs, src/caldav_adapter.rs, src/error.rs, src/sync/writeback.rs, src/sync/deps.rs

### Secondary (MEDIUM confidence)
- SabreDAV CalDAV client guide (ETag handling) - https://sabre.io/dav/building-a-caldav-client/
- Nextcloud CalDAV ETag issues - https://github.com/nextcloud/server/issues/6657

### Tertiary (LOW confidence)
- Specific Nextcloud/Baikal weak ETag behavior -- could not confirm whether they actually return weak ETags; implement defensive normalization regardless

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - quick-xml is the de facto Rust XML parser; verified version 0.39.2
- Architecture: HIGH - All four bugs have clear root causes in identified code locations with verified RFC specifications
- Pitfalls: HIGH - All pitfalls identified through direct code analysis with line-number references

**Research date:** 2026-03-18
**Valid until:** 2026-04-18 (stable domain -- RFC specs and Rust ecosystem do not change rapidly)
