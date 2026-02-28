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
    let mut dtstart: Option<DateTime<Utc>> = None;
    let mut due: Option<DateTime<Utc>> = None;
    let mut completed: Option<DateTime<Utc>> = None;
    let mut categories: Vec<String> = vec![];
    let mut rrule: Option<String> = None;
    let mut depends: Vec<(RelType, String)> = vec![];
    let mut extra_props: Vec<IcalProp> = vec![];

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
            }
            "DUE" => {
                due = parse_datetime_with_params(&value, &params);
            }
            "COMPLETED" => {
                completed = parse_datetime_with_params(&value, &params);
            }
            "CATEGORIES" => {
                for cat in value.split(',') {
                    let cat = cat.trim().to_string();
                    if !cat.is_empty() {
                        categories.push(cat);
                    }
                }
            }
            "RRULE" => rrule = Some(value.clone()),
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
            // Skip well-known non-data properties
            "DTSTAMP" | "BEGIN" | "END" => {}
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
        dtstart,
        due,
        completed,
        categories,
        rrule,
        depends,
        extra_props,
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
        lines.push(format!("CATEGORIES:{}", vtodo.categories.join(",")));
    }
    if let Some(ref rrule) = vtodo.rrule {
        lines.push(format!("RRULE:{}", rrule));
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
        let dt = tz.from_local_datetime(&naive).single()?;
        return Some(dt.to_utc());
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
            dtstart: Some(Utc.with_ymd_and_hms(2026, 2, 26, 9, 0, 0).unwrap()),
            due: Some(Utc.with_ymd_and_hms(2026, 2, 27, 12, 0, 0).unwrap()),
            completed: None,
            categories: vec!["work".to_string(), "personal".to_string()],
            rrule: None,
            depends: vec![],
            extra_props: vec![],
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
            description: None,
            status: None,
            last_modified: None,
            dtstart: None,
            due: None,
            completed: None,
            categories: vec![],
            rrule: None,
            depends: vec![],
            extra_props: vec![],
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
            description: None,
            status: None,
            last_modified: None,
            dtstart: None,
            due: None,
            completed: None,
            categories: vec![],
            rrule: None,
            depends: vec![],
            extra_props: vec![],
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
}
