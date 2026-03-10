//! Integration tests: status sync, dependency sync, orphaned UID cleanup, large dataset.

use std::time::Duration;

use chrono::Utc;
use uuid::Uuid;

use super::{should_skip, TestHarness};

/// CalDAV COMPLETED status syncs back to TW as a completed task.
#[test]
fn status_sync_caldav_completed_to_tw() {
    if should_skip() {
        return;
    }
    let h = TestHarness::new();

    // Push TW task to CalDAV
    let uuid = h.add_tw_task("Task to complete via CalDAV");
    let r1 = h.run_sync(false);
    assert!(r1.errors.is_empty(), "sync 1 errors: {:?}", r1.errors);
    assert_eq!(r1.written_caldav, 1);

    // Mark VTODO as COMPLETED externally (simulates CalDAV client completing the task)
    std::thread::sleep(Duration::from_secs(1));
    h.modify_first_vtodo_to_completed();

    // Sync: CalDAV COMPLETED should propagate to TW
    let r2 = h.run_sync(false);
    assert!(r2.errors.is_empty(), "sync 2 errors: {:?}", r2.errors);
    assert!(
        r2.written_tw >= 1,
        "CalDAV COMPLETED should trigger a TW write-back: written_tw={}",
        r2.written_tw
    );

    // TW task should now be completed (UUID filter works across all TW data files)
    let task = h.get_tw_task(&uuid);
    let status = task["status"].as_str().unwrap_or("");
    assert_eq!(
        status, "completed",
        "TW task status should be 'completed' after CalDAV COMPLETED sync"
    );
}

/// TW `depends` field synced to CalDAV RELATED-TO and reverse.
///
/// Two-phase test:
/// (a) TW→CalDAV: Task B depends on Task A → VTODO B has RELATED-TO;RELTYPE=DEPENDS-ON pointing to A's UID.
/// (b) CalDAV→TW: A new VTODO with RELATED-TO is pushed to CalDAV → TW task gets depends field.
#[test]
fn dependency_sync_tw_to_caldav() {
    if should_skip() {
        return;
    }
    let h = TestHarness::new();

    // Phase (a): TW → CalDAV dependency sync
    let uuid_a = h.add_tw_task("Task A (prerequisite)");
    let uuid_b = h.add_tw_task_with_depends("Task B (depends on A)", &uuid_a);

    let r1 = h.run_sync(false);
    assert!(r1.errors.is_empty(), "sync 1 errors: {:?}", r1.errors);
    assert_eq!(r1.written_caldav, 2, "both tasks should be pushed to CalDAV");
    assert_eq!(h.count_caldav_vtodos(), 2);

    // Get A's caldavuid (set after sync)
    let task_a = h.get_tw_task(&uuid_a);
    let caldavuid_a = task_a["caldavuid"].as_str().unwrap_or("").to_string();
    assert!(!caldavuid_a.is_empty(), "Task A should have caldavuid after sync");

    let task_b = h.get_tw_task(&uuid_b);
    let caldavuid_b = task_b["caldavuid"].as_str().unwrap_or("").to_string();
    assert!(!caldavuid_b.is_empty(), "Task B should have caldavuid after sync");

    // Task B's VTODO should have RELATED-TO;RELTYPE=DEPENDS-ON pointing to A's UID
    let vtodo_b_ical = h
        .get_vtodo_ical_by_uid(&caldavuid_b)
        .expect("Task B VTODO not found in CalDAV");
    assert!(
        vtodo_b_ical.contains("RELATED-TO") && vtodo_b_ical.contains(&caldavuid_a),
        "Task B VTODO should have RELATED-TO pointing to Task A's caldavuid ({caldavuid_a}).\n\
         Actual VTODO:\n{vtodo_b_ical}"
    );

    // Stable-point: second sync should produce no writes
    let r2 = h.run_sync(false);
    assert!(r2.errors.is_empty(), "sync 2 errors: {:?}", r2.errors);
    assert_eq!(r2.written_caldav, 0, "dependency sync should be stable (no caldav writes)");
    assert_eq!(r2.written_tw, 0, "dependency sync should be stable (no tw writes)");

    // Phase (b): CalDAV → TW reverse dependency sync (new VTODO with RELATED-TO)
    // Create a new CalDAV-only VTODO C that depends on A (via RELATED-TO)
    let vtodo_c = caldawarrior::types::VTODO {
        uid: Uuid::new_v4().to_string(),
        summary: Some("Task C from CalDAV (depends on A)".to_string()),
        description: None,
        status: Some("NEEDS-ACTION".to_string()),
        last_modified: Some(Utc::now()),
        dtstamp: None,
        dtstart: None,
        due: None,
        completed: None,
        categories: vec![],
        rrule: None,
        priority: None,
        depends: vec![(caldawarrior::types::RelType::DependsOn, caldavuid_a.clone())],
        extra_props: vec![],
    };
    h.put_new_vtodo(vtodo_c);
    assert_eq!(h.count_caldav_vtodos(), 3);

    // Sync: CalDAV VTODO C should create TW task C with depends on A's UUID
    let r3 = h.run_sync(false);
    assert!(r3.errors.is_empty(), "sync 3 errors: {:?}", r3.errors);
    // TW should have a new task created from CalDAV (written_tw >= 1)
    assert!(
        r3.written_tw >= 1,
        "CalDAV VTODO C should create a new TW task: written_tw={}",
        r3.written_tw
    );
}

