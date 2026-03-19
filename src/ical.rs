//! iCalendar VTODO serializer/deserializer (RFC 5545).

use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;

use crate::error::CaldaWarriorError;
use crate::types::{IcalProp, RelType, VTODO};

// ── Parsing ───────────────────────────────────────────────────────────────────

/// Parse a VCALENDAR string and extract the first VTODO component.
pub fn from_icalendar_string(s: &str) -> Result<VTODO, CaldaWarriorError> {
    let unfolded = unfold_lines(s);

    // Find BEGIN:VTODO ... END:VTODO
    let vtodo_start = unfolded
        .find("BEGIN:VTODO")
        .ok_or_else(|| CaldaWarriorError::IcalParse("No VTODO component found".to_string()))?;
    let after_begin = vtodo_start + "BEGIN:VTODO".len();
    let vtodo_end = unfolded[after_begin..]
        .find("END:VTODO")
        .ok_or_else(|| CaldaWarriorError::IcalParse("No END:VTODO found".to_string()))?;
    let vtodo_block = &unfolded[after_begin..after_begin + vtodo_end];

    let mut uid: Option<String> = None;
    let mut summary: Option<String> = None;
    let mut description: Option<String> = None;
    let mut status: Option<String> = None;
    let mut last_modified: Option<DateTime<Utc>> = None;
    let mut dtstamp: Option<DateTime<Utc>> = None;
    let mut dtstart: Option<DateTime<Utc>> = None;
    let mut due: Option<DateTime<Utc>> = None;
    let mut completed: Option<DateTime<Utc>> = None;
    let mut categories: Vec<String> = vec![];
    let mut rrule: Option<String> = None;
    let mut priority: Option<u8> = None;
    let mut depends: Vec<(RelType, String)> = vec![];
    let mut extra_props: Vec<IcalProp> = vec![];
    let mut due_is_date_only = false;
    let mut dtstart_is_date_only = false;

    for line in vtodo_block.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }

        let (name, params, value) = match parse_property_line(line) {
            Some(t) => t,
            None => continue,
        };

        match name.to_uppercase().as_str() {
            "UID" => uid = Some(value.clone()),
            "SUMMARY" => summary = Some(unescape_text(&value)),
            "DESCRIPTION" => description = Some(unescape_text(&value)),
            "STATUS" => status = Some(value.clone()),
            "LAST-MODIFIED" => {
                last_modified = parse_datetime_with_params(&value, &params);
            }
            "DTSTART" => {
                dtstart = parse_datetime_with_params(&value, &params);
                dtstart_is_date_only = is_date_only_value(&value, &params);
            }
            "DUE" => {
                due = parse_datetime_with_params(&value, &params);
                due_is_date_only = is_date_only_value(&value, &params);
            }
            "COMPLETED" => {
                completed = parse_datetime_with_params(&value, &params);
            }
            "CATEGORIES" => {
                for cat in split_on_unescaped_commas(&value) {
                    let cat = unescape_text(cat.trim());
                    if !cat.is_empty() {
                        categories.push(cat);
                    }
                }
            }
            "RRULE" => rrule = Some(value.clone()),
            "PRIORITY" => {
                priority = value.trim().parse::<u8>().ok().filter(|&v| v > 0);
            }
            "RELATED-TO" => {
                // Find RELTYPE param
                let reltype_val = params
                    .iter()
                    .find(|(k, _)| k.to_uppercase() == "RELTYPE")
                    .map(|(_, v)| v.to_uppercase());

                let rel = match reltype_val.as_deref() {
                    Some("DEPENDS-ON") => RelType::DependsOn,
                    Some(other) => RelType::Other(other.to_string()),
                    None => RelType::Other(String::new()),
                };
                depends.push((rel, value.clone()));
            }
            "DTSTAMP" => {
                dtstamp = parse_datetime_with_params(&value, &params);
            }
            // Skip well-known structural markers
            "BEGIN" | "END" => {}
            _ => {
                extra_props.push(IcalProp {
                    name: name.to_string(),
                    params,
                    value,
                });
            }
        }
    }

    let uid = uid.ok_or_else(|| CaldaWarriorError::IcalParse("UID property missing".to_string()))?;

    Ok(VTODO {
        uid,
        summary,
        description,
        status,
        last_modified,
        dtstamp,
        dtstart,
        due,
        completed,
        categories,
        rrule,
        priority,
        depends,
        extra_props,
        due_is_date_only,
        dtstart_is_date_only,
    })
}

