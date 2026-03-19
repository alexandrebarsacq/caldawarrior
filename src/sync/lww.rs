use chrono::{DateTime, Utc};

use crate::types::{IREntry, PlannedOp, RelType, Side, SkipReason, UpdateReason};

const WAIT_PROP: &str = "X-TASKWARRIOR-WAIT";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert an optional `DateTime<Utc>` to its Unix timestamp in whole seconds,
/// discarding sub-second precision for comparison purposes.
fn to_secs_opt(dt: Option<DateTime<Utc>>) -> Option<i64> {
    dt.map(|d| d.timestamp())
}

/// Normalize a status string (CalDAV or TW) to a canonical lowercase value
/// for cross-system comparison.
///
/// | Input               | Output      |
/// |---------------------|-------------|
/// | NEEDS-ACTION        | pending     |
/// | IN-PROCESS          | pending     |
/// | pending / waiting   | pending     |
/// | COMPLETED           | completed   |
/// | CANCELLED / CANCELED| deleted     |
/// | deleted             | deleted     |
/// | anything else       | lowercase   |
fn normalize_status(s: &str) -> String {
    let upper = s.to_ascii_uppercase();
    match upper.as_str() {
        "NEEDS-ACTION" | "IN-PROCESS" | "PENDING" | "WAITING" => "pending".to_string(),
        "COMPLETED" => "completed".to_string(),
        "CANCELLED" | "CANCELED" | "DELETED" => "deleted".to_string(),
        _ => upper.to_ascii_lowercase(),
    }
}

/// Parse an iCalendar UTC datetime string (`YYYYMMDDTHHMMSSZ`) into `DateTime<Utc>`.
fn parse_ical_dt(s: &str) -> Option<DateTime<Utc>> {
    use chrono::NaiveDateTime;
    let s = s.trim().trim_end_matches('Z');
    NaiveDateTime::parse_from_str(s, "%Y%m%dT%H%M%S")
        .ok()
        .map(|ndt| ndt.and_utc())
}

// ---------------------------------------------------------------------------
// Layer 2: content-identical check (loop prevention)
// ---------------------------------------------------------------------------