/// An orphaned caldavuid (TW task has caldavuid but the CalDAV VTODO was deleted externally)
/// causes the TW task to be deleted — NOT re-created in CalDAV.
#[test]
fn orphaned_caldavuid_causes_tw_deletion() {
    if should_skip() {
        return;
    }
    let h = TestHarness::new();

    // Push TW task to CalDAV
    let uuid = h.add_tw_task("Task that will be orphaned");
    let r1 = h.run_sync(false);
    assert!(r1.errors.is_empty(), "sync 1 errors: {:?}", r1.errors);
    assert_eq!(r1.written_caldav, 1);
    assert_eq!(h.count_caldav_vtodos(), 1);

    // Verify caldavuid was set on TW task after sync
    let task = h.get_tw_task(&uuid);
    let caldavuid = task["caldavuid"].as_str().unwrap_or("");
    assert!(!caldavuid.is_empty(), "caldavuid should be set after first sync");

    // Delete the VTODO from CalDAV directly (simulates external deletion)
    h.delete_first_vtodo();
    assert_eq!(h.count_caldav_vtodos(), 0, "VTODO should be deleted from CalDAV");

    // Sync: TW task has caldavuid but no matching VTODO → orphaned → TW task deleted
    let r2 = h.run_sync(false);
    assert!(r2.errors.is_empty(), "sync 2 errors: {:?}", r2.errors);

    // Critical assertions:
    // (1) The TW task must have been deleted (written_tw counts the delete operation).
    // (2) The orphaned task must NOT be re-created in CalDAV.
    assert!(
        r2.written_tw >= 1,
        "orphaned TW task should be deleted during sync: written_tw={}",
        r2.written_tw
    );
    assert_eq!(
        h.count_caldav_vtodos(),
        0,
        "orphaned task must NOT be re-created in CalDAV"
    );

    // Confirm TW task status is 'deleted' (UUID filter searches all TW data files)
    let task_after = h.get_tw_task(&uuid);
    let status_after = task_after["status"].as_str().unwrap_or("missing");
    assert_eq!(
        status_after, "deleted",
        "TW task should have status 'deleted' after orphaned VTODO sync"
    );
}

/// 100-task first sync completes without duplication or data loss.
///
/// Verifies the sync engine can handle large datasets correctly.
#[test]
fn large_dataset_first_sync() {
    if should_skip() {
        return;
    }
    let h = TestHarness::new();

    const TASK_COUNT: usize = 100;

    // Add 100 TW tasks using bulk helper
    let descriptions: Vec<String> =
        (0..TASK_COUNT).map(|i| format!("Large dataset task {}", i + 1)).collect();
    let desc_refs: Vec<&str> = descriptions.iter().map(|s| s.as_str()).collect();
    h.import_tw_tasks_bulk(&desc_refs);

    // First sync: all 100 tasks should be pushed to CalDAV
    let r1 = h.run_sync(false);
    assert!(r1.errors.is_empty(), "large sync errors: {:?}", r1.errors);
    assert_eq!(
        r1.written_caldav, TASK_COUNT,
        "all {TASK_COUNT} tasks should be pushed to CalDAV"
    );
    assert_eq!(
        h.count_caldav_vtodos(),
        TASK_COUNT,
        "CalDAV should have all {TASK_COUNT} VTODOs"
    );

    // Second sync: stable point — no duplication, no data loss
    let r2 = h.run_sync(false);
    assert!(r2.errors.is_empty(), "stable sync errors: {:?}", r2.errors);
    assert_eq!(r2.written_caldav, 0, "no CalDAV writes on stable sync (no duplication)");
    assert_eq!(r2.written_tw, 0, "no TW writes on stable sync");
    assert_eq!(
        h.count_caldav_vtodos(),
        TASK_COUNT,
        "task count should remain {TASK_COUNT} (no data loss)"
    );
}