// ── Serialization ──────────────────────────────────────────────────────────────

/// Serialize a VTODO into a complete VCALENDAR string (RFC 5545).
pub fn to_icalendar_string(vtodo: &VTODO) -> String {
    let mut lines: Vec<String> = vec![];

    lines.push("BEGIN:VCALENDAR".to_string());
    lines.push("VERSION:2.0".to_string());
    lines.push("PRODID:-//caldawarrior//EN".to_string());
    lines.push("BEGIN:VTODO".to_string());

    // DTSTAMP — current UTC timestamp
    let dtstamp = format_datetime(Utc::now());
    lines.push(format!("DTSTAMP:{}", dtstamp));

    // UID
    lines.push(format!("UID:{}", vtodo.uid));

    // Optional fields
    if let Some(ref summary) = vtodo.summary {
        lines.push(format!("SUMMARY:{}", escape_text(summary)));
    }
    if let Some(ref description) = vtodo.description {
        lines.push(format!("DESCRIPTION:{}", escape_text(description)));
    }
    if let Some(ref status) = vtodo.status {
        lines.push(format!("STATUS:{}", status));
    }
    if let Some(last_modified) = vtodo.last_modified {
        lines.push(format!("LAST-MODIFIED:{}", format_datetime(last_modified)));
    }
    if let Some(dtstart) = vtodo.dtstart {
        lines.push(format!("DTSTART:{}", format_datetime(dtstart)));
    }
    if let Some(due) = vtodo.due {
        lines.push(format!("DUE:{}", format_datetime(due)));
    }
    if let Some(completed) = vtodo.completed {
        lines.push(format!("COMPLETED:{}", format_datetime(completed)));
    }
    if !vtodo.categories.is_empty() {
        let escaped: Vec<String> = vtodo.categories.iter().map(|c| escape_text(c)).collect();
        lines.push(format!("CATEGORIES:{}", escaped.join(",")));
    }
    if let Some(ref rrule) = vtodo.rrule {
        lines.push(format!("RRULE:{}", rrule));
    }
    if let Some(p) = vtodo.priority {
        lines.push(format!("PRIORITY:{}", p));
    }

    // RELATED-TO / depends
    for (rel, uid) in &vtodo.depends {
        let reltype_str = match rel {
            RelType::DependsOn => "DEPENDS-ON".to_string(),
            RelType::Other(s) => s.clone(),
        };
        lines.push(format!("RELATED-TO;RELTYPE={}:{}", reltype_str, uid));
    }

    // extra_props
    for prop in &vtodo.extra_props {
        let mut line = prop.name.clone();
        for (pk, pv) in &prop.params {
            line.push(';');
            line.push_str(pk);
            line.push('=');
            line.push_str(pv);
        }
        line.push(':');
        line.push_str(&prop.value);
        lines.push(line);
    }

    lines.push("END:VTODO".to_string());
    lines.push("END:VCALENDAR".to_string());

    // Apply line folding and join with CRLF
    let mut output = String::new();
    for line in &lines {
        output.push_str(&fold_line(line));
        output.push_str("\r\n");
    }
    output
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// RFC 5545 §3.1: unfold CRLF or LF followed by space or tab.
fn unfold_lines(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\r' {
            // Consume optional \n
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            // Check if next char is space or tab (continuation line)
            if chars.peek() == Some(&' ') || chars.peek() == Some(&'\t') {
                chars.next(); // skip the whitespace — this is a fold continuation
            } else {
                result.push('\n');
            }
        } else if c == '\n' {
            if chars.peek() == Some(&' ') || chars.peek() == Some(&'\t') {
                chars.next(); // skip whitespace — fold continuation
            } else {
                result.push('\n');
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Parse a single unfolded property line into (name, params, value).
/// Returns None if the line has no colon or cannot be parsed.
fn parse_property_line(line: &str) -> Option<(String, Vec<(String, String)>, String)> {
    // Split on first ':' that is not inside a quoted string
    let colon_pos = find_colon_outside_quotes(line)?;
    let left = &line[..colon_pos];
    let value = line[colon_pos + 1..].to_string();

    // Parse the left part: NAME[;PARAM=VAL]*
    let mut segments = split_params(left);
    if segments.is_empty() {
        return None;
    }
    let name = segments.remove(0).to_uppercase();

    let mut params: Vec<(String, String)> = vec![];
    for seg in segments {
        if let Some(eq_pos) = seg.find('=') {
            let key = seg[..eq_pos].to_uppercase();
            let val = seg[eq_pos + 1..].trim_matches('"').to_string();
            params.push((key, val));
        } else {
            // param with no value — store as (KEY, "")
            params.push((seg.to_uppercase(), String::new()));
        }
    }

    Some((name, params, value))
}

/// Find the position of the first ':' that is not inside double-quoted string.
fn find_colon_outside_quotes(s: &str) -> Option<usize> {
    let mut in_quotes = false;
    for (i, c) in s.char_indices() {
        match c {
            '"' => in_quotes = !in_quotes,
            ':' if !in_quotes => return Some(i),
            _ => {}
        }
    }
    None
}

/// Split property param segment on ';', respecting double-quoted values.
fn split_params(s: &str) -> Vec<String> {
    let mut result = vec![];
    let mut current = String::new();
    let mut in_quotes = false;

    for c in s.chars() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                current.push(c);
            }
            ';' if !in_quotes => {
                result.push(current.trim().to_string());
                current = String::new();
            }
            _ => current.push(c),
        }
    }
    if !current.trim().is_empty() || !result.is_empty() {
        result.push(current.trim().to_string());
    }
    result
}

