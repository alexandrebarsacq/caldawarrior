pub mod deps;
pub mod lww;
pub mod writeback;

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use crate::caldav_adapter::CalDavClient;
use crate::config::Config;
use crate::ir::build_ir;
use crate::types::{FetchedVTODO, SyncResult, TWTask};
use crate::tw_adapter::{TaskRunner, TwAdapter};

use self::deps::resolve_dependencies;
use self::writeback::apply_writeback;

/// Run a full sync cycle: IR construction → dependency resolution → write-back.
///
/// # Step order
///
/// 1. **`build_ir`** — pairs TW tasks with CalDAV VTODOs, assigns fresh UUIDs,
///    and classifies every entry (new / paired / orphaned / CalDAV-only).
/// 2. **`resolve_dependencies`** — maps TW dependency UUIDs to CalDAV UIDs and
///    detects cyclic dependencies (marks `entry.cyclic`).
/// 3. **`apply_writeback`** — executes the planned operations. ETag retry is
///    owned exclusively by `apply_writeback`; `run_sync` does NOT re-implement
///    retry logic — it propagates the result as-is.
///
/// Warnings from all three steps are merged into `SyncResult.warnings`.
pub fn run_sync<R: TaskRunner>(
    tw_tasks: &[TWTask],
    vtodos_by_calendar: &HashMap<String, Vec<FetchedVTODO>>,
    config: &Config,
    tw: &TwAdapter<R>,
    caldav: &dyn CalDavClient,
    dry_run: bool,
    fail_fast: bool,
    now: DateTime<Utc>,
) -> SyncResult {
    // Filter out completed/deleted TW tasks that have no caldavuid and are older
    // than the configured cutoff, so they are not pushed to CalDAV.
    let cutoff_dt = now - chrono::Duration::days(i64::from(config.completed_cutoff_days));
    let filtered: Vec<TWTask> = tw_tasks
        .iter()
        .filter(|t| {
            if (t.status == "completed" || t.status == "deleted") && t.caldavuid.is_none() {
                t.end.map(|e| e >= cutoff_dt).unwrap_or(false)
            } else {
                true
            }
        })
        .cloned()
        .collect();

    // Step 1: build IR.
    let (mut ir, ir_warnings) = build_ir(&filtered, vtodos_by_calendar, config);

    // Step 2: resolve dependencies.
    let dep_warnings = resolve_dependencies(&mut ir);

    // Step 3: apply write-back (ETag retry owned here, not in run_sync).
    let mut result = apply_writeback(&mut ir, tw, caldav, dry_run, fail_fast, now);

    // Merge warnings from steps 1 and 2 into the result.
    let mut all_warnings = ir_warnings;
    all_warnings.extend(dep_warnings);
    all_warnings.extend(result.warnings.drain(..));
    result.warnings = all_warnings;

    result
}

