use crate::types::FetchedVTODO;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CaldaWarriorError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("TaskWarrior exited with code {code}: {stderr}")]
    Tw { code: i32, stderr: String },

    #[error("CalDAV request failed with status {status}: {body}")]
    CalDav { status: u16, body: String },

    #[error("Authentication failed for {server_url}: check your credentials in the config file")]
    Auth { server_url: String },

    #[error("iCalendar parse error: {0}")]
    IcalParse(String),

    #[error("Sync conflict could not be resolved automatically")]
    SyncConflict,

    #[error("ETag conflict: server resource changed during sync; refetched VTODO is attached for retry")]
    EtagConflict { refetched_vtodo: FetchedVTODO },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FetchedVTODO, VTODO};

    #[test]
    fn config_error_display() {
        let e = CaldaWarriorError::Config("missing field".to_string());
        assert!(e.to_string().contains("missing field"));
    }

    #[test]
    fn tw_error_display() {
        let e = CaldaWarriorError::Tw { code: 1, stderr: "not found".to_string() };
        assert!(e.to_string().contains("1"));
        assert!(e.to_string().contains("not found"));
    }

    #[test]
    fn auth_error_directs_to_credentials() {
        let e = CaldaWarriorError::Auth { server_url: "https://dav.example.com".to_string() };
        let msg = e.to_string();
        assert!(msg.contains("dav.example.com"));
        assert!(msg.to_lowercase().contains("credential"));
    }

    #[test]
    fn etag_conflict_carries_vtodo() {
        let vtodo = VTODO {
            uid: "test-uid".to_string(),
            ..Default::default()
        };
        let fetched = FetchedVTODO {
            href: "/cal/test.ics".to_string(),
            etag: Some("\"abc123\"".to_string()),
            vtodo,
        };
        let e = CaldaWarriorError::EtagConflict { refetched_vtodo: fetched.clone() };
        assert!(e.to_string().contains("ETag"));
        // Verify we can destructure and access the inner value
        if let CaldaWarriorError::EtagConflict { refetched_vtodo } = e {
            assert_eq!(refetched_vtodo.href, "/cal/test.ics");
        } else {
            panic!("wrong variant");
        }
    }
}
