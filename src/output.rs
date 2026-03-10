use crate::types::{IREntry, PlannedOp, Side, SkipReason, SyncResult};

/// Format and print sync results to stdout/stderr.
///
/// Errors and warnings are printed to stderr. In dry-run mode, each planned
/// operation is listed followed by a summary line. In live mode, a concise
/// "Synced: ..." summary is printed to stdout.
pub fn print_result(result: &SyncResult, dry_run: bool) {
    // Print errors to stderr first.
    for err in &result.errors {
        eprintln!("[ERROR] {}", err);
    }

    // Print warnings to stderr.
    for warn in &result.warnings {
        let prefix = match &warn.tw_uuid {
            Some(uuid) => format!("[WARN] [{}]", uuid),
            None => "[WARN]".to_string(),
        };
        eprintln!("{} {}", prefix, warn.message);
    }

    if dry_run {
        for op in &result.planned_ops {
            println!("{}", format_planned_op(op));
        }
        println!("{}", format_dry_run_summary(&result.planned_ops));
    } else {
        let (caldav_creates, caldav_updates, tw_creates, tw_updates, _deletes, _skips) =
            count_ops(&result.planned_ops);
        println!(
            "Synced: {} created, {} updated in CalDAV; {} created, {} updated in TW",
            caldav_creates, caldav_updates, tw_creates, tw_updates
        );
    }
}

/// Format the dry-run summary line.
pub(crate) fn format_dry_run_summary(ops: &[PlannedOp]) -> String {
    let (caldav_creates, caldav_updates, tw_creates, tw_updates, deletes, skips) = count_ops(ops);
    let total_creates = caldav_creates + tw_creates;
    let total_updates = caldav_updates + tw_updates;
    format!(
        "[DRY-RUN] Would: {} create(s), {} update(s), {} delete(s), {} skip(s)",
        total_creates, total_updates, deletes, skips
    )
}

/// Count operations by category from a slice of planned ops.
///
/// Returns `(caldav_creates, caldav_updates, tw_creates, tw_updates, deletes, skips)`.
fn count_ops(ops: &[PlannedOp]) -> (usize, usize, usize, usize, usize, usize) {
    let mut caldav_creates = 0usize;
    let mut caldav_updates = 0usize;
    let mut tw_creates = 0usize;
    let mut tw_updates = 0usize;
    let mut deletes = 0usize;
    let mut skips = 0usize;

    for op in ops {
        match op {
            // TW task pushed to CalDAV for the first time → new CalDAV entry.
            PlannedOp::PushToCalDav(_) => caldav_creates += 1,
            // CalDAV entry pulled into TW → new TW task.
            PlannedOp::PullFromCalDav(_) => tw_creates += 1,
            PlannedOp::DeleteFromCalDav(_) | PlannedOp::DeleteFromTw(_) => deletes += 1,
            // Conflict: the winning side's data is written to the OTHER side.
            PlannedOp::ResolveConflict { winner, .. } => match winner {
                // TW wins → CalDAV entry is updated with TW data.
                Side::Tw => caldav_updates += 1,
                // CalDAV wins → TW task is updated with CalDAV data.
                Side::CalDav => tw_updates += 1,
            },
            PlannedOp::Skip { .. } => skips += 1,
        }
    }

    (caldav_creates, caldav_updates, tw_creates, tw_updates, deletes, skips)
}

/// Format a single planned operation as a human-readable dry-run line.
pub(crate) fn format_planned_op(op: &PlannedOp) -> String {
    match op {
        PlannedOp::PushToCalDav(entry) => {
            format!("[DRY-RUN] [CREATE] CalDAV <- TW: {}", get_description(entry))
        }
        PlannedOp::PullFromCalDav(entry) => {
            format!("[DRY-RUN] [CREATE] TW <- CalDAV: {}", get_description(entry))
        }
        PlannedOp::DeleteFromCalDav(entry) => {
            format!("[DRY-RUN] [DELETE] CalDAV: {}", get_description(entry))
        }
        PlannedOp::DeleteFromTw(entry) => {
            format!("[DRY-RUN] [DELETE] TW: {}", get_description(entry))
        }
        PlannedOp::ResolveConflict {
            entry,
            winner,
            reason: _,
        } => {
            let winner_str = match winner {
                Side::Tw => "TW",
                Side::CalDav => "CalDAV",
            };
            format!(
                "[DRY-RUN] [UPDATE] Conflict resolved ({} wins): {}",
                winner_str,
                get_description(entry)
            )
        }
        PlannedOp::Skip { tw_uuid, reason } => {
            let id = tw_uuid
                .map(|u| u.to_string())
                .unwrap_or_else(|| "?".to_string());
            format!("[DRY-RUN] [SKIP] {} ({})", id, format_skip_reason(reason))
        }
    }
}

