//! Integration tests: bidirectional LWW conflict resolution and loop prevention.

use std::time::Duration;

use super::{should_skip, TestHarness};

/// TW task modified after first sync → TW wins LWW → CalDAV updated.
#[test]
fn tw_wins_lww() {
    if should_skip() {
        return;
    }
    let h = TestHarness::new();

    // Initial push: TW → CalDAV.
    let uuid = h.add_tw_task("Original description");
    let r1 = h.run_sync(false);
    assert!(r1.errors.is_empty(), "sync 1 errors: {:?}", r1.errors);
    assert_eq!(r1.written_caldav, 1, "initial push should write 1 VTODO");

    // Modify TW description → TW.modified > X-CALDAWARRIOR-LAST-SYNC.
    std::thread::sleep(Duration::from_secs(1));
    h.modify_tw_task_description(&uuid, "Updated by TaskWarrior");

    // Sync 2: TW wins (modified > last_sync, non-identical content).
    let r2 = h.run_sync(false);
    assert!(r2.errors.is_empty(), "sync 2 errors: {:?}", r2.errors);
    assert_eq!(r2.written_caldav, 1, "TW wins → CalDAV updated");
    assert_eq!(r2.written_tw, 0, "TW wins → no TW write");
    assert_eq!(h.count_caldav_vtodos(), 1);
}

/// CalDAV VTODO modified externally after a TW-wins sync → CalDAV wins LWW →
/// TW updated with the CalDAV content.
///
/// Setup requires a TW-wins sync first so that X-CALDAWARRIOR-LAST-SYNC is
/// updated to the current TW.modified value, allowing the subsequent CalDAV
/// modification to satisfy the LWW condition (CalDAV.LAST-MODIFIED > TW.modified
/// while TW.modified == last_sync).
#[test]
fn caldav_wins_lww() {
    if should_skip() {
        return;
    }
    let h = TestHarness::new();

    // Sync 1: initial push.
    let uuid = h.add_tw_task("Task A");
    let r1 = h.run_sync(false);
    assert!(r1.errors.is_empty(), "sync 1 errors: {:?}", r1.errors);
    assert_eq!(r1.written_caldav, 1);

    // Trigger a TW-wins sync so that X-CALDAWARRIOR-LAST-SYNC in the VTODO
    // is brought up to date with TW.modified.
    std::thread::sleep(Duration::from_secs(1));
    h.modify_tw_task_description(&uuid, "Task A v2");
    let r2 = h.run_sync(false);
    assert!(r2.errors.is_empty(), "sync 2 errors: {:?}", r2.errors);
    assert_eq!(r2.written_caldav, 1, "TW-wins sync 2 must write CalDAV");

    // Directly modify the CalDAV VTODO (new SUMMARY/DESCRIPTION + fresh
    // LAST-MODIFIED, X-CALDAWARRIOR-LAST-SYNC preserved from sync 2).
    std::thread::sleep(Duration::from_secs(1));
    h.modify_first_vtodo_summary("CalDAV updated");

    // Sync 3: CalDAV wins.
    // Condition: TW.modified == last_sync (no TW change since sync 2),
    //            CalDAV.LAST-MODIFIED > TW.modified (just modified externally).
    let r3 = h.run_sync(false);
    assert!(r3.errors.is_empty(), "sync 3 errors: {:?}", r3.errors);
    assert_eq!(r3.written_caldav, 0, "CalDAV wins → no CalDAV write");
    assert_eq!(r3.written_tw, 1, "CalDAV wins → TW updated");

    let task = h.get_tw_task(&uuid);
    assert_eq!(
        task["description"].as_str().unwrap_or(""),
        "CalDAV updated",
        "TW description should reflect CalDAV content after CalDAV-wins"
    );
}

/// After CalDAV wins, an immediate second sync produces zero writes —
/// the stable-point (loop prevention) assertion from the spec.
///
/// This verifies Layer 2 of the LWW algorithm: when both sides have
/// identical content after a CalDAV-wins write-back, `content_identical`
/// returns `Skip(Identical)` and no further writes occur.
#[test]
fn loop_prevention_stable_point() {
    if should_skip() {
        return;
    }
    let h = TestHarness::new();

    // Sync 1: initial push.
    let uuid = h.add_tw_task("Loop task");
    let r1 = h.run_sync(false);
    assert!(r1.errors.is_empty(), "sync 1 errors: {:?}", r1.errors);

    // Sync 2: TW-wins to bring X-CALDAWARRIOR-LAST-SYNC up to date.
    std::thread::sleep(Duration::from_secs(1));
    h.modify_tw_task_description(&uuid, "Loop task v2");
    let r2 = h.run_sync(false);
    assert!(r2.errors.is_empty(), "sync 2 errors: {:?}", r2.errors);
    assert_eq!(r2.written_caldav, 1);

    // Modify CalDAV externally → triggers CalDAV-wins on next sync.
    std::thread::sleep(Duration::from_secs(1));
    h.modify_first_vtodo_summary("CalDAV wins this round");

    // Sync 3: CalDAV wins → TW updated.
    let r3 = h.run_sync(false);
    assert!(r3.errors.is_empty(), "sync 3 errors: {:?}", r3.errors);
    assert_eq!(r3.written_tw, 1, "CalDAV wins on sync 3");
    assert_eq!(r3.written_caldav, 0);

    // Sync 4 immediately: content is now identical on both sides →
    // stable point, no writes in either direction.
    let r4 = h.run_sync(false);
    assert!(r4.errors.is_empty(), "sync 4 (stable-point) errors: {:?}", r4.errors);
    assert_eq!(
        r4.written_caldav, 0,
        "loop prevention: no CalDAV writes on stable-point sync"
    );
    assert_eq!(
        r4.written_tw, 0,
        "loop prevention: no TW writes on stable-point sync"
    );
}

/// Both TW and CalDAV modified concurrently (genuine conflict):
/// LWW resolves to TW wins, and the sync correctly uses the current
/// CalDAV ETag in the PUT — no ETag 412 conflict errors.
///
/// This test verifies that external CalDAV modifications (which change
/// the server-side ETag) are handled gracefully by the sync engine,
/// which always fetches the current ETag at the start of each run.
#[test]
fn etag_conflict_scenario() {
    if should_skip() {
        return;
    }
    let h = TestHarness::new();

    // Sync 1: push TW task to CalDAV (etag E1 established).
    let uuid = h.add_tw_task("ETag test task");
    let r1 = h.run_sync(false);
    assert!(r1.errors.is_empty(), "sync 1 errors: {:?}", r1.errors);
    assert_eq!(r1.written_caldav, 1);

    // External CalDAV modification → server now has etag E2.
    // A sync using the stale etag E1 would get HTTP 412; the engine must
    // fetch the current state (E2) at the start of each run.
    std::thread::sleep(Duration::from_secs(1));
    h.modify_first_vtodo_summary("CalDAV external edit");

    // Also modify TW → genuine bidirectional conflict.
    // TW.modified > X-CALDAWARRIOR-LAST-SYNC (T_add) → TW wins on LWW.
    h.modify_tw_task_description(&uuid, "TW concurrent edit");

    // Sync 2: LWW resolves the conflict. The sync fetches the current CalDAV
    // etag (E2) and uses it in the PUT, so no ETag 412 conflict occurs.
    let r2 = h.run_sync(false);
    assert!(
        r2.errors.is_empty(),
        "ETag conflict must be resolved without errors: {:?}",
        r2.errors
    );
    assert_eq!(r2.written_caldav, 1, "TW wins → CalDAV updated");
    assert_eq!(r2.written_tw, 0, "TW wins → no TW write");
}
