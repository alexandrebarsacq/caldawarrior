use crate::error::CaldaWarriorError;
use crate::types::{FetchedVTODO, VTODO};
use chrono::{DateTime, NaiveDateTime, Utc};
use reqwest::blocking::Client;
use std::sync::Mutex;
use std::time::Duration;

// ---------------------------------------------------------------------------
// CalDavClient trait
// ---------------------------------------------------------------------------

pub trait CalDavClient: Send + Sync {
    /// Fetch all VTODOs from a calendar URL.
    /// Returns list of FetchedVTODO (each has href, etag, vtodo).
    fn list_vtodos(&self, calendar_url: &str) -> Result<Vec<FetchedVTODO>, CaldaWarriorError>;

    /// PUT a VTODO to the CalDAV server.
    /// - If `etag` is Some: add `If-Match: "<etag>"` header (conditional update)
    /// - If `etag` is None: add `If-None-Match: *` header (create-only)
    /// Returns the new ETag from the response (if present).
    fn put_vtodo(
        &self,
        href: &str,
        ical_content: &str,
        etag: Option<&str>,
    ) -> Result<Option<String>, CaldaWarriorError>;

    /// DELETE a VTODO from the CalDAV server.
    /// - If `etag` is Some: add `If-Match: "<etag>"` header
    fn delete_vtodo(&self, href: &str, etag: Option<&str>) -> Result<(), CaldaWarriorError>;
}

// ---------------------------------------------------------------------------
// RealCalDavClient
// ---------------------------------------------------------------------------

pub struct RealCalDavClient {
    client: Client,
    base_url: String,
    username: String,
    password: String,
}

impl RealCalDavClient {
    pub fn new(
        base_url: String,
        username: String,
        password: String,
        timeout_seconds: u64,
        allow_insecure_tls: bool,
    ) -> Result<Self, CaldaWarriorError> {
        let client = reqwest::blocking::ClientBuilder::new()
            .timeout(Duration::from_secs(timeout_seconds))
            .danger_accept_invalid_certs(allow_insecure_tls)
            .build()
            .map_err(|e| CaldaWarriorError::CalDav {
                status: 0,
                body: format!("Failed to build HTTP client: {}", e),
            })?;

        Ok(Self {
            client,
            base_url,
            username,
            password,
        })
    }

    /// Resolve an href to a full URL.
    ///
    /// If `href` is already an absolute URL (starts with `http://` or `https://`),
    /// it is returned as-is. Otherwise `base_url` is prepended.
    fn resolve_url(&self, href: &str) -> String {
        if href.starts_with("http://") || href.starts_with("https://") {
            href.to_string()
        } else {
            format!("{}{}", self.base_url.trim_end_matches('/'), href)
        }
    }

    /// Fetch a single VTODO by href for ETag conflict re-fetch.
    fn fetch_single_vtodo(&self, href: &str) -> Result<FetchedVTODO, CaldaWarriorError> {
        let url = self.resolve_url(href);
        let resp = self
            .client
            .get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .map_err(|e| map_reqwest_error(e, &url))?;

        match resp.status().as_u16() {
            200 => {
                let etag = resp
                    .headers()
                    .get("etag")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());
                let body = resp.text().unwrap_or_default();
                let vtodo = crate::ical::from_icalendar_string(&body)
                    .map_err(|e| CaldaWarriorError::IcalParse(format!(
                        "Could not parse VTODO from GET response for {}: {}",
                        href, e
                    )))?;
                Ok(FetchedVTODO {
                    href: href.to_string(),
                    etag,
                    vtodo,
                })
            }
            401 => Err(CaldaWarriorError::Auth {
                server_url: url,
            }),
            status => {
                let body = resp.text().unwrap_or_default();
                Err(CaldaWarriorError::CalDav { status, body })
            }
        }
    }
}