/// Extract a human-readable description from an IREntry.
/// Prefers the TW task description, then the CalDAV VTODO summary, then the
/// UUID, then "unknown" as a last resort.
fn get_description(entry: &IREntry) -> String {
    if let Some(task) = &entry.tw_task {
        return task.description.clone();
    }
    if let Some(fetched) = &entry.fetched_vtodo {
        return fetched
            .vtodo
            .summary
            .clone()
            .unwrap_or_else(|| fetched.vtodo.uid.clone());
    }
    entry
        .tw_uuid
        .map(|u| u.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Convert a SkipReason to a short, readable string.
fn format_skip_reason(reason: &SkipReason) -> &'static str {
    match reason {
        SkipReason::Cancelled => "cancelled",
        SkipReason::Completed => "completed",
        SkipReason::Recurring => "recurring",
        SkipReason::Cyclic => "cyclic dependency",
        SkipReason::Identical => "identical",
        SkipReason::DeletedBeforeSync => "deleted before sync",
        SkipReason::AlreadyDeleted => "already deleted",
        SkipReason::CalDavDeletedTwTerminal => "caldav deleted, tw terminal",
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        FetchedVTODO, IREntry, PlannedOp, Side, SkipReason, SyncResult, TWTask, UpdateReason,
        Warning, VTODO,
    };
    use chrono::Utc;
    use uuid::Uuid;

    fn make_tw_task(description: &str) -> TWTask {
        TWTask {
            uuid: Uuid::new_v4(),
            status: "pending".to_string(),
            description: description.to_string(),
            entry: Utc::now(),
            modified: None,
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

    fn make_entry_with_tw(description: &str) -> IREntry {
        let task = make_tw_task(description);
        IREntry {
            tw_uuid: Some(task.uuid),
            caldav_uid: None,
            tw_task: Some(task),
            fetched_vtodo: None,
            resolved_depends: vec![],
            cyclic: false,
            calendar_url: None,
            dirty_tw: false,
            dirty_caldav: false,
            project: None,
        }
    }

    fn make_entry_with_caldav(summary: &str) -> IREntry {
        let vtodo = VTODO {
            uid: "test-uid-1".to_string(),
            summary: Some(summary.to_string()),
            description: None,
            status: None,
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
        };
        let fetched = FetchedVTODO {
            href: "/cal/test.ics".to_string(),
            etag: None,
            vtodo,
        };
        IREntry {
            tw_uuid: None,
            caldav_uid: Some("test-uid-1".to_string()),
            tw_task: None,
            fetched_vtodo: Some(fetched),
            resolved_depends: vec![],
            cyclic: false,
            calendar_url: None,
            dirty_tw: false,
            dirty_caldav: false,
            project: None,
        }
    }

    fn empty_result() -> SyncResult {
        SyncResult {
            planned_ops: vec![],
            warnings: vec![],
            errors: vec![],
            written_tw: 0,
            written_caldav: 0,
            skipped: 0,
        }
    }

    // -----------------------------------------------------------------------
    // format_planned_op tests
    // -----------------------------------------------------------------------

    #[test]
    fn format_push_to_caldav() {
        let entry = make_entry_with_tw("Buy groceries");
        let op = PlannedOp::PushToCalDav(entry);
        let s = format_planned_op(&op);
        assert!(s.contains("[DRY-RUN]"), "missing [DRY-RUN]: {}", s);
        assert!(s.contains("[CREATE]"), "missing [CREATE]: {}", s);
        assert!(s.contains("CalDAV <- TW"), "missing direction: {}", s);
        assert!(s.contains("Buy groceries"), "missing description: {}", s);
    }

    #[test]
    fn format_pull_from_caldav() {
        let entry = make_entry_with_caldav("Meeting notes");
        let op = PlannedOp::PullFromCalDav(entry);
        let s = format_planned_op(&op);
        assert!(s.contains("[DRY-RUN]"), "missing [DRY-RUN]: {}", s);
        assert!(s.contains("[CREATE]"), "missing [CREATE]: {}", s);
        assert!(s.contains("TW <- CalDAV"), "missing direction: {}", s);
        assert!(s.contains("Meeting notes"), "missing description: {}", s);
    }

    #[test]
    fn format_delete_from_caldav() {
        let entry = make_entry_with_tw("Old task");
        let op = PlannedOp::DeleteFromCalDav(entry);
        let s = format_planned_op(&op);
        assert!(s.contains("[DELETE]"), "missing [DELETE]: {}", s);
        assert!(s.contains("CalDAV"), "missing target: {}", s);
        assert!(s.contains("Old task"), "missing description: {}", s);
    }

    #[test]
    fn format_delete_from_tw() {
        let entry = make_entry_with_tw("Stale task");
        let op = PlannedOp::DeleteFromTw(entry);
        let s = format_planned_op(&op);
        assert!(s.contains("[DELETE]"), "missing [DELETE]: {}", s);
        assert!(s.contains("TW"), "missing target: {}", s);
        assert!(s.contains("Stale task"), "missing description: {}", s);
    }

    #[test]
    fn format_resolve_conflict_tw_wins() {
        let entry = make_entry_with_tw("Conflicted task");
        let op = PlannedOp::ResolveConflict {
            entry,
            winner: Side::Tw,
            reason: UpdateReason::LwwTwWins,
        };
        let s = format_planned_op(&op);
        assert!(s.contains("[UPDATE]"), "missing [UPDATE]: {}", s);
        assert!(s.contains("TW wins"), "missing winner: {}", s);
        assert!(s.contains("Conflicted task"), "missing description: {}", s);
    }

    #[test]
    fn format_resolve_conflict_caldav_wins() {
        let entry = make_entry_with_tw("Conflicted task");
        let op = PlannedOp::ResolveConflict {
            entry,
            winner: Side::CalDav,
            reason: UpdateReason::LwwCalDavWins,
        };
        let s = format_planned_op(&op);
        assert!(s.contains("[UPDATE]"), "missing [UPDATE]: {}", s);
        assert!(s.contains("CalDAV wins"), "missing winner: {}", s);
    }

    #[test]
    fn format_skip_with_uuid() {
        let uuid = Uuid::new_v4();
        let op = PlannedOp::Skip {
            tw_uuid: Some(uuid),
            reason: SkipReason::Identical,
        };
        let s = format_planned_op(&op);
        assert!(s.contains("[SKIP]"), "missing [SKIP]: {}", s);
        assert!(s.contains(&uuid.to_string()), "missing uuid: {}", s);
        assert!(s.contains("identical"), "missing reason: {}", s);
    }

    #[test]
    fn format_skip_without_uuid() {
        let op = PlannedOp::Skip {
            tw_uuid: None,
            reason: SkipReason::Cancelled,
        };
        let s = format_planned_op(&op);
        assert!(s.contains("[SKIP]"), "missing [SKIP]: {}", s);
        assert!(s.contains('?'), "missing placeholder for missing uuid: {}", s);
        assert!(s.contains("cancelled"), "missing reason: {}", s);
    }

    // -----------------------------------------------------------------------
    // count_ops tests
    // -----------------------------------------------------------------------

    #[test]
    fn count_ops_empty() {
        let (cc, cu, tc, tu, d, s) = count_ops(&[]);
        assert_eq!((cc, cu, tc, tu, d, s), (0, 0, 0, 0, 0, 0));
    }

    #[test]
    fn count_ops_mixed() {
        let ops = vec![
            PlannedOp::PushToCalDav(make_entry_with_tw("a")),
            PlannedOp::PushToCalDav(make_entry_with_tw("b")),
            PlannedOp::PullFromCalDav(make_entry_with_caldav("c")),
            PlannedOp::DeleteFromCalDav(make_entry_with_tw("d")),
            PlannedOp::DeleteFromTw(make_entry_with_tw("e")),
            PlannedOp::ResolveConflict {
                entry: make_entry_with_tw("f"),
                winner: Side::Tw,
                reason: UpdateReason::LwwTwWins,
            },
            PlannedOp::ResolveConflict {
                entry: make_entry_with_tw("g"),
                winner: Side::CalDav,
                reason: UpdateReason::LwwCalDavWins,
            },
            PlannedOp::Skip {
                tw_uuid: Some(Uuid::new_v4()),
                reason: SkipReason::Identical,
            },
        ];
        let (cc, cu, tc, tu, d, s) = count_ops(&ops);
        assert_eq!(cc, 2, "caldav creates");
        assert_eq!(cu, 1, "caldav updates (TW wins)");
        assert_eq!(tc, 1, "tw creates");
        assert_eq!(tu, 1, "tw updates (CalDAV wins)");
        assert_eq!(d, 2, "deletes");
        assert_eq!(s, 1, "skips");
    }

    // -----------------------------------------------------------------------
    // get_description tests
    // -----------------------------------------------------------------------

    #[test]
    fn get_description_prefers_tw_task() {
        let entry = make_entry_with_tw("TW description");
        assert_eq!(get_description(&entry), "TW description");
    }

    #[test]
    fn get_description_falls_back_to_caldav_summary() {
        let entry = make_entry_with_caldav("CalDAV summary");
        assert_eq!(get_description(&entry), "CalDAV summary");
    }

    #[test]
    fn get_description_falls_back_to_caldav_uid_when_no_summary() {
        let vtodo = VTODO {
            uid: "no-summary-uid".to_string(),
            summary: None,
            description: None,
            status: None,
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
        };
        let fetched = FetchedVTODO {
            href: "/cal/x.ics".to_string(),
            etag: None,
            vtodo,
        };
        let entry = IREntry {
            tw_uuid: None,
            caldav_uid: Some("no-summary-uid".to_string()),
            tw_task: None,
            fetched_vtodo: Some(fetched),
            resolved_depends: vec![],
            cyclic: false,
            calendar_url: None,
            dirty_tw: false,
            dirty_caldav: false,
            project: None,
        };
        assert_eq!(get_description(&entry), "no-summary-uid");
    }

    #[test]
    fn get_description_falls_back_to_uuid_string() {
        let uuid = Uuid::new_v4();
        let entry = IREntry {
            tw_uuid: Some(uuid),
            caldav_uid: None,
            tw_task: None,
            fetched_vtodo: None,
            resolved_depends: vec![],
            cyclic: false,
            calendar_url: None,
            dirty_tw: false,
            dirty_caldav: false,
            project: None,
        };
        assert_eq!(get_description(&entry), uuid.to_string());
    }

    #[test]
    fn get_description_unknown_when_nothing_available() {
        let entry = IREntry {
            tw_uuid: None,
            caldav_uid: None,
            tw_task: None,
            fetched_vtodo: None,
            resolved_depends: vec![],
            cyclic: false,
            calendar_url: None,
            dirty_tw: false,
            dirty_caldav: false,
            project: None,
        };
        assert_eq!(get_description(&entry), "unknown");
    }

    // -----------------------------------------------------------------------
    // format_skip_reason tests
    // -----------------------------------------------------------------------

    #[test]
    fn all_skip_reasons_have_non_empty_text() {
        let reasons = [
            SkipReason::Cancelled,
            SkipReason::Completed,
            SkipReason::Recurring,
            SkipReason::Cyclic,
            SkipReason::Identical,
            SkipReason::DeletedBeforeSync,
            SkipReason::AlreadyDeleted,
            SkipReason::CalDavDeletedTwTerminal,
        ];
        for reason in &reasons {
            let text = format_skip_reason(reason);
            assert!(!text.is_empty(), "empty text for reason: {:?}", reason);
        }
    }

    // -----------------------------------------------------------------------
    // Dry-run summary string format verification
    // -----------------------------------------------------------------------

    #[test]
    fn dry_run_summary_string_format() {
        let ops = vec![
            PlannedOp::PushToCalDav(make_entry_with_tw("a")),
            PlannedOp::PullFromCalDav(make_entry_with_caldav("b")),
            PlannedOp::ResolveConflict {
                entry: make_entry_with_tw("c"),
                winner: Side::Tw,
                reason: UpdateReason::LwwTwWins,
            },
            PlannedOp::DeleteFromCalDav(make_entry_with_tw("d")),
            PlannedOp::Skip {
                tw_uuid: Some(Uuid::new_v4()),
                reason: SkipReason::Identical,
            },
        ];
        let s = format_dry_run_summary(&ops);
        assert!(s.starts_with("[DRY-RUN] Would:"), "missing prefix: {}", s);
        assert!(s.contains("2 create(s)"), "expected 2 creates: {}", s);
        assert!(s.contains("1 update(s)"), "expected 1 update: {}", s);
        assert!(s.contains("1 delete(s)"), "expected 1 delete: {}", s);
        assert!(s.contains("1 skip(s)"), "expected 1 skip: {}", s);
    }

    // -----------------------------------------------------------------------
    // Dry-run summary count verification
    // -----------------------------------------------------------------------

    #[test]
    fn dry_run_summary_correct_counts() {
        // Test the count_ops logic that feeds into the dry-run summary line.
        let ops = vec![
            PlannedOp::PushToCalDav(make_entry_with_tw("task1")),
            PlannedOp::PullFromCalDav(make_entry_with_caldav("task2")),
            PlannedOp::ResolveConflict {
                entry: make_entry_with_tw("task3"),
                winner: Side::Tw,
                reason: UpdateReason::LwwTwWins,
            },
            PlannedOp::DeleteFromCalDav(make_entry_with_tw("task4")),
            PlannedOp::Skip {
                tw_uuid: Some(Uuid::new_v4()),
                reason: SkipReason::Identical,
            },
        ];
        let result = SyncResult {
            planned_ops: ops,
            ..empty_result()
        };

        let (cc, cu, tc, tu, d, s) = count_ops(&result.planned_ops);
        let total_creates = cc + tc;
        let total_updates = cu + tu;
        // 1 PushToCalDav + 1 PullFromCalDav = 2 creates
        assert_eq!(total_creates, 2, "creates");
        // 1 ResolveConflict(TW wins) → caldav update = 1 update
        assert_eq!(total_updates, 1, "updates");
        // 1 DeleteFromCalDav = 1 delete
        assert_eq!(d, 1, "deletes");
        // 1 Skip = 1 skip
        assert_eq!(s, 1, "skips");
    }

    // -----------------------------------------------------------------------
    // Live output breakdown matches SyncResult counters
    // -----------------------------------------------------------------------

    #[test]
    fn live_output_caldav_creates_match_written_caldav() {
        // In the common case (only PushToCalDav ops), caldav_creates should
        // equal written_caldav.
        let ops = vec![
            PlannedOp::PushToCalDav(make_entry_with_tw("task1")),
            PlannedOp::PushToCalDav(make_entry_with_tw("task2")),
            PlannedOp::PullFromCalDav(make_entry_with_caldav("task3")),
        ];
        let result = SyncResult {
            planned_ops: ops,
            written_caldav: 2,
            written_tw: 1,
            ..empty_result()
        };

        let (cc, _cu, tc, _tu, _d, _s) = count_ops(&result.planned_ops);
        assert_eq!(cc, result.written_caldav, "caldav creates match written_caldav");
        assert_eq!(tc, result.written_tw, "tw creates match written_tw");
    }

    // -----------------------------------------------------------------------
    // Warning formatting
    // -----------------------------------------------------------------------

    #[test]
    fn warning_with_uuid_format() {
        let uuid = Uuid::new_v4();
        let warn = Warning {
            tw_uuid: Some(uuid),
            message: "something went wrong".to_string(),
        };
        let prefix = match &warn.tw_uuid {
            Some(u) => format!("[WARN] [{}]", u),
            None => "[WARN]".to_string(),
        };
        let line = format!("{} {}", prefix, warn.message);
        assert!(line.starts_with("[WARN] ["), "expected uuid prefix: {}", line);
        assert!(line.contains(&uuid.to_string()), "uuid missing from line: {}", line);
        assert!(line.contains("something went wrong"), "message missing: {}", line);
    }

    #[test]
    fn warning_without_uuid_format() {
        let warn = Warning {
            tw_uuid: None,
            message: "generic warning".to_string(),
        };
        let prefix = match &warn.tw_uuid {
            Some(u) => format!("[WARN] [{}]", u),
            None => "[WARN]".to_string(),
        };
        let line = format!("{} {}", prefix, warn.message);
        assert_eq!(line, "[WARN] generic warning");
    }
}
