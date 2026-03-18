//! Integration test harness for caldawarrior.
//!
//! Requires Docker:
//! - Radicale CalDAV server via `docker-compose.yml`.
//! - TaskWarrior via a locally-built image (`Dockerfile.taskwarrior`).
//!
//! Tests are skipped automatically when Docker is unavailable or when
//! `SKIP_INTEGRATION_TESTS=1` is set.

mod test_first_sync;
mod test_lww;
mod test_scenarios;

use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use tempfile::TempDir;
use uuid::Uuid;

use caldawarrior::caldav_adapter::{CalDavClient, RealCalDavClient};
use caldawarrior::config::{CalendarEntry, Config};
use caldawarrior::error::CaldaWarriorError;
use caldawarrior::sync::run_sync;
use caldawarrior::tw_adapter::{TaskRunner, TwAdapter};
use caldawarrior::types::SyncResult;
use chrono::Utc;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const RADICALE_URL: &str = "http://localhost:5233";
const RADICALE_USERNAME: &str = "testuser";
const RADICALE_PASSWORD: &str = "testpassword";
const COMPOSE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/integration");

/// Docker image name for the Dockerized TaskWarrior used in tests.
const TASKWARRIOR_IMAGE: &str = "caldawarrior-taskwarrior:latest";

// ---------------------------------------------------------------------------
// Skip guard
// ---------------------------------------------------------------------------

/// Returns `true` when integration tests should be skipped.
///
/// Only skips when `SKIP_INTEGRATION_TESTS` env var is set.
/// If Docker is unavailable the tests will fail loudly rather than silently pass.
pub fn should_skip() -> bool {
    std::env::var("SKIP_INTEGRATION_TESTS").is_ok()
}

// ---------------------------------------------------------------------------
// Docker lifecycle
// ---------------------------------------------------------------------------

static CONTAINER_STARTED: OnceLock<()> = OnceLock::new();
static TASKWARRIOR_IMAGE_READY: OnceLock<()> = OnceLock::new();

