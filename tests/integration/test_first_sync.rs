//! Integration tests: first sync from TaskWarrior to CalDAV.

use super::{should_skip, TestHarness};

/// After syncing two TW tasks, both appear as VTODOs in CalDAV.
#[test]
fn first_sync_pushes_tw_tasks_to_caldav() {
    if should_skip() {
        return;
    }
    let h = TestHarness::new();
    h.add_tw_task("Buy groceries");
    h.add_tw_task("Write integration tests");

    let result = h.run_sync(false);

    assert!(result.errors.is_empty(), "sync errors: {:?}", result.errors);
    assert_eq!(h.count_caldav_vtodos(), 2, "expected 2 VTODOs in CalDAV after sync");
    assert_eq!(result.written_caldav, 2, "expected 2 CalDAV writes");
}

/// After syncing, the `caldavuid` UDA is set on the TW task.
#[test]
fn first_sync_sets_caldavuid_uda_on_tw_task() {
    if should_skip() {
        return;
    }
    let h = TestHarness::new();
    let uuid = h.add_tw_task("Task for caldavuid check");

    let result = h.run_sync(false);

    assert!(result.errors.is_empty(), "sync errors: {:?}", result.errors);

    let task_json = h.get_tw_task(&uuid);
    let caldavuid = task_json["caldavuid"].as_str().unwrap_or("");
    assert!(
        !caldavuid.is_empty(),
        "expected caldavuid UDA set on TW task after sync; task JSON: {task_json}"
    );
}

/// A dry-run sync must not write any VTODOs to CalDAV.
#[test]
fn first_sync_dry_run_does_not_write_vtodos() {
    if should_skip() {
        return;
    }
    let h = TestHarness::new();
    h.add_tw_task("Should not appear in CalDAV during dry run");

    let result = h.run_sync(true);

    assert!(result.errors.is_empty(), "sync errors: {:?}", result.errors);
    assert_eq!(h.count_caldav_vtodos(), 0, "dry run must not write VTODOs to CalDAV");
    assert!(!result.planned_ops.is_empty(), "expected at least one planned op in dry run");
}

/// TW tasks without an explicit project land in the calendar mapped to
/// `project = "default"` in the config.
#[test]
fn first_sync_project_mapping_routes_to_default_calendar() {
    if should_skip() {
        return;
    }
    let h = TestHarness::new();
    // The harness config maps project="default" → the single calendar URL.
    // A task added without a project is treated as the default project.
    h.add_tw_task("Project-less task");

    let result = h.run_sync(false);

    assert!(result.errors.is_empty(), "sync errors: {:?}", result.errors);
    assert_eq!(
        h.count_caldav_vtodos(),
        1,
        "task should be routed to the default calendar"
    );
}