/// Split a string on commas that are NOT escaped by a preceding backslash.
///
/// In RFC 5545, CATEGORIES values are comma-separated, but a literal comma
/// inside a value is escaped as `\,`.  This function splits only on
/// unescaped commas and returns slices of the original string.
fn split_on_unescaped_commas(s: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut start = 0;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            // Skip the next character (it is escaped)
            i += 2;
        } else if bytes[i] == b',' {
            result.push(&s[start..i]);
            i += 1;
            start = i;
        } else {
            i += 1;
        }
    }
    result.push(&s[start..]);
    result
}

/// RFC 5545 TEXT unescape: \n→newline, \N→newline, \,→comma, \;→semicolon, \\→backslash
fn unescape_text(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') | Some('N') => result.push('\n'),
                Some(',') => result.push(','),
                Some(';') => result.push(';'),
                Some('\\') => result.push('\\'),
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// RFC 5545 TEXT escape: \→\\, ;→\;, ,→\,, newline→\n
fn escape_text(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => result.push_str("\\\\"),
            ';' => result.push_str("\\;"),
            ',' => result.push_str("\\,"),
            '\n' => result.push_str("\\n"),
            '\r' => {} // skip bare CR
            other => result.push(other),
        }
    }
    result
}

/// Fold a single line at 75 **octets** (bytes), inserting CRLF+space at UTF-8
/// character boundaries. The first segment is 75 bytes; continuations are 74
/// bytes of content (preceded by 1 byte of space).
fn fold_line(line: &str) -> String {
    const FIRST_MAX: usize = 75;
    const CONT_MAX: usize = 74; // 75 - 1 byte for the leading space

    if line.len() <= FIRST_MAX {
        return line.to_string();
    }

    let mut result = String::with_capacity(line.len() + (line.len() / 60) * 3);
    let mut remaining = line;
    let mut first = true;

    while !remaining.is_empty() {
        let max = if first { FIRST_MAX } else { CONT_MAX };

        if remaining.len() <= max {
            if !first {
                result.push(' ');
            }
            result.push_str(remaining);
            break;
        }

        // Find the last UTF-8 character boundary at or before `max` bytes.
        let mut cut = max;
        while cut > 0 && !remaining.is_char_boundary(cut) {
            cut -= 1;
        }

        if !first {
            result.push(' ');
        }
        result.push_str(&remaining[..cut]);
        result.push_str("\r\n");
        remaining = &remaining[cut..];
        first = false;
    }

    result
}