impl CalDavClient for RealCalDavClient {
    fn list_vtodos(&self, calendar_url: &str) -> Result<Vec<FetchedVTODO>, CaldaWarriorError> {
        let report_body = r#"<?xml version="1.0" encoding="UTF-8"?>
<C:calendar-query xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop>
    <D:getetag/>
    <C:calendar-data/>
  </D:prop>
  <C:filter>
    <C:comp-filter name="VCALENDAR">
      <C:comp-filter name="VTODO"/>
    </C:comp-filter>
  </C:filter>
</C:calendar-query>"#;

        let resp = self
            .client
            .request(reqwest::Method::from_bytes(b"REPORT").unwrap(), calendar_url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Content-Type", "application/xml; charset=utf-8")
            .header("Depth", "1")
            .body(report_body)
            .send()
            .map_err(|e| map_reqwest_error(e, calendar_url))?;

        match resp.status().as_u16() {
            200 | 207 => {
                let body = resp.text().unwrap_or_default();
                Ok(parse_multistatus_vtodos(&body))
            }
            401 => Err(CaldaWarriorError::Auth {
                server_url: calendar_url.to_string(),
            }),
            status => {
                let body = resp.text().unwrap_or_default();
                Err(CaldaWarriorError::CalDav { status, body })
            }
        }
    }

    fn put_vtodo(
        &self,
        href: &str,
        ical_content: &str,
        etag: Option<&str>,
    ) -> Result<Option<String>, CaldaWarriorError> {
        let url = self.resolve_url(href);
        let mut req = self
            .client
            .put(&url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Content-Type", "text/calendar; charset=utf-8")
            .body(ical_content.to_string());

        if let Some(e) = etag {
            req = req.header("If-Match", format!("\"{}\"", e.trim_matches('"')));
        } else {
            req = req.header("If-None-Match", "*");
        }

        let resp = req.send().map_err(|e| map_reqwest_error(e, &url))?;

        match resp.status().as_u16() {
            200 | 201 | 204 => {
                let new_etag = resp
                    .headers()
                    .get("etag")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());
                Ok(new_etag)
            }
            401 => Err(CaldaWarriorError::Auth { server_url: url }),
            412 => {
                let refetched = self.fetch_single_vtodo(href)?;
                Err(CaldaWarriorError::EtagConflict {
                    refetched_vtodo: refetched,
                })
            }
            status => {
                let body = resp.text().unwrap_or_default();
                Err(CaldaWarriorError::CalDav { status, body })
            }
        }
    }

    fn delete_vtodo(&self, href: &str, etag: Option<&str>) -> Result<(), CaldaWarriorError> {
        let url = self.resolve_url(href);
        let mut req = self
            .client
            .delete(&url)
            .basic_auth(&self.username, Some(&self.password));

        if let Some(e) = etag {
            req = req.header("If-Match", format!("\"{}\"", e.trim_matches('"')));
        }

        let resp = req.send().map_err(|e| map_reqwest_error(e, &url))?;

        match resp.status().as_u16() {
            200 | 204 => Ok(()),
            401 => Err(CaldaWarriorError::Auth { server_url: url }),
            412 => {
                let refetched = self.fetch_single_vtodo(href)?;
                Err(CaldaWarriorError::EtagConflict {
                    refetched_vtodo: refetched,
                })
            }
            status => {
                let body = resp.text().unwrap_or_default();
                Err(CaldaWarriorError::CalDav { status, body })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// MockCalDavClient
// ---------------------------------------------------------------------------

pub struct MockCalDavClient {
    pub list_responses: Mutex<Vec<Result<Vec<FetchedVTODO>, CaldaWarriorError>>>,
    pub put_responses: Mutex<Vec<Result<Option<String>, CaldaWarriorError>>>,
    pub delete_responses: Mutex<Vec<Result<(), CaldaWarriorError>>>,
    pub calls: Mutex<Vec<CalDavCall>>,
}

#[derive(Debug)]
pub enum CalDavCall {
    List(String),
    Put { href: String, etag: Option<String> },
    Delete { href: String, etag: Option<String> },
}

impl MockCalDavClient {
    pub fn new() -> Self {
        Self {
            list_responses: Mutex::new(vec![]),
            put_responses: Mutex::new(vec![]),
            delete_responses: Mutex::new(vec![]),
            calls: Mutex::new(vec![]),
        }
    }
}

impl CalDavClient for MockCalDavClient {
    fn list_vtodos(&self, calendar_url: &str) -> Result<Vec<FetchedVTODO>, CaldaWarriorError> {
        self.calls
            .lock()
            .unwrap()
            .push(CalDavCall::List(calendar_url.to_string()));
        let mut resp = self.list_responses.lock().unwrap();
        if resp.is_empty() {
            Ok(vec![])
        } else {
            resp.remove(0)
        }
    }

    fn put_vtodo(
        &self,
        href: &str,
        _ical: &str,
        etag: Option<&str>,
    ) -> Result<Option<String>, CaldaWarriorError> {
        self.calls.lock().unwrap().push(CalDavCall::Put {
            href: href.to_string(),
            etag: etag.map(|e| e.to_string()),
        });
        let mut resp = self.put_responses.lock().unwrap();
        if resp.is_empty() {
            Ok(None)
        } else {
            resp.remove(0)
        }
    }

    fn delete_vtodo(&self, href: &str, etag: Option<&str>) -> Result<(), CaldaWarriorError> {
        self.calls.lock().unwrap().push(CalDavCall::Delete {
            href: href.to_string(),
            etag: etag.map(|e| e.to_string()),
        });
        let mut resp = self.delete_responses.lock().unwrap();
        if resp.is_empty() {
            Ok(())
        } else {
            resp.remove(0)
        }
    }
}

// ---------------------------------------------------------------------------
// XML parsing helpers (string-based, no external XML crate)
// ---------------------------------------------------------------------------

/// Extract the text content between the first occurrence of an opening tag
/// (with or without namespace prefix) and its matching closing tag.
fn extract_tag_content<'a>(xml: &'a str, local_name: &str) -> Option<&'a str> {
    // Try with namespace prefix variants then without
    let prefixes = ["D:", "C:", ""];
    for prefix in &prefixes {
        let open_tag = format!("<{}{}>", prefix, local_name);
        let close_tag = format!("</{}{}>", prefix, local_name);
        if let Some(start) = xml.find(&open_tag) {
            let content_start = start + open_tag.len();
            if let Some(end) = xml[content_start..].find(&close_tag) {
                return Some(&xml[content_start..content_start + end]);
            }
        }
    }
    None
}

/// Parse the XML multi-status REPORT/PROPFIND response into a list of FetchedVTODO.
fn parse_multistatus_vtodos(xml: &str) -> Vec<FetchedVTODO> {
    let mut results = Vec::new();

    // Split on <response> boundaries (handles D:response or response)
    // We look for both namespace-prefixed and bare variants
    let response_open_variants = ["<D:response>", "<response>"];
    let response_close_variants = ["</D:response>", "</response>"];

    // Find which variant is used
    let (open_tag, close_tag) = {
        let mut found = ("<D:response>", "</D:response>");
        for (o, c) in response_open_variants.iter().zip(response_close_variants.iter()) {
            if xml.contains(o) {
                found = (o, c);
                break;
            }
        }
        found
    };

    let mut remaining = xml;
    while let Some(start) = remaining.find(open_tag) {
        let after_open = start + open_tag.len();
        if let Some(end) = remaining[after_open..].find(close_tag) {
            let response_xml = &remaining[start + open_tag.len()..after_open + end];

            // Extract href
            let href = extract_tag_content(response_xml, "href")
                .map(|s| s.trim().to_string());

            // Extract etag (getetag)
            let etag = extract_tag_content(response_xml, "getetag")
                .map(|s| s.trim().to_string());

            // Extract calendar-data — try multiple tag name forms
            let calendar_data = extract_calendar_data(response_xml);

            if let (Some(href), Some(cal_data)) = (href, calendar_data) {
                if let Ok(vtodo) = crate::ical::from_icalendar_string(&cal_data) {
                    results.push(FetchedVTODO {
                        href,
                        etag: if etag.as_deref().map(|s| !s.is_empty()).unwrap_or(false) {
                            etag
                        } else {
                            None
                        },
                        vtodo,
                    });
                }
            }

            remaining = &remaining[after_open + end + close_tag.len()..];
        } else {
            break;
        }
    }

    results
}

/// Extract calendar-data content, handling various namespace prefix forms.
fn extract_calendar_data(xml: &str) -> Option<String> {
    // Try various forms of the calendar-data tag
    let tag_variants = [
        ("C:calendar-data", "/C:calendar-data"),
        ("calendar-data", "/calendar-data"),
    ];

    for (open_local, close_local) in &tag_variants {
        // Find opening tag (may have attributes like content-type="...")
        if let Some(open_pos) = find_tag_start(xml, open_local) {
            let tag_end = xml[open_pos..].find('>')?;
            let content_start = open_pos + tag_end + 1;
            let close_tag = format!("<{}>", close_local);
            if let Some(close_pos) = xml[content_start..].find(&close_tag) {
                return Some(xml[content_start..content_start + close_pos].to_string());
            }
        }
    }
    None
}

/// Find the position of an opening tag (ignoring any attributes).
fn find_tag_start(xml: &str, tag_name: &str) -> Option<usize> {
    let search = format!("<{}", tag_name);
    xml.find(&search)
}

// ---------------------------------------------------------------------------
// iCal text parser (legacy; production code now delegates to ical::from_icalendar_string)
// Kept here so that caldav_adapter unit tests can exercise these helpers directly.
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn parse_vtodo_from_ical(ical: &str) -> Option<VTODO> {
    // Unfold continuation lines (lines starting with space or tab continue previous)
    let unfolded = unfold_ical(ical);

    // Find BEGIN:VTODO ... END:VTODO
    let vtodo_start = unfolded.find("BEGIN:VTODO")?;
    let after_begin = vtodo_start + "BEGIN:VTODO".len();
    let vtodo_end = unfolded[after_begin..].find("END:VTODO")?;
    let vtodo_block = &unfolded[after_begin..after_begin + vtodo_end];

    let mut uid = None;
    let mut summary = None;
    let mut description = None;
    let mut status = None;
    let mut last_modified = None;
    let mut dtstart = None;
    let mut due = None;
    let mut completed = None;
    let mut categories: Vec<String> = vec![];
    let mut rrule = None;

    for line in vtodo_block.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }

        // Split on first ':' to get property name (with possible params) and value
        if let Some(colon_pos) = line.find(':') {
            let prop_part = &line[..colon_pos];
            let value = &line[colon_pos + 1..];

            // Property name is the part before ';' (params follow ';')
            let prop_name = prop_part.split(';').next().unwrap_or(prop_part).trim();

            match prop_name.to_uppercase().as_str() {
                "UID" => uid = Some(value.to_string()),
                "SUMMARY" => summary = Some(unescape_ical(value)),
                "DESCRIPTION" => description = Some(unescape_ical(value)),
                "STATUS" => status = Some(value.to_string()),
                "LAST-MODIFIED" => last_modified = parse_ical_datetime(value),
                "DTSTART" => dtstart = parse_ical_datetime(value),
                "DUE" => due = parse_ical_datetime(value),
                "COMPLETED" => completed = parse_ical_datetime(value),
                "CATEGORIES" => {
                    // May be comma-separated
                    for cat in value.split(',') {
                        let cat = cat.trim().to_string();
                        if !cat.is_empty() {
                            categories.push(cat);
                        }
                    }
                }
                "RRULE" => rrule = Some(value.to_string()),
                _ => {}
            }
        }
    }

    let uid = uid?;

    Some(VTODO {
        uid,
        summary,
        description,
        status,
        last_modified,
        dtstamp: None,
        dtstart,
        due,
        completed,
        categories,
        rrule,
        priority: None,
        depends: vec![],
        extra_props: vec![],
    })
}

#[allow(dead_code)]
/// Unfold iCal continuation lines (RFC 5545: lines folded at 75 chars, continued with CRLF + space/tab).
fn unfold_ical(ical: &str) -> String {
    let mut result = String::with_capacity(ical.len());
    let mut chars = ical.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\r' {
            // Consume optional \n
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            // Check if next char is space or tab (continuation)
            if chars.peek() == Some(&' ') || chars.peek() == Some(&'\t') {
                chars.next(); // skip the whitespace continuation character
                              // Do not append newline — this is a continuation
            } else {
                result.push('\n');
            }
        } else if c == '\n' {
            // Check if next char is space or tab (continuation)
            if chars.peek() == Some(&' ') || chars.peek() == Some(&'\t') {
                chars.next(); // skip whitespace
            } else {
                result.push('\n');
            }
        } else {
            result.push(c);
        }
    }

    result
}