/// Start the Radicale container (idempotent; only runs once per test process).
///
/// Uses the `docker-compose.yml` in `tests/integration/`.
pub fn ensure_radicale_running() {
    CONTAINER_STARTED.get_or_init(|| {
        let status = Command::new("docker")
            .args(["compose", "up", "-d"])
            .current_dir(COMPOSE_DIR)
            .status()
            .expect("failed to run docker compose up");
        assert!(status.success(), "docker compose up -d failed");

        // Wait up to 30 s for Radicale to become reachable.
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .expect("reqwest client");
        for attempt in 0..30u32 {
            if client.get(RADICALE_URL).send().is_ok() {
                return;
            }
            if attempt < 29 {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
        panic!("Radicale did not become reachable within 30 s");
    });
}

/// Build the TaskWarrior Docker image (idempotent; only runs once per test process).
///
/// Builds from `Dockerfile.taskwarrior` in `tests/integration/` and tags
/// the result as `caldawarrior-taskwarrior:latest`.
pub fn ensure_taskwarrior_image() {
    TASKWARRIOR_IMAGE_READY.get_or_init(|| {
        let status = Command::new("docker")
            .args([
                "build",
                "-t",
                TASKWARRIOR_IMAGE,
                "-f",
                "Dockerfile.taskwarrior",
                ".",
            ])
            .current_dir(COMPOSE_DIR)
            .status()
            .expect("failed to run docker build for taskwarrior");
        assert!(status.success(), "docker build for taskwarrior failed");
    });
}

// ---------------------------------------------------------------------------
// DockerizedTaskRunner — TaskRunner backed by `docker run --rm`
// ---------------------------------------------------------------------------

/// A `TaskRunner` that executes every `task` command inside an isolated
/// container (`caldawarrior-taskwarrior:latest`).
///
/// The host `data_dir` is bind-mounted into the container as `/data`, so
/// task data persists on the host across separate `docker run` invocations
/// while providing a pinned, reproducible TaskWarrior version.
struct DockerizedTaskRunner {
    /// Host path mounted as `/data` inside the container (TASKDATA).
    data_dir: std::path::PathBuf,
}

impl DockerizedTaskRunner {
    /// Build the common `docker run --rm` argument prefix.
    fn docker_args(&self) -> Vec<String> {
        let vol = format!("{}:/data", self.data_dir.display());
        vec![
            "run".into(),
            "--rm".into(),
            "-v".into(),
            vol,
            "-e".into(),
            "TASKDATA=/data".into(),
            "-e".into(),
            "TASKRC=/data/.taskrc".into(),
            TASKWARRIOR_IMAGE.into(),
        ]
    }
}

impl TaskRunner for DockerizedTaskRunner {
    fn run(&self, args: &[&str]) -> Result<String, CaldaWarriorError> {
        let docker_args = self.docker_args();
        let output = Command::new("docker")
            .args(&docker_args)
            .args(args)
            .output()
            .map_err(|e| CaldaWarriorError::Config(format!("Failed to run dockerized task: {e}")))?;

        if !output.status.success() {
            let code = output.status.code().unwrap_or(-1);
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            return Err(CaldaWarriorError::Tw { code, stderr });
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    fn import(&self, json: &[u8]) -> Result<String, CaldaWarriorError> {
        let docker_args = self.docker_args();
        let mut child = Command::new("docker")
            .args(&docker_args)
            .arg("import")
            // -i keeps stdin open so we can pipe JSON in
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| CaldaWarriorError::Config(format!("Failed to spawn dockerized task import: {e}")))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(json)
                .map_err(|e| CaldaWarriorError::Config(format!("Failed to write task import stdin: {e}")))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| CaldaWarriorError::Config(format!("Failed to wait for dockerized task import: {e}")))?;

        if !output.status.success() {
            let code = output.status.code().unwrap_or(-1);
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            return Err(CaldaWarriorError::Tw { code, stderr });
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

// ---------------------------------------------------------------------------
// TestHarness
// ---------------------------------------------------------------------------

/// An isolated integration test environment.
///
/// Each harness owns:
/// - A unique CalDAV calendar (UUID-based path under the running Radicale).
/// - An isolated TW database in a temporary directory (mounted into Docker).
///
/// On `Drop`, the CalDAV calendar collection is deleted.
pub struct TestHarness {
    /// The caldawarrior configuration pointing at this harness's calendar.
    pub config: Config,
    tw_dir: TempDir,
    http: reqwest::blocking::Client,
}

impl TestHarness {
    /// Create a new harness.
    ///
    /// Panics if Docker is unavailable — call `should_skip()` first to guard.
    pub fn new() -> Self {
        ensure_radicale_running();
        ensure_taskwarrior_image();

        let test_id = Uuid::new_v4();
        let calendar_url = format!("{RADICALE_URL}/{RADICALE_USERNAME}/{test_id}/");

        let http = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("reqwest client");

        // Create the CalDAV calendar collection via MKCOL with CalDAV XML body.
        // The XML body sets resourcetype to <C:calendar/>, which tells Radicale the
        // collection type — required so that owner_only rights grant 'w' (not 'W').
        let mkcol_body = concat!(
            r#"<?xml version="1.0" encoding="UTF-8"?>"#,
            r#"<D:mkcol xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">"#,
            r#"<D:set><D:prop><D:resourcetype>"#,
            r#"<D:collection/><C:calendar/>"#,
            r#"</D:resourcetype></D:prop></D:set></D:mkcol>"#,
        );
        let resp = http
            .request(
                reqwest::Method::from_bytes(b"MKCOL").unwrap(),
                &calendar_url,
            )
            .basic_auth(RADICALE_USERNAME, Some(RADICALE_PASSWORD))
            .header("Content-Type", "application/xml; charset=utf-8")
            .body(mkcol_body)
            .send()
            .expect("MKCOL request");
        let status = resp.status().as_u16();
        assert!(
            matches!(status, 201 | 204 | 405),
            "MKCOL {calendar_url} failed with status {status}"
        );

        // Set up isolated TW directory with a pre-configured .taskrc.
        // The directory is bind-mounted as /data inside the TaskWarrior container.
        let tw_dir = TempDir::new().expect("tempdir");
        std::fs::write(
            tw_dir.path().join(".taskrc"),
            "confirmation=no\nuda.caldavuid.type=string\nuda.caldavuid.label=CaldavUID\n",
        )
        .expect("write .taskrc");

        let config = Config {
            server_url: RADICALE_URL.to_string(),
            username: RADICALE_USERNAME.to_string(),
            password: RADICALE_PASSWORD.to_string(),
            completed_cutoff_days: 90,
            allow_insecure_tls: false,
            caldav_timeout_seconds: 10,
            calendars: vec![CalendarEntry {
                project: "default".to_string(),
                url: calendar_url,
            }],
        };

        Self { config, tw_dir, http }
    }

    /// The URL of this harness's CalDAV calendar.
    pub fn calendar_url(&self) -> &str {
        &self.config.calendars[0].url
    }

    // -----------------------------------------------------------------------
    // State management
    // -----------------------------------------------------------------------

    /// Reset all state: wipe CalDAV VTODOs and TW tasks.
    ///
    /// Call between logical test cases within a single test function.
    pub fn reset(&self) {
        self.wipe_caldav();
        self.wipe_tw();
    }

    /// Delete all VTODOs from the CalDAV calendar (one DELETE per .ics).
    fn wipe_caldav(&self) {
        for href in self.list_vtodo_hrefs() {
            let url = format!("{RADICALE_URL}{href}");
            let _ = self
                .http
                .delete(&url)
                .basic_auth(RADICALE_USERNAME, Some(RADICALE_PASSWORD))
                .send();
        }
    }

    /// Remove all TaskWarrior data files from the temp directory.
    fn wipe_tw(&self) {
        if let Ok(entries) = std::fs::read_dir(self.tw_dir.path()) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "data") {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }

    /// Count VTODOs currently in the CalDAV calendar.
    pub fn count_caldav_vtodos(&self) -> usize {
        self.list_vtodo_hrefs().len()
    }

    /// Fetch the first VTODO from the calendar as raw iCalendar text.
    ///
    /// Returns `(href, etag, ical_text)` or `None` if the calendar is empty.
    pub fn get_first_vtodo_raw(&self) -> Option<(String, String, String)> {
        let hrefs = self.list_vtodo_hrefs();
        let href = hrefs.into_iter().next()?;
        let url = format!("{RADICALE_URL}{href}");
        let resp = self
            .http
            .get(&url)
            .basic_auth(RADICALE_USERNAME, Some(RADICALE_PASSWORD))
            .send()
            .ok()?;
        let etag = resp
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let body = resp.text().ok()?;
        Some((href, etag, body))
    }

    /// Directly modify the first VTODO in the calendar on the CalDAV server.
    ///
    /// Changes `SUMMARY` and `DESCRIPTION` to `new_summary`, updates
    /// `LAST-MODIFIED` to the current UTC time, and preserves all other
    /// properties (including `X-CALDAWARRIOR-LAST-SYNC`).
    ///
    /// Uses a parse → mutate → serialize round-trip so that the resulting
    /// iCalendar text is always well-formed.
    pub fn modify_first_vtodo_summary(&self, new_summary: &str) {
        let (href, etag, ical_text) = self
            .get_first_vtodo_raw()
            .expect("no VTODO found to modify");
        let mut vtodo = caldawarrior::ical::from_icalendar_string(&ical_text)
            .expect("parse VTODO for modification");
        vtodo.summary = Some(new_summary.to_string());
        vtodo.last_modified = Some(Utc::now());
        let new_ical = caldawarrior::ical::to_icalendar_string(&vtodo);

        let url = format!("{RADICALE_URL}{href}");
        let resp = self
            .http
            .put(&url)
            .basic_auth(RADICALE_USERNAME, Some(RADICALE_PASSWORD))
            .header("Content-Type", "text/calendar; charset=utf-8")
            .header("If-Match", &etag)
            .body(new_ical)
            .send()
            .expect("PUT modified VTODO to CalDAV");
        let status = resp.status().as_u16();
        assert!(
            matches!(status, 200 | 201 | 204),
            "modify_first_vtodo_summary PUT failed with HTTP {status}"
        );
    }

    /// Mark a TW task as done via Docker (`task {uuid} done`).
    pub fn complete_tw_task(&self, uuid: &str) {
        let vol = format!("{}:/data", self.tw_dir.path().display());
        let output = Command::new("docker")
            .args([
                "run", "--rm",
                "-v", &vol,
                "-e", "TASKDATA=/data",
                "-e", "TASKRC=/data/.taskrc",
                TASKWARRIOR_IMAGE,
                uuid, "done",
            ])
            .output()
            .expect("docker run task done");
        assert!(
            output.status.success(),
            "task done failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// Add a TW task with a dependency and return its UUID.
    ///
    /// Like `add_tw_task` but also passes `depends:{depends_on_uuid}`.
    pub fn add_tw_task_with_depends(&self, description: &str, depends_on_uuid: &str) -> String {
        let vol = format!("{}:/data", self.tw_dir.path().display());
        let depends_arg = format!("depends:{depends_on_uuid}");
        let output = Command::new("docker")
            .args([
                "run", "--rm",
                "-v", &vol,
                "-e", "TASKDATA=/data",
                "-e", "TASKRC=/data/.taskrc",
                TASKWARRIOR_IMAGE,
                "add", description, &depends_arg,
            ])
            .output()
            .expect("docker run task add with depends");
        assert!(
            output.status.success(),
            "task add with depends failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        // Parse the numeric task ID from "Created task N."
        let stdout = String::from_utf8_lossy(&output.stdout);
        let task_id: u64 = stdout
            .lines()
            .find_map(|line| {
                let line = line.trim();
                line.strip_prefix("Created task ")
                    .and_then(|rest| rest.trim_end_matches('.').parse().ok())
            })
            .unwrap_or(1);

        // Export that specific task to get its UUID.
        let out = Command::new("docker")
            .args([
                "run", "--rm",
                "-v", &vol,
                "-e", "TASKDATA=/data",
                "-e", "TASKRC=/data/.taskrc",
                TASKWARRIOR_IMAGE,
                task_id.to_string().as_str(), "export",
            ])
            .output()
            .expect("docker run task export");
        let tasks: Vec<serde_json::Value> =
            serde_json::from_slice(&out.stdout).unwrap_or_default();
        tasks
            .first()
            .and_then(|t| t["uuid"].as_str())
            .unwrap_or_default()
            .to_string()
    }

    /// Modify the first VTODO in the calendar to COMPLETED status.
    ///
    /// Sets STATUS=COMPLETED, COMPLETED=now, LAST-MODIFIED=now and PUTs back.
    pub fn modify_first_vtodo_to_completed(&self) {
        let (href, etag, ical_text) = self
            .get_first_vtodo_raw()
            .expect("no VTODO found to mark completed");
        let mut vtodo = caldawarrior::ical::from_icalendar_string(&ical_text)
            .expect("parse VTODO for completion");
        vtodo.status = Some("COMPLETED".to_string());
        vtodo.completed = Some(Utc::now());
        vtodo.last_modified = Some(Utc::now());
        let new_ical = caldawarrior::ical::to_icalendar_string(&vtodo);

        let url = format!("{RADICALE_URL}{href}");
        let resp = self
            .http
            .put(&url)
            .basic_auth(RADICALE_USERNAME, Some(RADICALE_PASSWORD))
            .header("Content-Type", "text/calendar; charset=utf-8")
            .header("If-Match", &etag)
            .body(new_ical)
            .send()
            .expect("PUT completed VTODO to CalDAV");
        let status = resp.status().as_u16();
        assert!(
            matches!(status, 200 | 201 | 204),
            "modify_first_vtodo_to_completed PUT failed with HTTP {status}"
        );
    }

    /// Delete the first VTODO in the calendar via HTTP DELETE.
    pub fn delete_first_vtodo(&self) {
        let hrefs = self.list_vtodo_hrefs();
        let href = hrefs.into_iter().next().expect("no VTODO found to delete");
        let url = format!("{RADICALE_URL}{href}");
        let resp = self
            .http
            .delete(&url)
            .basic_auth(RADICALE_USERNAME, Some(RADICALE_PASSWORD))
            .send()
            .expect("DELETE VTODO from CalDAV");
        let status = resp.status().as_u16();
        assert!(
            matches!(status, 200 | 204),
            "delete_first_vtodo DELETE failed with HTTP {status}"
        );
    }

    /// Fetch the raw iCal text of the VTODO whose UID matches `uid`.
    ///
    /// Returns `None` if no VTODO with that UID is found.
    pub fn get_vtodo_ical_by_uid(&self, uid: &str) -> Option<String> {
        let search = format!("UID:{uid}");
        for href in self.list_vtodo_hrefs() {
            let url = format!("{RADICALE_URL}{href}");
            let resp = self
                .http
                .get(&url)
                .basic_auth(RADICALE_USERNAME, Some(RADICALE_PASSWORD))
                .send()
                .ok()?;
            let body = resp.text().ok()?;
            if body.contains(&search) {
                return Some(body);
            }
        }
        None
    }

    /// PUT a new VTODO to the CalDAV calendar and return the uid.
    ///
    /// The resource is created at `{calendar_url}{uid}.ics`.
    pub fn put_new_vtodo(&self, vtodo: caldawarrior::types::VTODO) -> String {
        let ical = caldawarrior::ical::to_icalendar_string(&vtodo);
        let href = format!("{}{}.ics", self.calendar_url(), vtodo.uid);
        let resp = self
            .http
            .put(&href)
            .basic_auth(RADICALE_USERNAME, Some(RADICALE_PASSWORD))
            .header("Content-Type", "text/calendar; charset=utf-8")
            .body(ical)
            .send()
            .expect("PUT new VTODO to CalDAV");
        let status = resp.status().as_u16();
        assert!(
            matches!(status, 200 | 201 | 204),
            "put_new_vtodo PUT failed with HTTP {status}"
        );
        vtodo.uid
    }

    /// Add multiple TW tasks in bulk (one Docker call per task, no export step).
    pub fn import_tw_tasks_bulk(&self, task_descs: &[&str]) {
        let vol = format!("{}:/data", self.tw_dir.path().display());
        for desc in task_descs {
            let output = Command::new("docker")
                .args([
                    "run", "--rm",
                    "-v", &vol,
                    "-e", "TASKDATA=/data",
                    "-e", "TASKRC=/data/.taskrc",
                    TASKWARRIOR_IMAGE,
                    "add", desc,
                ])
                .output()
                .expect("docker run task add bulk");
            assert!(
                output.status.success(),
                "bulk add failed for {:?}: {}",
                desc,
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    /// List href paths of all VTODO resources in the calendar.
    fn list_vtodo_hrefs(&self) -> Vec<String> {
        let propfind = r#"<?xml version="1.0"?><D:propfind xmlns:D="DAV:"><D:prop><D:getetag/></D:prop></D:propfind>"#;
        let resp = self
            .http
            .request(
                reqwest::Method::from_bytes(b"PROPFIND").unwrap(),
                self.calendar_url(),
            )
            .basic_auth(RADICALE_USERNAME, Some(RADICALE_PASSWORD))
            .header("Depth", "1")
            .header("Content-Type", "application/xml")
            .body(propfind)
            .send();
        let Ok(resp) = resp else { return vec![] };
        let body = resp.text().unwrap_or_default();
        parse_hrefs_from_multistatus(&body)
    }

    // -----------------------------------------------------------------------
    // TW helpers
    // -----------------------------------------------------------------------

    /// Add a TW task via Docker and return its UUID.
    pub fn add_tw_task(&self, description: &str) -> String {
        let vol = format!("{}:/data", self.tw_dir.path().display());
        let output = Command::new("docker")
            .args([
                "run", "--rm",
                "-v", &vol,
                "-e", "TASKDATA=/data",
                "-e", "TASKRC=/data/.taskrc",
                TASKWARRIOR_IMAGE,
                "add", description,
            ])
            .output()
            .expect("docker run task add");
        assert!(
            output.status.success(),
            "task add failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        // Parse the numeric task ID from "Created task N."
        let stdout = String::from_utf8_lossy(&output.stdout);
        let task_id: u64 = stdout
            .lines()
            .find_map(|line| {
                let line = line.trim();
                line.strip_prefix("Created task ")
                    .and_then(|rest| rest.trim_end_matches('.').parse().ok())
            })
            .unwrap_or(1);

        // Export that specific task to get its UUID.
        let out = Command::new("docker")
            .args([
                "run", "--rm",
                "-v", &vol,
                "-e", "TASKDATA=/data",
                "-e", "TASKRC=/data/.taskrc",
                TASKWARRIOR_IMAGE,
                task_id.to_string().as_str(), "export",
            ])
            .output()
            .expect("docker run task export");
        let tasks: Vec<serde_json::Value> =
            serde_json::from_slice(&out.stdout).unwrap_or_default();
        tasks
            .first()
            .and_then(|t| t["uuid"].as_str())
            .unwrap_or_default()
            .to_string()
    }

    /// Modify the description of an existing TW task via Docker.
    pub fn modify_tw_task_description(&self, uuid: &str, description: &str) {
        let vol = format!("{}:/data", self.tw_dir.path().display());
        let output = Command::new("docker")
            .args([
                "run", "--rm",
                "-v", &vol,
                "-e", "TASKDATA=/data",
                "-e", "TASKRC=/data/.taskrc",
                TASKWARRIOR_IMAGE,
                uuid, "modify",
                &format!("description:{description}"),
            ])
            .output()
            .expect("docker run task modify description");
        assert!(
            output.status.success(),
            "task modify description failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// Export a TW task by UUID via Docker and return its JSON object.
    pub fn get_tw_task(&self, uuid: &str) -> serde_json::Value {
        let vol = format!("{}:/data", self.tw_dir.path().display());
        let out = Command::new("docker")
            .args([
                "run", "--rm",
                "-v", &vol,
                "-e", "TASKDATA=/data",
                "-e", "TASKRC=/data/.taskrc",
                TASKWARRIOR_IMAGE,
                uuid, "export",
            ])
            .output()
            .expect("docker run task export");
        let tasks: Vec<serde_json::Value> =
            serde_json::from_slice(&out.stdout).unwrap_or_default();
        tasks.into_iter().next().unwrap_or(serde_json::Value::Null)
    }

    // -----------------------------------------------------------------------
    // Sync
    // -----------------------------------------------------------------------

    /// Run a full sync cycle and return the `SyncResult`.
    pub fn run_sync(&self, dry_run: bool) -> SyncResult {
        let runner = DockerizedTaskRunner {
            data_dir: self.tw_dir.path().to_path_buf(),
        };
        let tw = TwAdapter::new(runner).expect("TwAdapter::new");
        let caldav = RealCalDavClient::new(
            self.config.server_url.clone(),
            self.config.username.clone(),
            self.config.password.clone(),
            self.config.caldav_timeout_seconds,
            self.config.allow_insecure_tls,
        )
        .expect("RealCalDavClient::new");

        let tw_tasks = tw.list_all().expect("tw.list_all");
        let mut vtodos_by_calendar = HashMap::new();
        for cal in &self.config.calendars {
            let vtodos = caldav.list_vtodos(&cal.url).expect("list_vtodos");
            vtodos_by_calendar.insert(cal.url.clone(), vtodos);
        }

        run_sync(&tw_tasks, &vtodos_by_calendar, &self.config, &tw, &caldav, dry_run, false, Utc::now())
    }
}

impl Drop for TestHarness {
    fn drop(&mut self) {
        // Best-effort cleanup: delete the calendar collection.
        let _ = self
            .http
            .delete(self.calendar_url())
            .basic_auth(RADICALE_USERNAME, Some(RADICALE_PASSWORD))
            .send();
    }
}

// ---------------------------------------------------------------------------
// XML helper
// ---------------------------------------------------------------------------

/// Extract `.ics` href values from a CalDAV PROPFIND multistatus response.
pub(crate) fn parse_hrefs_from_multistatus(xml: &str) -> Vec<String> {
    let mut hrefs = Vec::new();
    let mut remaining = xml;
    loop {
        let (open_pos, open_tag_len, close_tag) =
            if let Some(p) = remaining.find("<D:href>") {
                (p, 8usize, "</D:href>")
            } else if let Some(p) = remaining.find("<href>") {
                (p, 6usize, "</href>")
            } else {
                break;
            };

        let content_start = open_pos + open_tag_len;
        if let Some(end) = remaining[content_start..].find(close_tag) {
            let href = remaining[content_start..content_start + end].trim();
            if href.ends_with(".ics") {
                hrefs.push(href.to_string());
            }
            remaining = &remaining[content_start + end + close_tag.len()..];
        } else {
            break;
        }
    }
    hrefs
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Pure unit tests (no Docker) ----------------------------------------

    #[test]
    fn parse_hrefs_extracts_ics_files_only() {
        let xml = r#"<D:multistatus>
            <D:response><D:href>/test/default/</D:href></D:response>
            <D:response><D:href>/test/default/task1.ics</D:href></D:response>
            <D:response><D:href>/test/default/task2.ics</D:href></D:response>
        </D:multistatus>"#;
        let hrefs = parse_hrefs_from_multistatus(xml);
        assert_eq!(hrefs.len(), 2);
        assert!(hrefs.contains(&"/test/default/task1.ics".to_string()));
        assert!(hrefs.contains(&"/test/default/task2.ics".to_string()));
    }

    #[test]
    fn parse_hrefs_empty_on_no_ics() {
        let xml = r#"<D:multistatus>
            <D:response><D:href>/test/default/</D:href></D:response>
        </D:multistatus>"#;
        let hrefs = parse_hrefs_from_multistatus(xml);
        assert!(hrefs.is_empty());
    }

    #[test]
    fn parse_hrefs_handles_bare_tags() {
        // Some servers use bare <href> without namespace prefix.
        let xml = "<multistatus><response><href>/cal/item.ics</href></response></multistatus>";
        let hrefs = parse_hrefs_from_multistatus(xml);
        assert_eq!(hrefs, vec!["/cal/item.ics"]);
    }

    // -- Integration tests (require Docker) ---------------------------------

    #[test]
    fn harness_creates_isolated_calendar_and_tw_dir() {
        if should_skip() {
            return;
        }
        let h = TestHarness::new();
        assert!(!h.calendar_url().is_empty());
        assert!(h.tw_dir.path().exists());
        assert!(h.tw_dir.path().join(".taskrc").exists());
    }

    #[test]
    fn harness_reset_clears_tw_task_data() {
        if should_skip() {
            return;
        }
        let h = TestHarness::new();
        // Simulate existing TaskWarrior data file in the data directory.
        let task_file = h.tw_dir.path().join("pending.data");
        std::fs::write(&task_file, b"[]\n").expect("write pending.data");
        assert!(task_file.exists());
        h.reset();
        assert!(!task_file.exists(), "reset should remove pending.data");
    }

    #[test]
    fn harness_add_tw_task_returns_uuid() {
        if should_skip() {
            return;
        }
        let h = TestHarness::new();
        let uuid = h.add_tw_task("Integration test task");
        assert!(!uuid.is_empty(), "expected non-empty UUID");
        // Basic UUID format check: 8-4-4-4-12
        assert_eq!(uuid.len(), 36, "UUID should be 36 chars: {uuid}");
    }
}