/// Format a UTC DateTime in iCal UTC format (YYYYMMDDTHHMMSSZ).
fn format_datetime(dt: DateTime<Utc>) -> String {
    dt.format("%Y%m%dT%H%M%SZ").to_string()
}

/// Detect whether a datetime property value represents a DATE-only value
/// (no time component). Checks for explicit VALUE=DATE parameter or
/// implicit 8-char date format without 'T'.
fn is_date_only_value(value: &str, params: &[(String, String)]) -> bool {
    params.iter().any(|(k, v)| {
        k.eq_ignore_ascii_case("VALUE") && v.eq_ignore_ascii_case("DATE")
    }) || {
        let trimmed = value.trim();
        trimmed.len() == 8 && !trimmed.contains('T')
    }
}

/// Parse a datetime property value, handling TZID param, UTC suffix 'Z',
/// floating (treat as UTC), and date-only (YYYYMMDD → midnight UTC).
fn parse_datetime_with_params(value: &str, params: &[(String, String)]) -> Option<DateTime<Utc>> {
    let value = value.trim();

    // Check for TZID param
    let tzid = params
        .iter()
        .find(|(k, _)| k.to_uppercase() == "TZID")
        .map(|(_, v)| v.as_str());

    if let Some(tzid_str) = tzid {
        // Parse as local time in the given timezone, then convert to UTC
        let tz: Tz = tzid_str
            .parse()
            .ok()?;
        // Strip trailing 'Z' if present (shouldn't be with TZID, but be safe)
        let s = if value.ends_with('Z') {
            &value[..value.len() - 1]
        } else {
            value
        };
        let naive = NaiveDateTime::parse_from_str(s, "%Y%m%dT%H%M%S").ok()?;
        let mapped = tz.from_local_datetime(&naive);
        let dt_utc = mapped.single().map(|dt| dt.to_utc())
            .or_else(|| mapped.latest().map(|dt| dt.to_utc()))
            .unwrap_or_else(|| naive.and_utc());  // gap: treat as UTC
        return Some(dt_utc);
    }

    // UTC suffix
    if value.ends_with('Z') {
        let s = &value[..value.len() - 1];
        if let Ok(naive) = NaiveDateTime::parse_from_str(s, "%Y%m%dT%H%M%S") {
            return Some(naive.and_utc());
        }
    }

    // Date-only: YYYYMMDD
    if value.len() == 8 && !value.contains('T') {
        if let Ok(nd) = NaiveDate::parse_from_str(value, "%Y%m%d") {
            return Some(nd.and_hms_opt(0, 0, 0)?.and_utc());
        }
    }

    // Floating (no Z, not date-only) — treat as UTC
    if let Ok(naive) = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S") {
        return Some(naive.and_utc());
    }

    None
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, TimeZone, Timelike};

    fn make_basic_vtodo() -> VTODO {
        VTODO {
            uid: "test-uid-001".to_string(),
            summary: Some("My Task".to_string()),
            description: Some("A description".to_string()),
            status: Some("NEEDS-ACTION".to_string()),
            last_modified: Some(Utc.with_ymd_and_hms(2026, 2, 26, 10, 0, 0).unwrap()),
            dtstamp: None,
            dtstart: Some(Utc.with_ymd_and_hms(2026, 2, 26, 9, 0, 0).unwrap()),
            due: Some(Utc.with_ymd_and_hms(2026, 2, 27, 12, 0, 0).unwrap()),
            completed: None,
            categories: vec!["work".to_string(), "personal".to_string()],
            rrule: None,
            priority: None,
            depends: vec![],
            extra_props: vec![],
            due_is_date_only: false,
            dtstart_is_date_only: false,
        }
    }

    #[test]
    fn test_round_trip_basic() {
        let original = make_basic_vtodo();
        let serialized = to_icalendar_string(&original);
        let parsed = from_icalendar_string(&serialized).expect("parse should succeed");

        assert_eq!(parsed.uid, original.uid);
        assert_eq!(parsed.summary, original.summary);
        assert_eq!(parsed.description, original.description);
        assert_eq!(parsed.status, original.status);
        assert_eq!(parsed.last_modified, original.last_modified);
        assert_eq!(parsed.dtstart, original.dtstart);
        assert_eq!(parsed.due, original.due);
        assert_eq!(parsed.completed, original.completed);
        assert_eq!(parsed.categories, original.categories);
        assert_eq!(parsed.rrule, original.rrule);
    }

    #[test]
    fn test_text_escaping() {
        let vtodo = VTODO {
            uid: "escape-uid".to_string(),
            summary: Some("Back\\slash; semi,comma\nnewline".to_string()),
            ..Default::default()
        };

        let serialized = to_icalendar_string(&vtodo);

        // Verify the escaped form appears in the raw output
        assert!(
            serialized.contains("Back\\\\slash\\;"),
            "backslash and semicolon should be escaped"
        );
        assert!(
            serialized.contains("semi\\,comma"),
            "comma should be escaped"
        );
        assert!(
            serialized.contains("\\n"),
            "newline should be escaped as \\n"
        );

        // Round-trip: the parsed value should be the original
        let parsed = from_icalendar_string(&serialized).expect("parse");
        assert_eq!(
            parsed.summary.as_deref(),
            Some("Back\\slash; semi,comma\nnewline")
        );
    }

    #[test]
    fn test_line_folding() {
        // A summary that exceeds 75 bytes when prefixed with "SUMMARY:"
        // "SUMMARY:" is 8 chars, so 68+ chars of content will push it over 75
        let long_summary = "A".repeat(80);
        let vtodo = VTODO {
            uid: "fold-uid".to_string(),
            summary: Some(long_summary.clone()),
            ..Default::default()
        };

        let serialized = to_icalendar_string(&vtodo);

        // Verify fold markers (CRLF+space) are present for the long SUMMARY line
        assert!(
            serialized.contains("\r\n "),
            "folded output should contain CRLF+space fold markers"
        );

        // Verify each physical line is at most 75 bytes
        for physical_line in serialized.split("\r\n") {
            assert!(
                physical_line.len() <= 75,
                "physical line exceeds 75 bytes: {:?} (len={})",
                physical_line,
                physical_line.len()
            );
        }

        // Verify round-trip
        let parsed = from_icalendar_string(&serialized).expect("parse folded");
        assert_eq!(parsed.summary.as_deref(), Some(long_summary.as_str()));
    }

    #[test]
    fn test_tzid_conversion() {
        // DTSTART with TZID=America/New_York; 2026-01-15T10:00:00 Eastern = 15:00 UTC
        let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\n\
            UID:tzid-test-001\r\n\
            DTSTART;TZID=America/New_York:20260115T100000\r\n\
            END:VTODO\r\nEND:VCALENDAR\r\n";

        let vtodo = from_icalendar_string(ical).expect("parse");
        let dtstart = vtodo.dtstart.expect("dtstart present");

        // America/New_York in January is UTC-5
        assert_eq!(dtstart.hour(), 15, "10:00 EST should be 15:00 UTC");
        assert_eq!(dtstart.minute(), 0);
        assert_eq!(dtstart.day(), 15);
    }

    #[test]
    fn test_reltype_depends_on() {
        let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\n\
            UID:rel-test-001\r\n\
            RELATED-TO;RELTYPE=DEPENDS-ON:some-uid-abc\r\n\
            END:VTODO\r\nEND:VCALENDAR\r\n";

        let vtodo = from_icalendar_string(ical).expect("parse");
        assert_eq!(vtodo.depends.len(), 1);
        assert_eq!(vtodo.depends[0].0, RelType::DependsOn);
        assert_eq!(vtodo.depends[0].1, "some-uid-abc");
    }

    #[test]
    fn test_reltype_other() {
        let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\n\
            UID:rel-test-002\r\n\
            RELATED-TO;RELTYPE=CHILD:some-uid-xyz\r\n\
            END:VTODO\r\nEND:VCALENDAR\r\n";

        let vtodo = from_icalendar_string(ical).expect("parse");
        assert_eq!(vtodo.depends.len(), 1);
        assert_eq!(vtodo.depends[0].0, RelType::Other("CHILD".to_string()));
        assert_eq!(vtodo.depends[0].1, "some-uid-xyz");
    }

    #[test]
    fn test_extra_props_preserved() {
        let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\n\
            UID:extra-uid\r\n\
            X-CUSTOM:myvalue\r\n\
            END:VTODO\r\nEND:VCALENDAR\r\n";

        let vtodo = from_icalendar_string(ical).expect("parse");
        assert_eq!(vtodo.extra_props.len(), 1);
        assert_eq!(vtodo.extra_props[0].name, "X-CUSTOM");
        assert_eq!(vtodo.extra_props[0].value, "myvalue");

        // Round-trip
        let serialized = to_icalendar_string(&vtodo);
        let reparsed = from_icalendar_string(&serialized).expect("reparse");
        assert_eq!(reparsed.extra_props.len(), 1);
        assert_eq!(reparsed.extra_props[0].name, "X-CUSTOM");
        assert_eq!(reparsed.extra_props[0].value, "myvalue");
    }

    #[test]
    fn test_dtstamp_present() {
        let vtodo = make_basic_vtodo();
        let serialized = to_icalendar_string(&vtodo);
        assert!(
            serialized.contains("DTSTAMP:"),
            "DTSTAMP should be present in output"
        );
    }

    #[test]
    fn test_dtstamp_parsed() {
        // A VTODO with an explicit DTSTAMP should populate vtodo.dtstamp after parsing.
        let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\n\
            UID:dtstamp-test-001\r\n\
            DTSTAMP:20260215T120000Z\r\n\
            END:VTODO\r\nEND:VCALENDAR\r\n";

        let vtodo = from_icalendar_string(ical).expect("parse");
        let dtstamp = vtodo.dtstamp.expect("dtstamp should be parsed");
        assert_eq!(dtstamp.format("%Y%m%dT%H%M%SZ").to_string(), "20260215T120000Z");
        // LAST-MODIFIED absent; dtstamp is present
        assert!(vtodo.last_modified.is_none());
    }

    #[test]
    fn test_missing_uid_error() {
        let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\n\
            SUMMARY:No UID here\r\n\
            END:VTODO\r\nEND:VCALENDAR\r\n";

        let result = from_icalendar_string(ical);
        assert!(result.is_err(), "should return error when UID is missing");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.to_lowercase().contains("uid"),
            "error message should mention UID: {}",
            err_msg
        );
    }

    #[test]
    fn priority_parsed_from_vtodo() {
        let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\n\
            UID:prio-test-001\r\nSUMMARY:High priority\r\nPRIORITY:3\r\n\
            END:VTODO\r\nEND:VCALENDAR\r\n";
        let vtodo = from_icalendar_string(ical).expect("parse");
        assert_eq!(vtodo.priority, Some(3));
    }

    #[test]
    fn priority_zero_treated_as_absent() {
        let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\n\
            UID:prio-test-002\r\nSUMMARY:No priority\r\nPRIORITY:0\r\n\
            END:VTODO\r\nEND:VCALENDAR\r\n";
        let vtodo = from_icalendar_string(ical).expect("parse");
        assert_eq!(vtodo.priority, None);
    }

    #[test]
    fn priority_serialized_to_vtodo() {
        let vtodo = VTODO {
            uid: "prio-ser-001".to_string(),
            summary: Some("Task".to_string()),
            priority: Some(1),
            ..Default::default()
        };
        let s = to_icalendar_string(&vtodo);
        assert!(s.contains("PRIORITY:1"), "serialized output should contain PRIORITY:1: {}", s);
        // Round-trip: parse back and verify priority preserved
        let parsed = from_icalendar_string(&s).expect("parse");
        assert_eq!(parsed.priority, Some(1));
    }

    #[test]
    fn priority_absent_not_emitted() {
        let vtodo = VTODO {
            uid: "prio-absent-001".to_string(),
            summary: Some("Task".to_string()),
            ..Default::default()
        };
        let s = to_icalendar_string(&vtodo);
        assert!(!s.contains("PRIORITY"), "serialized output should not contain PRIORITY when None: {}", s);
    }

    // ── CATEGORIES comma-escaping tests (AUDIT-01) ───────────────────────

    #[test]
    fn test_split_on_unescaped_commas_basic() {
        // Simple split
        assert_eq!(split_on_unescaped_commas("a,b,c"), vec!["a", "b", "c"]);
        // Escaped comma stays together
        assert_eq!(
            split_on_unescaped_commas("Smith\\, John,Work"),
            vec!["Smith\\, John", "Work"]
        );
        // Multiple escaped commas
        assert_eq!(
            split_on_unescaped_commas("a\\,b,c\\,d"),
            vec!["a\\,b", "c\\,d"]
        );
        // No comma
        assert_eq!(split_on_unescaped_commas("simple"), vec!["simple"]);
        // Empty string
        assert_eq!(split_on_unescaped_commas(""), vec![""]);
    }

    #[test]
    fn test_categories_comma_parse() {
        // "CATEGORIES:Smith\, John,Work" should produce 2 categories, not 3
        let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\n\
            UID:cat-comma-001\r\n\
            CATEGORIES:Smith\\, John,Work\r\n\
            END:VTODO\r\nEND:VCALENDAR\r\n";
        let vtodo = from_icalendar_string(ical).expect("parse");
        assert_eq!(vtodo.categories, vec!["Smith, John", "Work"]);
    }

    #[test]
    fn test_categories_comma_serialize() {
        let vtodo = VTODO {
            uid: "cat-comma-ser-001".to_string(),
            categories: vec!["Smith, John".to_string(), "Work".to_string()],
            ..Default::default()
        };
        let serialized = to_icalendar_string(&vtodo);
        // The serialized form must escape commas within category values
        assert!(
            serialized.contains("CATEGORIES:Smith\\, John,Work"),
            "Expected escaped CATEGORIES line, got: {}",
            serialized
        );
    }

    #[test]
    fn test_categories_comma_roundtrip() {
        let original_cats = vec!["Smith, John".to_string(), "Work".to_string()];
        let vtodo = VTODO {
            uid: "cat-rt-001".to_string(),
            categories: original_cats.clone(),
            ..Default::default()
        };
        let serialized = to_icalendar_string(&vtodo);
        let parsed = from_icalendar_string(&serialized).expect("roundtrip parse");
        assert_eq!(parsed.categories, original_cats);
    }

    #[test]
    fn test_categories_simple_unchanged() {
        // No-comma categories must still work as before
        let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\n\
            UID:cat-simple-001\r\n\
            CATEGORIES:work,personal\r\n\
            END:VTODO\r\nEND:VCALENDAR\r\n";
        let vtodo = from_icalendar_string(ical).expect("parse");
        assert_eq!(vtodo.categories, vec!["work", "personal"]);
    }

    #[test]
    fn test_categories_empty() {
        let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\n\
            UID:cat-empty-001\r\n\
            CATEGORIES:\r\n\
            END:VTODO\r\nEND:VCALENDAR\r\n";
        let vtodo = from_icalendar_string(ical).expect("parse");
        assert!(vtodo.categories.is_empty(), "empty CATEGORIES should produce empty vec");
    }

    // ── DATE-only detection tests (COMPAT-02) ────────────────────────────

    #[test]
    fn test_vtodo_default_derive() {
        let vtodo = VTODO::default();
        assert!(!vtodo.due_is_date_only, "due_is_date_only should default to false");
        assert!(!vtodo.dtstart_is_date_only, "dtstart_is_date_only should default to false");
        assert!(vtodo.uid.is_empty());
        assert!(vtodo.due.is_none());
        assert!(vtodo.dtstart.is_none());
    }

    #[test]
    fn test_date_only_due_parsed() {
        let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\n\
            UID:date-only-due-001\r\n\
            DUE;VALUE=DATE:20260315\r\n\
            END:VTODO\r\nEND:VCALENDAR\r\n";
        let vtodo = from_icalendar_string(ical).expect("parse");
        assert!(vtodo.due_is_date_only, "DUE;VALUE=DATE should set due_is_date_only");
        let due = vtodo.due.expect("due should be present");
        assert_eq!(due, Utc.with_ymd_and_hms(2026, 3, 15, 0, 0, 0).unwrap());
    }

    #[test]
    fn test_date_only_due_implicit() {
        // No VALUE=DATE param, but 8-char date-only format
        let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\n\
            UID:date-only-due-002\r\n\
            DUE:20260315\r\n\
            END:VTODO\r\nEND:VCALENDAR\r\n";
        let vtodo = from_icalendar_string(ical).expect("parse");
        assert!(vtodo.due_is_date_only, "implicit 8-char DUE should set due_is_date_only");
    }

    #[test]
    fn test_datetime_due_not_date_only() {
        let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\n\
            UID:datetime-due-001\r\n\
            DUE:20260315T120000Z\r\n\
            END:VTODO\r\nEND:VCALENDAR\r\n";
        let vtodo = from_icalendar_string(ical).expect("parse");
        assert!(!vtodo.due_is_date_only, "DUE with time should NOT set due_is_date_only");
    }

    #[test]
    fn test_date_only_dtstart() {
        let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\n\
            UID:date-only-dtstart-001\r\n\
            DTSTART;VALUE=DATE:20260401\r\n\
            END:VTODO\r\nEND:VCALENDAR\r\n";
        let vtodo = from_icalendar_string(ical).expect("parse");
        assert!(vtodo.dtstart_is_date_only, "DTSTART;VALUE=DATE should set dtstart_is_date_only");
        let dtstart = vtodo.dtstart.expect("dtstart should be present");
        assert_eq!(dtstart, Utc.with_ymd_and_hms(2026, 4, 1, 0, 0, 0).unwrap());
    }

    // ── DST edge-case tests (COMPAT-03) ─────────────────────────────────

    #[test]
    fn test_tzid_fall_back_ambiguous() {
        // 2026-11-01 01:30 America/New_York is ambiguous (fall-back: EDT and EST both valid)
        // .latest() should pick EST (standard time): 01:30 EST = 06:30 UTC
        let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\n\
            UID:dst-fallback-001\r\n\
            DUE;TZID=America/New_York:20261101T013000\r\n\
            END:VTODO\r\nEND:VCALENDAR\r\n";
        let vtodo = from_icalendar_string(ical).expect("parse");
        let due = vtodo.due.expect("due should be present for ambiguous DST time");
        assert_eq!(due.hour(), 6, "01:30 EST (latest/standard) = 06:30 UTC, got hour={}", due.hour());
        assert_eq!(due.minute(), 30);
    }

    #[test]
    fn test_tzid_spring_forward_gap() {
        // 2026-03-08 02:30 America/New_York is a gap time (spring forward: 02:00 -> 03:00)
        // Should NOT return None -- fallback to naive-as-UTC
        let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\n\
            UID:dst-gap-001\r\n\
            DUE;TZID=America/New_York:20260308T023000\r\n\
            END:VTODO\r\nEND:VCALENDAR\r\n";
        let vtodo = from_icalendar_string(ical).expect("parse");
        assert!(vtodo.due.is_some(), "gap time should NOT return None");
    }

    #[test]
    fn test_tzid_paris_summer() {
        // 2026-07-15 14:00 Europe/Paris (CEST = UTC+2) = 12:00 UTC
        let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\n\
            UID:paris-summer-001\r\n\
            DTSTART;TZID=Europe/Paris:20260715T140000\r\n\
            END:VTODO\r\nEND:VCALENDAR\r\n";
        let vtodo = from_icalendar_string(ical).expect("parse");
        let dtstart = vtodo.dtstart.expect("dtstart should be present");
        assert_eq!(dtstart.hour(), 12, "14:00 CEST = 12:00 UTC, got hour={}", dtstart.hour());
    }

    #[test]
    fn test_tzid_paris_winter() {
        // 2026-01-15 14:00 Europe/Paris (CET = UTC+1) = 13:00 UTC
        let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\n\
            UID:paris-winter-001\r\n\
            DTSTART;TZID=Europe/Paris:20260115T140000\r\n\
            END:VTODO\r\nEND:VCALENDAR\r\n";
        let vtodo = from_icalendar_string(ical).expect("parse");
        let dtstart = vtodo.dtstart.expect("dtstart should be present");
        assert_eq!(dtstart.hour(), 13, "14:00 CET = 13:00 UTC, got hour={}", dtstart.hour());
    }
}