#[allow(dead_code)]
/// Unescape iCal text values (\n → newline, \, → comma, \; → semicolon, \\ → backslash).
fn unescape_ical(s: &str) -> String {
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

#[allow(dead_code)]
/// Parse an iCal datetime string into a UTC DateTime.
/// Handles formats:
/// - `YYYYMMDDTHHMMSSZ` (UTC)
/// - `YYYYMMDDTHHMMSS` (floating, treated as UTC)
/// - `YYYYMMDD` (date-only, treated as midnight UTC)
fn parse_ical_datetime(s: &str) -> Option<DateTime<Utc>> {
    let s = s.trim();
    // Strip any trailing 'Z'
    let (s_stripped, _is_utc) = if s.ends_with('Z') {
        (&s[..s.len() - 1], true)
    } else {
        (s, false)
    };

    // Try YYYYMMDDTHHMMSS
    if s_stripped.len() == 15 && s_stripped.contains('T') {
        NaiveDateTime::parse_from_str(s_stripped, "%Y%m%dT%H%M%S")
            .ok()
            .map(|ndt| ndt.and_utc())
    } else if s_stripped.len() == 8 && !s_stripped.contains('T') {
        // Date-only: YYYYMMDD → midnight UTC
        NaiveDateTime::parse_from_str(&format!("{}T000000", s_stripped), "%Y%m%dT%H%M%S")
            .ok()
            .map(|ndt| ndt.and_utc())
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// map_reqwest_error helper
// ---------------------------------------------------------------------------

fn map_reqwest_error(e: reqwest::Error, url: &str) -> CaldaWarriorError {
    let msg = e.to_string().to_lowercase();
    if msg.contains("tls") || msg.contains("certificate") || msg.contains("ssl") {
        CaldaWarriorError::CalDav {
            status: 0,
            body: format!(
                "TLS error connecting to {}: {}. If using a self-signed certificate, set allow_insecure_tls = true in config.",
                url, e
            ),
        }
    } else {
        CaldaWarriorError::CalDav {
            status: 0,
            body: format!("Request to {} failed: {}", url, e),
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FetchedVTODO, VTODO};

    fn make_vtodo(uid: &str) -> VTODO {
        VTODO {
            uid: uid.to_string(),
            summary: Some("Test".to_string()),
            description: None,
            status: Some("NEEDS-ACTION".to_string()),
            last_modified: None,
            dtstamp: None,
            dtstart: None,
            due: None,
            completed: None,
            categories: vec![],
            rrule: None,
            priority: None,
            depends: vec![],
            extra_props: vec![],
        }
    }

    fn make_fetched(uid: &str, href: &str) -> FetchedVTODO {
        FetchedVTODO {
            href: href.to_string(),
            etag: Some("\"abc123\"".to_string()),
            vtodo: make_vtodo(uid),
        }
    }

    #[test]
    fn mock_list_returns_queued_response() {
        let mock = MockCalDavClient::new();
        let expected = vec![make_fetched("uid-1", "/cal/uid-1.ics")];
        mock.list_responses.lock().unwrap().push(Ok(expected.clone()));

        let result = mock
            .list_vtodos("https://dav.example.com/alice/default/")
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].vtodo.uid, "uid-1");
    }

    #[test]
    fn mock_put_records_call_with_etag() {
        let mock = MockCalDavClient::new();
        mock.put_vtodo("/cal/test.ics", "BEGIN:VCALENDAR...", Some("\"abc\""))
            .unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        match &calls[0] {
            CalDavCall::Put { href, etag } => {
                assert_eq!(href, "/cal/test.ics");
                assert_eq!(etag.as_deref(), Some("\"abc\""));
            }
            _ => panic!("expected Put call"),
        }
    }

    #[test]
    fn mock_delete_records_call() {
        let mock = MockCalDavClient::new();
        mock.delete_vtodo("/cal/test.ics", Some("\"etag1\"")).unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        match &calls[0] {
            CalDavCall::Delete { href, etag } => {
                assert_eq!(href, "/cal/test.ics");
                assert_eq!(etag.as_deref(), Some("\"etag1\""));
            }
            _ => panic!("expected Delete call"),
        }
    }

    #[test]
    fn mock_list_returns_error() {
        let mock = MockCalDavClient::new();
        mock.list_responses
            .lock()
            .unwrap()
            .push(Err(CaldaWarriorError::Auth {
                server_url: "https://dav.example.com".to_string(),
            }));

        let result = mock.list_vtodos("https://dav.example.com/cal/");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Authentication"));
    }

    #[test]
    fn ical_text_parser_extracts_vtodo() {
        let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\nUID:test-uid-123\r\nSUMMARY:My task\r\nSTATUS:NEEDS-ACTION\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";
        let vtodo = parse_vtodo_from_ical(ical).expect("parse");
        assert_eq!(vtodo.uid, "test-uid-123");
        assert_eq!(vtodo.summary.as_deref(), Some("My task"));
        assert_eq!(vtodo.status.as_deref(), Some("NEEDS-ACTION"));
    }

    #[test]
    fn ical_parser_handles_all_fields() {
        let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\nUID:full-uid-456\r\nSUMMARY:Full task\r\nDESCRIPTION:Some desc\r\nSTATUS:IN-PROCESS\r\nLAST-MODIFIED:20260226T120000Z\r\nDTSTART:20260226T100000Z\r\nDUE:20260227T120000Z\r\nCOMPLETED:20260228T080000Z\r\nCATEGORIES:work,personal\r\nRRULE:FREQ=WEEKLY\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";
        let vtodo = parse_vtodo_from_ical(ical).expect("parse");
        assert_eq!(vtodo.uid, "full-uid-456");
        assert_eq!(vtodo.summary.as_deref(), Some("Full task"));
        assert_eq!(vtodo.description.as_deref(), Some("Some desc"));
        assert_eq!(vtodo.status.as_deref(), Some("IN-PROCESS"));
        assert!(vtodo.last_modified.is_some());
        assert!(vtodo.dtstart.is_some());
        assert!(vtodo.due.is_some());
        assert!(vtodo.completed.is_some());
        assert_eq!(vtodo.categories, vec!["work", "personal"]);
        assert_eq!(vtodo.rrule.as_deref(), Some("FREQ=WEEKLY"));
    }

    #[test]
    fn ical_parser_returns_none_without_vtodo() {
        let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nEND:VCALENDAR\r\n";
        let result = parse_vtodo_from_ical(ical);
        assert!(result.is_none());
    }

    #[test]
    fn parse_ical_datetime_utc() {
        let dt = parse_ical_datetime("20260226T140000Z").expect("parse");
        assert_eq!(dt.format("%Y%m%d").to_string(), "20260226");
    }

    #[test]
    fn parse_ical_datetime_floating() {
        let dt = parse_ical_datetime("20260226T140000").expect("parse");
        assert_eq!(dt.format("%Y%m%d").to_string(), "20260226");
    }

    #[test]
    fn parse_ical_datetime_date_only() {
        let dt = parse_ical_datetime("20260226").expect("parse");
        assert_eq!(dt.format("%Y%m%d").to_string(), "20260226");
    }

    #[test]
    fn unescape_ical_newlines() {
        let s = unescape_ical("line1\\nline2");
        assert_eq!(s, "line1\nline2");
    }

    #[test]
    fn mock_empty_responses_default_to_ok() {
        let mock = MockCalDavClient::new();
        // No responses queued — should return empty/None/Ok defaults
        let list = mock.list_vtodos("https://example.com/").unwrap();
        assert!(list.is_empty());

        let put = mock.put_vtodo("/cal/x.ics", "...", None).unwrap();
        assert!(put.is_none());

        mock.delete_vtodo("/cal/x.ics", None).unwrap();
    }
}
