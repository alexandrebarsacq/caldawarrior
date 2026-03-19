use crate::error::CaldaWarriorError;
use crate::types::FetchedVTODO;
use quick_xml::events::Event;
use quick_xml::name::{Namespace, ResolveResult};
use quick_xml::reader::NsReader;
use reqwest::blocking::Client;
use std::sync::Mutex;
use std::time::Duration;

// ---------------------------------------------------------------------------
// ETag normalization
// ---------------------------------------------------------------------------

/// Normalize an ETag value: strip W/ weak prefix and ensure double-quote wrapping.
/// Converts weak ETags to strong for use in If-Match headers (RFC 7232 section 2.3.2).
fn normalize_etag(raw: &str) -> String {
    let s = raw.trim();
    // Strip weak indicator (case-insensitive)
    let s = if s.starts_with("W/") || s.starts_with("w/") {
        &s[2..]
    } else {
        s
    };
    // Strip existing quotes, then re-wrap
    let s = s.trim_matches('"');
    format!("\"{}\"", s)
}

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
    ///
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
                    .map(normalize_etag);
                let body = resp.text().map_err(|e| CaldaWarriorError::CalDav {
                    status: 0,
                    body: format!("Failed to read response body: {}", e),
                })?;
                let vtodo = crate::ical::from_icalendar_string(&body).map_err(|e| {
                    CaldaWarriorError::IcalParse(format!(
                        "Could not parse VTODO from GET response for {}: {}",
                        href, e
                    ))
                })?;
                Ok(FetchedVTODO {
                    href: href.to_string(),
                    etag,
                    vtodo,
                })
            }
            401 => Err(CaldaWarriorError::Auth { server_url: url }),
            status => {
                let body = resp
                    .text()
                    .unwrap_or_else(|e| format!("<body unreadable: {}>", e));
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
            .request(
                reqwest::Method::from_bytes(b"REPORT").unwrap(),
                calendar_url,
            )
            .basic_auth(&self.username, Some(&self.password))
            .header("Content-Type", "application/xml; charset=utf-8")
            .header("Depth", "1")
            .body(report_body)
            .send()
            .map_err(|e| map_reqwest_error(e, calendar_url))?;

        match resp.status().as_u16() {
            200 | 207 => {
                let body = resp.text().map_err(|e| CaldaWarriorError::CalDav {
                    status: 0,
                    body: format!("Failed to read response body: {}", e),
                })?;
                Ok(parse_multistatus_vtodos(&body))
            }
            401 => Err(CaldaWarriorError::Auth {
                server_url: calendar_url.to_string(),
            }),
            status => {
                let body = resp
                    .text()
                    .unwrap_or_else(|e| format!("<body unreadable: {}>", e));
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
            req = req.header("If-Match", e);
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
                    .map(normalize_etag);
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
                let body = resp
                    .text()
                    .unwrap_or_else(|e| format!("<body unreadable: {}>", e));
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
            req = req.header("If-Match", e);
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
                let body = resp
                    .text()
                    .unwrap_or_else(|e| format!("<body unreadable: {}>", e));
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

impl Default for MockCalDavClient {
    fn default() -> Self {
        Self::new()
    }
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
// XML parsing helpers (namespace-aware, using quick-xml NsReader)
// ---------------------------------------------------------------------------

/// XML namespace URIs (from RFC 4791 and WebDAV).
const DAV_NS: &[u8] = b"DAV:";
const CALDAV_NS: &[u8] = b"urn:ietf:params:xml:ns:caldav";

/// Parse the XML multi-status REPORT/PROPFIND response into a list of FetchedVTODO.
///
/// Uses quick-xml NsReader for namespace-aware parsing, correctly handling any
/// namespace prefix (D:, d:, ns0:, bare default xmlns, etc.).
fn parse_multistatus_vtodos(xml: &str) -> Vec<FetchedVTODO> {
    let mut reader = NsReader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut results = Vec::new();

    // Per-response accumulators
    let mut in_response = false;
    let mut href = String::new();
    let mut etag = String::new();
    let mut calendar_data = String::new();

    // Which element are we currently reading text for?
    #[derive(PartialEq)]
    enum Reading {
        None,
        Href,
        Etag,
        CalendarData,
    }
    let mut reading = Reading::None;

    loop {
        match reader.read_resolved_event() {
            Ok((ResolveResult::Bound(Namespace(ns)), Event::Start(e))) => {
                let local = e.local_name();
                if ns == DAV_NS {
                    match local.as_ref() {
                        b"response" => {
                            in_response = true;
                            href.clear();
                            etag.clear();
                            calendar_data.clear();
                            reading = Reading::None;
                        }
                        b"href" if in_response => {
                            reading = Reading::Href;
                        }
                        b"getetag" if in_response => {
                            reading = Reading::Etag;
                        }
                        _ => {}
                    }
                } else if ns == CALDAV_NS && local.as_ref() == b"calendar-data" && in_response {
                    reading = Reading::CalendarData;
                }
            }
            Ok((ResolveResult::Bound(Namespace(ns)), Event::End(e))) => {
                let local = e.local_name();
                if ns == DAV_NS && local.as_ref() == b"response" && in_response {
                    // End of a <response> element -- try to build a FetchedVTODO
                    if !href.is_empty() && !calendar_data.is_empty() {
                        match crate::ical::from_icalendar_string(&calendar_data) {
                            Ok(vtodo) => {
                                let normalized_etag = if etag.is_empty() {
                                    None
                                } else {
                                    Some(normalize_etag(&etag))
                                };
                                results.push(FetchedVTODO {
                                    href: href.trim().to_string(),
                                    etag: normalized_etag,
                                    vtodo,
                                });
                            }
                            Err(e) => {
                                eprintln!(
                                    "Warning: skipping unparseable VTODO at {}: {}",
                                    href.trim(),
                                    e
                                );
                            }
                        }
                    }
                    in_response = false;
                    reading = Reading::None;
                } else if ns == DAV_NS {
                    match local.as_ref() {
                        b"href" if reading == Reading::Href => reading = Reading::None,
                        b"getetag" if reading == Reading::Etag => reading = Reading::None,
                        _ => {}
                    }
                } else if ns == CALDAV_NS
                    && local.as_ref() == b"calendar-data"
                    && reading == Reading::CalendarData
                {
                    reading = Reading::None;
                }
            }
            Ok((_, Event::Text(e))) => {
                if let Ok(text) = e.decode() {
                    match reading {
                        Reading::Href => href.push_str(&text),
                        Reading::Etag => etag.push_str(&text),
                        Reading::CalendarData => calendar_data.push_str(&text),
                        Reading::None => {}
                    }
                }
            }
            Ok((_, Event::CData(e))) => {
                if let Ok(text) = e.decode() {
                    match reading {
                        Reading::Href => href.push_str(&text),
                        Reading::Etag => etag.push_str(&text),
                        Reading::CalendarData => calendar_data.push_str(&text),
                        Reading::None => {}
                    }
                }
            }
            Ok((_, Event::Eof)) => break,
            Err(e) => {
                eprintln!("Warning: XML parse error in multistatus response: {}", e);
                break;
            }
            _ => {} // Skip processing instructions, comments, etc.
        }
    }

    results
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
            status: Some("NEEDS-ACTION".to_string()),
            ..Default::default()
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
        mock.list_responses
            .lock()
            .unwrap()
            .push(Ok(expected.clone()));

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
        mock.delete_vtodo("/cal/test.ics", Some("\"etag1\""))
            .unwrap();

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

    // -----------------------------------------------------------------------
    // ETag normalization tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_normalize_etag_weak() {
        assert_eq!(normalize_etag("W/\"abc123\""), "\"abc123\"");
    }

    #[test]
    fn test_normalize_etag_weak_lowercase() {
        assert_eq!(normalize_etag("w/\"abc123\""), "\"abc123\"");
    }

    #[test]
    fn test_normalize_etag_strong() {
        assert_eq!(normalize_etag("\"abc123\""), "\"abc123\"");
    }

    #[test]
    fn test_normalize_etag_bare() {
        assert_eq!(normalize_etag("abc123"), "\"abc123\"");
    }

    #[test]
    fn test_normalize_etag_weak_no_quotes() {
        assert_eq!(normalize_etag("W/abc123"), "\"abc123\"");
    }

    // -----------------------------------------------------------------------
    // XML parser tests (namespace-aware)
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_multistatus_radicale_format() {
        let xml = r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:response>
    <D:href>/cal/test-uid.ics</D:href>
    <D:propstat>
      <D:prop>
        <D:getetag>"etag123"</D:getetag>
        <C:calendar-data>BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VTODO
UID:test-uid
SUMMARY:Test task
END:VTODO
END:VCALENDAR</C:calendar-data>
      </D:prop>
    </D:propstat>
  </D:response>
</D:multistatus>"#;

        let results = parse_multistatus_vtodos(xml);
        assert_eq!(results.len(), 1, "should parse one VTODO");
        assert_eq!(results[0].href, "/cal/test-uid.ics");
        assert_eq!(results[0].etag.as_deref(), Some("\"etag123\""));
        assert_eq!(results[0].vtodo.uid, "test-uid");
        assert_eq!(results[0].vtodo.summary.as_deref(), Some("Test task"));
    }

    #[test]
    fn test_parse_multistatus_custom_ns() {
        let xml = r#"<?xml version="1.0"?>
<ns0:multistatus xmlns:ns0="DAV:" xmlns:cal="urn:ietf:params:xml:ns:caldav">
  <ns0:response>
    <ns0:href>/cal/custom-uid.ics</ns0:href>
    <ns0:propstat>
      <ns0:prop>
        <ns0:getetag>"custom-etag"</ns0:getetag>
        <cal:calendar-data>BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VTODO
UID:custom-uid
SUMMARY:Custom ns task
END:VTODO
END:VCALENDAR</cal:calendar-data>
      </ns0:prop>
    </ns0:propstat>
  </ns0:response>
</ns0:multistatus>"#;

        let results = parse_multistatus_vtodos(xml);
        assert_eq!(results.len(), 1, "should parse VTODO with custom ns prefix");
        assert_eq!(results[0].href, "/cal/custom-uid.ics");
        assert_eq!(results[0].etag.as_deref(), Some("\"custom-etag\""));
        assert_eq!(results[0].vtodo.uid, "custom-uid");
    }

    #[test]
    fn test_parse_multistatus_bare_ns() {
        let xml = r#"<?xml version="1.0"?>
<multistatus xmlns="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <response>
    <href>/cal/bare-uid.ics</href>
    <propstat>
      <prop>
        <getetag>"bare-etag"</getetag>
        <C:calendar-data>BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VTODO
UID:bare-uid
SUMMARY:Bare ns task
END:VTODO
END:VCALENDAR</C:calendar-data>
      </prop>
    </propstat>
  </response>
</multistatus>"#;

        let results = parse_multistatus_vtodos(xml);
        assert_eq!(
            results.len(),
            1,
            "should parse VTODO with bare ns (default xmlns)"
        );
        assert_eq!(results[0].href, "/cal/bare-uid.ics");
        assert_eq!(results[0].etag.as_deref(), Some("\"bare-etag\""));
        assert_eq!(results[0].vtodo.uid, "bare-uid");
    }

    #[test]
    fn test_parse_multistatus_cdata() {
        let xml = r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:response>
    <D:href>/cal/cdata-uid.ics</D:href>
    <D:propstat>
      <D:prop>
        <D:getetag>"cdata-etag"</D:getetag>
        <C:calendar-data><![CDATA[BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VTODO
UID:cdata-uid
SUMMARY:CDATA task
END:VTODO
END:VCALENDAR]]></C:calendar-data>
      </D:prop>
    </D:propstat>
  </D:response>
</D:multistatus>"#;

        let results = parse_multistatus_vtodos(xml);
        assert_eq!(results.len(), 1, "should parse VTODO from CDATA");
        assert_eq!(results[0].vtodo.uid, "cdata-uid");
    }

    #[test]
    fn test_parse_multistatus_skip_bad_vtodo() {
        let xml = r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:response>
    <D:href>/cal/good-uid.ics</D:href>
    <D:propstat>
      <D:prop>
        <D:getetag>"good-etag"</D:getetag>
        <C:calendar-data>BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VTODO
UID:good-uid
SUMMARY:Good task
END:VTODO
END:VCALENDAR</C:calendar-data>
      </D:prop>
    </D:propstat>
  </D:response>
  <D:response>
    <D:href>/cal/bad-uid.ics</D:href>
    <D:propstat>
      <D:prop>
        <D:getetag>"bad-etag"</D:getetag>
        <C:calendar-data>THIS IS NOT VALID ICAL DATA</C:calendar-data>
      </D:prop>
    </D:propstat>
  </D:response>
</D:multistatus>"#;

        let results = parse_multistatus_vtodos(xml);
        assert_eq!(results.len(), 1, "should skip bad VTODO and keep good one");
        assert_eq!(results[0].vtodo.uid, "good-uid");
    }

    #[test]
    fn test_parse_multistatus_multiple() {
        let xml = r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:response>
    <D:href>/cal/uid-1.ics</D:href>
    <D:propstat>
      <D:prop>
        <D:getetag>"etag-1"</D:getetag>
        <C:calendar-data>BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VTODO
UID:uid-1
SUMMARY:First task
END:VTODO
END:VCALENDAR</C:calendar-data>
      </D:prop>
    </D:propstat>
  </D:response>
  <D:response>
    <D:href>/cal/uid-2.ics</D:href>
    <D:propstat>
      <D:prop>
        <D:getetag>"etag-2"</D:getetag>
        <C:calendar-data>BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VTODO
UID:uid-2
SUMMARY:Second task
END:VTODO
END:VCALENDAR</C:calendar-data>
      </D:prop>
    </D:propstat>
  </D:response>
</D:multistatus>"#;

        let results = parse_multistatus_vtodos(xml);
        assert_eq!(results.len(), 2, "should parse both VTODOs");
        assert_eq!(results[0].vtodo.uid, "uid-1");
        assert_eq!(results[1].vtodo.uid, "uid-2");
    }

    #[test]
    fn test_parse_multistatus_large() {
        let mut xml = String::from(
            r#"<?xml version="1.0"?><D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">"#,
        );
        for i in 1..=25 {
            xml.push_str(&format!(
                r#"<D:response><D:href>/cal/uid-{i:03}.ics</D:href><D:propstat><D:prop><D:getetag>"etag-{i:03}"</D:getetag><C:calendar-data>BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VTODO
UID:uid-{i:03}
SUMMARY:Task {i}
END:VTODO
END:VCALENDAR</C:calendar-data></D:prop></D:propstat></D:response>"#
            ));
        }
        xml.push_str("</D:multistatus>");

        let results = parse_multistatus_vtodos(&xml);
        assert_eq!(results.len(), 25, "should parse all 25 VTODOs");
        assert_eq!(results[0].vtodo.uid, "uid-001");
        assert_eq!(results[24].vtodo.uid, "uid-025");
        // Verify no UIDs are missing
        for (i, fv) in results.iter().enumerate() {
            assert_eq!(fv.vtodo.uid, format!("uid-{:03}", i + 1));
        }
    }

    #[test]
    fn test_parse_multistatus_special_chars() {
        // Test Unicode characters and iCal-escaped content in VTODO.
        // Real Radicale responses embed iCal text directly (no XML entity escaping
        // needed for iCal content since it doesn't contain <, >, or &).
        let xml = "<?xml version=\"1.0\"?>\n\
<D:multistatus xmlns:D=\"DAV:\" xmlns:C=\"urn:ietf:params:xml:ns:caldav\">\n\
  <D:response>\n\
    <D:href>/cal/special-uid.ics</D:href>\n\
    <D:propstat>\n\
      <D:prop>\n\
        <D:getetag>\"special-etag\"</D:getetag>\n\
        <C:calendar-data>BEGIN:VCALENDAR\n\
VERSION:2.0\n\
BEGIN:VTODO\n\
UID:special-uid\n\
SUMMARY:Caf\u{00e9} meeting \u{2014} don\u{2019}t forget\n\
DESCRIPTION:Line one\\nLine two\\, with comma\n\
END:VTODO\n\
END:VCALENDAR</C:calendar-data>\n\
      </D:prop>\n\
    </D:propstat>\n\
  </D:response>\n\
</D:multistatus>";

        let results = parse_multistatus_vtodos(xml);
        assert_eq!(
            results.len(),
            1,
            "should parse VTODO with special characters"
        );
        assert_eq!(results[0].vtodo.uid, "special-uid");
        let summary = results[0].vtodo.summary.as_deref().unwrap_or("");
        // Unicode characters should be preserved through XML + iCal parsing
        assert!(
            summary.contains("Caf\u{00e9}"),
            "should preserve e-with-acute: {:?}",
            summary
        );
        assert!(
            summary.contains("\u{2014}"),
            "should preserve em-dash: {:?}",
            summary
        );
        assert!(
            summary.contains("\u{2019}"),
            "should preserve right single quote: {:?}",
            summary
        );
        // iCal-escaped newline and comma in DESCRIPTION
        let desc = results[0].vtodo.description.as_deref().unwrap_or("");
        assert!(
            desc.contains("Line one\nLine two"),
            "should unescape \\n in description: {:?}",
            desc
        );
        assert!(
            desc.contains(", with comma"),
            "should unescape \\, in description: {:?}",
            desc
        );
    }

    #[test]
    fn test_parse_multistatus_empty() {
        // Empty multistatus with no <response> elements
        let xml = r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
</D:multistatus>"#;

        let results = parse_multistatus_vtodos(xml);
        assert_eq!(results.len(), 0, "empty calendar should return empty vec");
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