/// Check whether the 8 tracked fields are identical between TW and CalDAV,
/// making a sync write in either direction a no-op.
///
/// Normalization contract (per spec):
/// - Timestamps at second precision.
/// - TEXT fields (`SUMMARY`, `DESCRIPTION`) compared as stored (already unescaped by parser).
/// - `STATUS` normalized via [`normalize_status`].
/// - `RELATED-TO[DEPENDS-ON]` sorted by UID string before comparison.
/// - `X-TASKWARRIOR-WAIT` compared at second precision; expired TW wait → None.
///
/// **Note:** `now` is injected for deterministic testing of the expired-wait
/// collapse logic (avoids wall-clock dependency).
///
/// # Panics
/// Panics if `entry.tw_task` or `entry.fetched_vtodo` is `None` (caller ensures
/// this is called only for paired entries).
fn content_identical(entry: &IREntry, now: DateTime<Utc>) -> bool {
    let tw = entry.tw_task.as_ref().expect("content_identical: tw_task is None");
    let vtodo = &entry
        .fetched_vtodo
        .as_ref()
        .expect("content_identical: fetched_vtodo is None")
        .vtodo;

    // 1. SUMMARY — Phase 3 write-back sets VTODO SUMMARY = TW description.
    if vtodo.summary.as_deref() != Some(tw.description.as_str()) {
        return false;
    }

    // 2. DESCRIPTION — CalDAV DESCRIPTION holds TW first annotation text (or None when absent).
    let tw_first_annotation = tw.annotations.first().map(|a| a.description.as_str());
    if vtodo.description.as_deref() != tw_first_annotation {
        return false;
    }

    // 3. STATUS (normalized)
    let vtodo_status = vtodo.status.as_deref().unwrap_or("NEEDS-ACTION");
    if normalize_status(vtodo_status) != normalize_status(&tw.status) {
        return false;
    }

    // 4. DUE (second precision)
    if to_secs_opt(vtodo.due) != to_secs_opt(tw.due) {
        return false;
    }

    // 5. DTSTART vs TW scheduled (second precision)
    if to_secs_opt(vtodo.dtstart) != to_secs_opt(tw.scheduled) {
        return false;
    }

    // 6. COMPLETED vs TW end (second precision)
    if to_secs_opt(vtodo.completed) != to_secs_opt(tw.end) {
        return false;
    }

    // 7. RELATED-TO[DEPENDS-ON] vs resolved_depends (sorted)
    let mut caldav_deps: Vec<&str> = vtodo
        .depends
        .iter()
        .filter(|(rel, _)| matches!(rel, RelType::DependsOn))
        .map(|(_, uid)| uid.as_str())
        .collect();
    caldav_deps.sort_unstable();

    let mut tw_deps: Vec<&str> = entry.resolved_depends.iter().map(String::as_str).collect();
    tw_deps.sort_unstable();

    if caldav_deps != tw_deps {
        return false;
    }

    // 8. X-TASKWARRIOR-WAIT (second precision; expired TW wait collapsed to None)
    let caldav_wait_secs: Option<i64> = vtodo
        .extra_props
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case(WAIT_PROP))
        .and_then(|p| parse_ical_dt(&p.value))
        .map(|dt| dt.timestamp());

    let tw_wait_secs: Option<i64> = tw.wait.filter(|&w| w > now).map(|w| w.timestamp());

    if caldav_wait_secs != tw_wait_secs {
        return false;
    }

    // 9. PRIORITY — TW priority letter H/M/L ↔ iCal integer 1/5/9.
    let tw_priority_ical: Option<u8> = tw.priority.as_deref().and_then(|p| match p {
        "H" => Some(1),
        "M" => Some(5),
        "L" => Some(9),
        _ => None,
    });
    if vtodo.priority != tw_priority_ical {
        return false;
    }

    // 10. CATEGORIES — VTODO categories ↔ TW tags (sorted for order-independent comparison).
    let mut caldav_cats: Vec<&str> = vtodo.categories.iter().map(String::as_str).collect();
    caldav_cats.sort_unstable();
    let mut tw_tags: Vec<&str> = tw
        .tags
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(String::as_str)
        .collect();
    tw_tags.sort_unstable();
    if caldav_cats != tw_tags {
        return false;
    }

    true
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Resolve a sync conflict for a **paired** `IREntry` (both `tw_task` and
/// `fetched_vtodo` are `Some`) using Last-Write-Wins.
///
/// Two-layer conflict resolution:
///
/// **Layer 2 (content-identical):** If all 8 tracked fields match after
/// normalization, return `Skip(Identical)` — no write needed. This is the
/// primary loop-prevention mechanism.
///
/// **Layer 1 (LWW timestamp):**
/// - `TW.modified` (fallback `TW.entry`) vs `VTODO.LAST-MODIFIED` (fallback `VTODO.DTSTAMP`).
/// - `CalDAV timestamp > TW timestamp` → CalDAV wins.
/// - `CalDAV timestamp absent` or `TW timestamp >= CalDAV timestamp` → TW wins (authoritative tiebreaker).
///
/// **DTSTAMP:** Used as a fallback when `LAST-MODIFIED` is absent — the iCalendar
/// parser now captures DTSTAMP into `VTODO.dtstamp`. If both are absent, CalDAV
/// timestamp comparison is skipped and TW wins via tiebreaker.
///
/// # Panics
/// Panics if `entry.tw_task` or `entry.fetched_vtodo` is `None`.
pub fn resolve_lww(entry: IREntry, now: DateTime<Utc>) -> PlannedOp {
    // Layer 2: loop prevention via content-identical check.
    if content_identical(&entry, now) {
        let tw_uuid = entry
            .tw_uuid
            .expect("resolve_lww: paired entry must have tw_uuid");
        return PlannedOp::Skip {
            tw_uuid: Some(tw_uuid),
            reason: SkipReason::Identical,
        };
    }

    // Layer 1: LWW timestamp comparison.
    let tw = entry.tw_task.as_ref().expect("resolve_lww: tw_task is None");
    let vtodo = &entry
        .fetched_vtodo
        .as_ref()
        .expect("resolve_lww: fetched_vtodo is None")
        .vtodo;

    // TW modification timestamp (fall back to entry timestamp if modified is None).
    let tw_modified = tw.modified.unwrap_or(tw.entry);

    // CalDAV wins when its LAST-MODIFIED (or DTSTAMP fallback) is more recent than TW.
    let caldav_ts = vtodo.last_modified.or(vtodo.dtstamp);
    if let Some(caldav_ts) = caldav_ts {
        if caldav_ts > tw_modified {
            return PlannedOp::ResolveConflict {
                entry,
                winner: Side::CalDav,
                reason: UpdateReason::LwwCalDavWins,
            };
        }
    }

    // Tiebreaker: TW is authoritative.
    PlannedOp::ResolveConflict {
        entry,
        winner: Side::Tw,
        reason: UpdateReason::LwwTwWins,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FetchedVTODO, IREntry, TWTask, VTODO};
    use chrono::TimeZone;
    use uuid::Uuid;

    fn t(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, s).unwrap()
    }

    fn make_tw_task(uuid: Uuid, description: &str, modified: DateTime<Utc>) -> TWTask {
        TWTask {
            uuid,
            status: "pending".to_string(),
            description: description.to_string(),
            entry: t(2026, 1, 1, 0, 0, 0),
            modified: Some(modified),
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

    fn make_vtodo(
        uid: &str,
        summary: &str,
        status: &str,
        last_modified: Option<DateTime<Utc>>,
    ) -> VTODO {
        VTODO {
            uid: uid.to_string(),
            summary: Some(summary.to_string()),
            description: None, // no annotations on test tasks
            status: Some(status.to_string()),
            last_modified,
            ..Default::default()
        }
    }

    fn make_entry(
        tw_task: TWTask,
        vtodo: VTODO,
    ) -> IREntry {
        IREntry {
            tw_uuid: Some(tw_task.uuid),
            caldav_uid: Some(vtodo.uid.clone()),
            tw_task: Some(tw_task),
            fetched_vtodo: Some(FetchedVTODO {
                href: format!("/{}.ics", vtodo.uid),
                etag: None,
                vtodo,
            }),
            resolved_depends: vec![],
            cyclic: false,
            calendar_url: None,
            dirty_tw: false,
            dirty_caldav: false,
            project: None,
        }
    }

    #[test]
    fn tw_wins_when_modified_is_newer() {
        let uuid = Uuid::new_v4();
        let tw_modified = t(2026, 2, 1, 11, 0, 0);
        let caldav_lm = t(2026, 2, 1, 9, 0, 0); // older than TW

        let task = make_tw_task(uuid, "Updated task", tw_modified);
        let vtodo = make_vtodo(
            &uuid.to_string(),
            "Old task", // different content
            "NEEDS-ACTION",
            Some(caldav_lm),
        );
        let entry = make_entry(task, vtodo);
        let now = t(2026, 2, 2, 0, 0, 0);

        match resolve_lww(entry, now) {
            PlannedOp::ResolveConflict { winner, reason, .. } => {
                assert!(matches!(winner, Side::Tw));
                assert!(matches!(reason, UpdateReason::LwwTwWins));
            }
            other => panic!("expected TW wins, got {:?}", other),
        }
    }

    #[test]
    fn caldav_wins_when_newer_than_tw() {
        let uuid = Uuid::new_v4();
        let tw_modified = t(2026, 2, 1, 9, 0, 0);
        let caldav_lm = t(2026, 2, 1, 12, 0, 0); // newer than TW

        let task = make_tw_task(uuid, "Old TW task", tw_modified);
        let vtodo = make_vtodo(
            &uuid.to_string(),
            "Updated CalDAV task", // different content
            "NEEDS-ACTION",
            Some(caldav_lm),
        );
        let entry = make_entry(task, vtodo);
        let now = t(2026, 2, 2, 0, 0, 0);

        match resolve_lww(entry, now) {
            PlannedOp::ResolveConflict { winner, reason, .. } => {
                assert!(matches!(winner, Side::CalDav));
                assert!(matches!(reason, UpdateReason::LwwCalDavWins));
            }
            other => panic!("expected CalDAV wins, got {:?}", other),
        }
    }

    #[test]
    fn identical_content_skips() {
        let uuid = Uuid::new_v4();
        let ts = t(2026, 2, 1, 10, 0, 0);

        let task = make_tw_task(uuid, "Buy milk", ts);
        let vtodo = make_vtodo(
            &uuid.to_string(),
            "Buy milk", // identical
            "NEEDS-ACTION",
            Some(ts),
        );
        let entry = make_entry(task, vtodo);
        let now = t(2026, 2, 2, 0, 0, 0);

        match resolve_lww(entry, now) {
            PlannedOp::Skip { reason: SkipReason::Identical, .. } => {}
            other => panic!("expected Skip(Identical), got {:?}", other),
        }
    }

    #[test]
    fn tw_wins_on_equal_timestamps() {
        // Scenario (b): TW.modified == LAST-MODIFIED → TW wins (equality prefers local edit).
        let uuid = Uuid::new_v4();
        let ts = t(2026, 2, 1, 10, 0, 0);

        let task = make_tw_task(uuid, "TW version", ts);
        let vtodo = make_vtodo(
            &uuid.to_string(),
            "CalDAV version", // different content so Layer 2 doesn't fire
            "NEEDS-ACTION",
            Some(ts), // equal timestamp
        );
        let entry = make_entry(task, vtodo);
        let now = t(2026, 2, 2, 0, 0, 0);

        match resolve_lww(entry, now) {
            PlannedOp::ResolveConflict { winner, reason, .. } => {
                assert!(matches!(winner, Side::Tw), "TW should win on equal timestamps");
                assert!(matches!(reason, UpdateReason::LwwTwWins));
            }
            other => panic!("expected TW wins on equal timestamps, got {:?}", other),
        }
    }

    #[test]
    fn regression_caldav_wins_then_identical_on_resync() {
        // Sync 1: CalDAV wins.
        // TW was last modified at 8am; CalDAV was updated at 10am (newer than TW) → CalDAV wins.
        let uuid = Uuid::new_v4();
        let tw_modified = t(2026, 2, 1, 8, 0, 0);
        let caldav_lm = t(2026, 2, 1, 10, 0, 0);

        let task_before = make_tw_task(uuid, "Old TW content", tw_modified);
        let vtodo_before = make_vtodo(
            &uuid.to_string(),
            "Updated CalDAV content",
            "NEEDS-ACTION",
            Some(caldav_lm),
        );
        let entry_before = make_entry(task_before, vtodo_before);
        let now = t(2026, 2, 2, 0, 0, 0);

        // Verify CalDAV wins on sync 1.
        match resolve_lww(entry_before, now) {
            PlannedOp::ResolveConflict { winner, .. } => {
                assert!(matches!(winner, Side::CalDav), "sync 1: CalDAV should win");
            }
            other => panic!("sync 1: expected CalDAV wins, got {:?}", other),
        }

        // Sync 2: TW has been updated from CalDAV, content now matches.
        // Simulate: TW description updated, TW modified = now.
        let tw_after_update = t(2026, 2, 2, 0, 0, 1);
        let mut task_after = make_tw_task(uuid, "Updated CalDAV content", tw_after_update);
        task_after.status = "pending".to_string();

        let vtodo_after = make_vtodo(
            &uuid.to_string(),
            "Updated CalDAV content", // same as TW now
            "NEEDS-ACTION",
            Some(caldav_lm),     // CalDAV unchanged
        );
        let entry_after = make_entry(task_after, vtodo_after);

        // Layer 2 (content-identical) should fire and produce Skip(Identical).
        match resolve_lww(entry_after, now) {
            PlannedOp::Skip { reason: SkipReason::Identical, .. } => {}
            other => panic!(
                "sync 2 (regression): expected Skip(Identical) after CalDAV-wins resync, got {:?}",
                other
            ),
        }
    }

    #[test]
    fn status_normalization_identical() {
        // TW "pending" == CalDAV "NEEDS-ACTION" after normalization.
        let uuid = Uuid::new_v4();
        let ts = t(2026, 2, 1, 10, 0, 0);

        let task = make_tw_task(uuid, "My task", ts);
        // vtodo has NEEDS-ACTION status; TW has pending — should be identical.
        let vtodo = make_vtodo(
            &uuid.to_string(),
            "My task",
            "NEEDS-ACTION", // normalizes to "pending"
            Some(ts),
        );
        let entry = make_entry(task, vtodo);
        let now = t(2026, 2, 2, 0, 0, 0);

        match resolve_lww(entry, now) {
            PlannedOp::Skip { reason: SkipReason::Identical, .. } => {}
            other => panic!("expected Skip(Identical) for status normalization, got {:?}", other),
        }
    }

    #[test]
    fn no_last_modified_no_dtstamp_tw_wins_tiebreaker() {
        // When both LAST-MODIFIED and DTSTAMP are absent, CalDAV timestamp comparison
        // is skipped entirely and TW wins via tiebreaker.
        let uuid = Uuid::new_v4();
        let tw_modified = t(2026, 2, 1, 8, 0, 0);

        let task = make_tw_task(uuid, "Task content", tw_modified);
        let vtodo = make_vtodo(
            &uuid.to_string(),
            "Different CalDAV content", // different content so Layer 2 doesn't fire
            "NEEDS-ACTION",
            None, // LAST-MODIFIED absent
        );
        // vtodo.dtstamp is also None (struct literal via make_vtodo)
        let entry = make_entry(task, vtodo);
        let now = t(2026, 2, 2, 0, 0, 0);

        // Both timestamps absent → tiebreaker → TW wins.
        match resolve_lww(entry, now) {
            PlannedOp::ResolveConflict { winner, reason, .. } => {
                assert!(matches!(winner, Side::Tw), "TW should win via tiebreaker when no CalDAV timestamps");
                assert!(matches!(reason, UpdateReason::LwwTwWins));
            }
            other => panic!("expected TW wins (no CalDAV timestamps), got {:?}", other),
        }
    }

    #[test]
    fn no_last_modified_dtstamp_fallback_caldav_wins() {
        // Scenario (e): LAST-MODIFIED absent, DTSTAMP present and newer than TW → CalDAV wins.
        let uuid = Uuid::new_v4();
        let tw_modified = t(2026, 2, 1, 8, 0, 0);
        let dtstamp_ts = t(2026, 2, 1, 11, 0, 0); // newer than TW

        let task = make_tw_task(uuid, "Task content", tw_modified);
        let mut vtodo = make_vtodo(
            &uuid.to_string(),
            "Updated CalDAV content", // different content so Layer 2 doesn't fire
            "NEEDS-ACTION",
            None, // LAST-MODIFIED absent
        );
        vtodo.dtstamp = Some(dtstamp_ts);
        let entry = make_entry(task, vtodo);
        let now = t(2026, 2, 2, 0, 0, 0);

        match resolve_lww(entry, now) {
            PlannedOp::ResolveConflict { winner, reason, .. } => {
                assert!(matches!(winner, Side::CalDav), "CalDAV should win via DTSTAMP fallback");
                assert!(matches!(reason, UpdateReason::LwwCalDavWins));
            }
            other => panic!("expected CalDAV wins via DTSTAMP fallback, got {:?}", other),
        }
    }

    #[test]
    fn tw_modified_none_falls_back_to_entry() {
        // Scenario (f): TW.modified=None → falls back to TW.entry for the comparison.
        // TW.entry = 2026-01-01 09:00:00, CalDAV LAST-MODIFIED = 2026-01-01 08:00:00 (older).
        // Expected: TW wins because TW.entry > LAST-MODIFIED.
        let uuid = Uuid::new_v4();
        let entry_ts = t(2026, 1, 1, 9, 0, 0);
        let caldav_lm = t(2026, 1, 1, 8, 0, 0); // older than TW entry

        // Build TWTask manually so we can set modified=None.
        let task = TWTask {
            uuid,
            status: "pending".to_string(),
            description: "TW entry fallback task".to_string(),
            entry: entry_ts,
            modified: None, // explicitly None
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
        };
        let vtodo = make_vtodo(
            &uuid.to_string(),
            "CalDAV content", // different from TW so Layer 2 doesn't fire
            "NEEDS-ACTION",
            Some(caldav_lm),
        );
        let entry = make_entry(task, vtodo);
        let now = t(2026, 2, 2, 0, 0, 0);

        match resolve_lww(entry, now) {
            PlannedOp::ResolveConflict { winner, reason, .. } => {
                assert!(
                    matches!(winner, Side::Tw),
                    "TW should win when modified=None and entry > LAST-MODIFIED"
                );
                assert!(matches!(reason, UpdateReason::LwwTwWins));
            }
            other => panic!(
                "expected TW wins via entry fallback, got {:?}",
                other
            ),
        }
    }

    #[test]
    fn identical_content_skips_even_when_tw_newer() {
        // Scenario (g): Layer 2 (content-identical) fires before Layer 1 (LWW timestamp).
        // TW.modified > LAST-MODIFIED, but all 8 tracked fields are identical →
        // must return Skip(Identical), NOT ResolveConflict.
        let uuid = Uuid::new_v4();
        let tw_modified = t(2026, 2, 1, 12, 0, 0);
        let caldav_lm = t(2026, 2, 1, 9, 0, 0); // older — TW would win LWW

        let task = make_tw_task(uuid, "Identical content", tw_modified);
        let vtodo = make_vtodo(
            &uuid.to_string(),
            "Identical content", // same as TW — Layer 2 should fire
            "NEEDS-ACTION",
            Some(caldav_lm),
        );
        let entry = make_entry(task, vtodo);
        let now = t(2026, 2, 2, 0, 0, 0);

        match resolve_lww(entry, now) {
            PlannedOp::Skip { reason: SkipReason::Identical, .. } => {}
            other => panic!(
                "expected Skip(Identical) — Layer 2 must fire before Layer 1, got {:?}",
                other
            ),
        }
    }
}
