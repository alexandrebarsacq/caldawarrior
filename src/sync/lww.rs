use chrono::{DateTime, Utc};

use crate::types::{IREntry, PlannedOp, RelType, Side, SkipReason, UpdateReason};

const LAST_SYNC_PROP: &str = "X-CALDAWARRIOR-LAST-SYNC";
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

/// Read the `X-CALDAWARRIOR-LAST-SYNC` timestamp from the VTODO's extra props.
/// Returns Unix epoch (`1970-01-01T00:00:00Z`) if the property is absent or
/// cannot be parsed — making any TW modification appear "newer" and ensuring
/// TW wins on the very first sync.
fn get_last_sync(entry: &IREntry) -> DateTime<Utc> {
    entry
        .fetched_vtodo
        .as_ref()
        .and_then(|fv| {
            fv.vtodo
                .extra_props
                .iter()
                .find(|p| p.name.eq_ignore_ascii_case(LAST_SYNC_PROP))
                .and_then(|p| parse_ical_dt(&p.value))
        })
        .unwrap_or(DateTime::<Utc>::UNIX_EPOCH)
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

    // 2. DESCRIPTION — field mapper maps TW description → VTODO DESCRIPTION.
    if vtodo.description.as_deref().unwrap_or("") != tw.description.as_str() {
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
/// - `TW.modified > X-CALDAWARRIOR-LAST-SYNC` → TW wins.
/// - `CalDAV.LAST-MODIFIED > TW.modified` → CalDAV wins.
/// - Otherwise → TW wins (authoritative tiebreaker).
///
/// The `X-CALDAWARRIOR-LAST-SYNC` property must be written to the VTODO
/// (as `TW.modified`) on **every CalDAV write** by the write-back layer so
/// that the LAST-SYNC guard remains accurate across syncs.
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

    // LAST-SYNC stored in CalDAV VTODO (defaults to epoch when absent = first sync).
    let last_sync = get_last_sync(&entry);

    // TW wins when modified after the last sync point.
    if tw_modified > last_sync {
        return PlannedOp::ResolveConflict {
            entry,
            winner: Side::Tw,
            reason: UpdateReason::LwwTwWins,
        };
    }

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
    use crate::types::{FetchedVTODO, IcalProp, IREntry, TWTask, VTODO};
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
        }
    }

    fn make_vtodo(
        uid: &str,
        summary: &str,
        status: &str,
        last_modified: Option<DateTime<Utc>>,
        last_sync: Option<DateTime<Utc>>,
    ) -> VTODO {
        let mut extra_props = vec![];
        if let Some(ls) = last_sync {
            extra_props.push(IcalProp {
                name: LAST_SYNC_PROP.to_string(),
                params: vec![],
                value: ls.format("%Y%m%dT%H%M%SZ").to_string(),
            });
        }
        VTODO {
            uid: uid.to_string(),
            summary: Some(summary.to_string()),
            description: Some(summary.to_string()),
            status: Some(status.to_string()),
            last_modified,
            dtstamp: None,
            dtstart: None,
            due: None,
            completed: None,
            categories: vec![],
            rrule: None,
            depends: vec![],
            extra_props,
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
        }
    }

    #[test]
    fn tw_wins_when_modified_after_last_sync() {
        let uuid = Uuid::new_v4();
        let last_sync_ts = t(2026, 2, 1, 10, 0, 0);
        let tw_modified = t(2026, 2, 1, 11, 0, 0); // after last sync
        let caldav_lm = t(2026, 2, 1, 9, 0, 0); // older than TW

        let task = make_tw_task(uuid, "Updated task", tw_modified);
        let vtodo = make_vtodo(
            &uuid.to_string(),
            "Old task", // different content
            "NEEDS-ACTION",
            Some(caldav_lm),
            Some(last_sync_ts),
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
        let last_sync_ts = t(2026, 2, 1, 10, 0, 0);
        let tw_modified = t(2026, 2, 1, 9, 0, 0); // before last sync
        let caldav_lm = t(2026, 2, 1, 12, 0, 0); // newer than TW

        let task = make_tw_task(uuid, "Old TW task", tw_modified);
        let vtodo = make_vtodo(
            &uuid.to_string(),
            "Updated CalDAV task", // different content
            "NEEDS-ACTION",
            Some(caldav_lm),
            Some(last_sync_ts),
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
            Some(t(2026, 2, 1, 9, 0, 0)),
        );
        let entry = make_entry(task, vtodo);
        let now = t(2026, 2, 2, 0, 0, 0);

        match resolve_lww(entry, now) {
            PlannedOp::Skip { reason: SkipReason::Identical, .. } => {}
            other => panic!("expected Skip(Identical), got {:?}", other),
        }
    }

    #[test]
    fn no_last_sync_tw_wins() {
        // First sync: LAST-SYNC absent → defaults to epoch → TW.modified > epoch → TW wins.
        let uuid = Uuid::new_v4();
        let tw_modified = t(2026, 2, 1, 10, 0, 0);

        let task = make_tw_task(uuid, "New task", tw_modified);
        let vtodo = make_vtodo(
            &uuid.to_string(),
            "Different CalDAV content",
            "NEEDS-ACTION",
            Some(t(2026, 2, 1, 9, 0, 0)),
            None, // no LAST-SYNC prop
        );
        let entry = make_entry(task, vtodo);
        let now = t(2026, 2, 2, 0, 0, 0);

        match resolve_lww(entry, now) {
            PlannedOp::ResolveConflict { winner, reason, .. } => {
                assert!(matches!(winner, Side::Tw));
                assert!(matches!(reason, UpdateReason::LwwTwWins));
            }
            other => panic!("expected TW wins on first sync, got {:?}", other),
        }
    }

    #[test]
    fn regression_caldav_wins_then_identical_on_resync() {
        // Sync 1: CalDAV wins.
        // TW was last modified at 8am; last sync was at 9am (TW hasn't changed since);
        // CalDAV was updated at 10am (newer than TW) → CalDAV wins.
        let uuid = Uuid::new_v4();
        let tw_modified = t(2026, 2, 1, 8, 0, 0);
        let caldav_lm = t(2026, 2, 1, 10, 0, 0);
        let last_sync_ts = t(2026, 2, 1, 9, 0, 0); // after TW.modified

        let task_before = make_tw_task(uuid, "Old TW content", tw_modified);
        let vtodo_before = make_vtodo(
            &uuid.to_string(),
            "Updated CalDAV content",
            "NEEDS-ACTION",
            Some(caldav_lm),
            Some(last_sync_ts),
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
            Some(last_sync_ts),  // LAST-SYNC still at old value
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
            Some(t(2026, 2, 1, 9, 0, 0)),
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
        //
        // Scenario: TW modified BEFORE last_sync, CalDAV has neither LAST-MODIFIED nor DTSTAMP.
        let uuid = Uuid::new_v4();
        let last_sync_ts = t(2026, 2, 1, 10, 0, 0);
        let tw_modified = t(2026, 2, 1, 8, 0, 0); // before last_sync

        let task = make_tw_task(uuid, "Task content", tw_modified);
        let vtodo = make_vtodo(
            &uuid.to_string(),
            "Different CalDAV content", // different content so Layer 2 doesn't fire
            "NEEDS-ACTION",
            None, // LAST-MODIFIED absent
            Some(last_sync_ts),
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
        // When LAST-MODIFIED is absent but DTSTAMP is present and newer than TW, CalDAV wins.
        let uuid = Uuid::new_v4();
        let last_sync_ts = t(2026, 2, 1, 9, 0, 0);
        let tw_modified = t(2026, 2, 1, 8, 0, 0); // before last_sync
        let dtstamp_ts = t(2026, 2, 1, 11, 0, 0); // newer than TW

        let task = make_tw_task(uuid, "Task content", tw_modified);
        let mut vtodo = make_vtodo(
            &uuid.to_string(),
            "Updated CalDAV content", // different content so Layer 2 doesn't fire
            "NEEDS-ACTION",
            None, // LAST-MODIFIED absent
            Some(last_sync_ts),
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
}