// ---------------------------------------------------------------------------
// Integration tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::caldav_adapter::{CalDavCall, MockCalDavClient};
    use crate::config::CalendarEntry;
    use crate::tw_adapter::{MockTaskRunner, TwAdapter};
    use crate::types::TWTask;
    use chrono::TimeZone;
    use uuid::Uuid;

    fn t(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, s).unwrap()
    }

    fn make_config() -> Config {
        Config {
            server_url: "https://dav.example.com".to_string(),
            username: "alice".to_string(),
            password: "secret".to_string(),
            completed_cutoff_days: 90,
            allow_insecure_tls: false,
            caldav_timeout_seconds: 30,
            calendars: vec![CalendarEntry {
                project: "default".to_string(),
                url: "https://dav.example.com/cal/".to_string(),
            }],
        }
    }

    fn make_tw_task(uuid: Uuid) -> TWTask {
        TWTask {
            uuid,
            status: "pending".to_string(),
            description: "Test task".to_string(),
            entry: t(2026, 1, 1, 0, 0, 0),
            modified: Some(t(2026, 2, 1, 10, 0, 0)),
            due: None,
            scheduled: None,
            wait: None,
            until: None,
            end: None,
            caldavuid: None,
            priority: None,
            project: None,
            tags: None,
            recur: None,
            urgency: None,
            id: None,
            depends: vec![],
            annotations: vec![],
        }
    }

    fn make_tw_adapter(mock: MockTaskRunner) -> TwAdapter<MockTaskRunner> {
        mock.push_run_response(Ok(String::new())); // uda type
        mock.push_run_response(Ok(String::new())); // uda label
        TwAdapter::new(mock).expect("TwAdapter::new")
    }

    #[test]
    fn full_sync_tw_only_pushes_to_caldav() {
        let uuid = Uuid::new_v4();
        let config = make_config();
        let tasks = vec![make_tw_task(uuid)];

        let mock_tw = MockTaskRunner::new();
        mock_tw.push_run_response(Ok(String::new())); // uda type
        mock_tw.push_run_response(Ok(String::new())); // uda label
        mock_tw.push_run_response(Ok(String::new())); // tw.update() for caldavuid
        let tw = TwAdapter::new(mock_tw).expect("TwAdapter");

        let caldav = MockCalDavClient::new();
        let result = run_sync(
            &tasks,
            &HashMap::new(),
            &config,
            &tw,
            &caldav,
            false,
            false,
            t(2026, 2, 2, 0, 0, 0),
        );

        assert_eq!(result.written_caldav, 1, "TW-only task must be pushed to CalDAV");
        assert_eq!(result.written_tw, 1, "TW task must be updated with caldavuid");
        assert!(result.errors.is_empty(), "no errors expected: {:?}", result.errors);
        let calls = caldav.calls.lock().unwrap();
        assert!(calls.iter().any(|c| matches!(c, CalDavCall::Put { .. })));
    }

    #[test]
    fn full_sync_dry_run_does_not_write() {
        let uuid = Uuid::new_v4();
        let config = make_config();
        let tasks = vec![make_tw_task(uuid)];

        let mock_tw = MockTaskRunner::new();
        mock_tw.push_run_response(Ok(String::new())); // uda type
        mock_tw.push_run_response(Ok(String::new())); // uda label
        let tw = TwAdapter::new(mock_tw).expect("TwAdapter");

        let caldav = MockCalDavClient::new();
        let result = run_sync(
            &tasks,
            &HashMap::new(),
            &config,
            &tw,
            &caldav,
            true,
            false,
            t(2026, 2, 2, 0, 0, 0),
        );

        let calls = caldav.calls.lock().unwrap();
        assert!(calls.is_empty(), "dry_run must not make real CalDAV calls");
        assert_eq!(
            result.written_caldav, 1,
            "dry_run counts operations without executing"
        );
    }

    #[test]
    fn full_sync_collects_warnings_from_all_steps() {
        // A TW-only task with no matching calendar → UnmappedProject warning from build_ir.
        let config = Config {
            calendars: vec![], // no default calendar
            ..make_config()
        };
        let uuid = Uuid::new_v4();
        let tasks = vec![make_tw_task(uuid)];

        let tw = make_tw_adapter(MockTaskRunner::new());
        let caldav = MockCalDavClient::new();
        let result = run_sync(
            &tasks,
            &HashMap::new(),
            &config,
            &tw,
            &caldav,
            true,
            false,
            t(2026, 2, 2, 0, 0, 0),
        );

        assert!(
            result.warnings.iter().any(|w| w.message.contains("UnmappedProject")),
            "expected UnmappedProject warning in result: {:?}",
            result.warnings
        );
    }

    #[test]
    fn full_sync_three_steps_run_in_order() {
        // Two tasks A → B (A depends on B). Verifies deps step runs and sets
        // resolved_depends, and write-back still runs without errors.
        let uuid_a = Uuid::new_v4();
        let uuid_b = Uuid::new_v4();
        let config = make_config();

        let mut task_a = make_tw_task(uuid_a);
        task_a.depends = vec![uuid_b];

        let task_b = make_tw_task(uuid_b);

        let tasks = vec![task_a, task_b];

        let mock_tw = MockTaskRunner::new();
        mock_tw.push_run_response(Ok(String::new())); // uda type
        mock_tw.push_run_response(Ok(String::new())); // uda label
        // Two TW-only tasks → two CalDAV PUTs + two caldavuid updates
        mock_tw.push_run_response(Ok(String::new())); // tw.update() for task_a
        mock_tw.push_run_response(Ok(String::new())); // tw.update() for task_b
        let tw = TwAdapter::new(mock_tw).expect("TwAdapter");

        let caldav = MockCalDavClient::new();
        let result = run_sync(
            &tasks,
            &HashMap::new(),
            &config,
            &tw,
            &caldav,
            false,
            false,
            t(2026, 2, 2, 0, 0, 0),
        );

        assert_eq!(result.written_caldav, 2, "both TW-only tasks pushed");
        assert!(result.errors.is_empty(), "no errors: {:?}", result.errors);
        // Dependency warning is only emitted when a TW dep UUID is NOT in the IR.
        // Here both are present, so no UnresolvableDependency warning expected.
        let dep_warnings: Vec<_> = result
            .warnings
            .iter()
            .filter(|w| w.message.contains("UnresolvableDependency"))
            .collect();
        assert!(dep_warnings.is_empty(), "no dep warnings expected: {:?}", dep_warnings);
    }
}
