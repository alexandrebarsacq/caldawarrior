use std::collections::HashMap;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::caldav_adapter::CalDavClient;
use crate::error::CaldaWarriorError;
use crate::ical;
use crate::mapper::fields::{caldav_to_tw_fields, tw_to_caldav_fields};
use crate::mapper::status::{tw_to_caldav_status, TwToCalDavStatus};
use crate::sync::lww::resolve_lww;
use crate::types::{
    FetchedVTODO, IcalProp, IREntry, PlannedOp, RelType, Side, SkipReason, SyncResult, TWTask,
    TwAnnotation, UpdateReason, VTODO,
};
use crate::tw_adapter::{TaskRunner, TwAdapter};

const MAX_ETAG_RETRIES: usize = 3;

// ---------------------------------------------------------------------------
// Reverse index: CalDAV UID → TW UUID (for depends remapping)
// ---------------------------------------------------------------------------

fn build_caldav_index(ir: &[IREntry]) -> HashMap<String, Uuid> {
    ir.iter()
        .filter_map(|e| {
            if let (Some(uid), Some(uuid)) = (&e.caldav_uid, e.tw_uuid) {
                Some((uid.clone(), uuid))
            } else {
                None
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Annotation Slot Invariant helper
// ---------------------------------------------------------------------------

/// Merge CalDAV DESCRIPTION text into a TW annotations list.
///
/// The invariant: slot 0 is owned by CalDAV sync; slots 1+ are user-created
/// and must never be touched.
///
/// | annotations_text | base.len() | result                                    |
/// |------------------|------------|-------------------------------------------|
/// | None             | 0          | []                                        |
/// | None             | ≥1         | base unchanged                            |
/// | Some(t)          | 0          | [TwAnnotation(t)]                         |
/// | Some(t)          | 1, t same  | base unchanged (no-op)                    |
/// | Some(t)          | 1, t diff  | [TwAnnotation(t)]                         |
/// | Some(t)          | ≥2         | [TwAnnotation(t)] + base[1..]             |
fn merge_annotations(
    text: Option<&str>,
    base: Vec<TwAnnotation>,
    now: DateTime<Utc>,
) -> Vec<TwAnnotation> {
    match text {
        None => base,
        Some(t) => {
            let new_ann = TwAnnotation {
                entry: now,
                description: t.to_string(),
            };
            if base.is_empty() {
                vec![new_ann]
            } else if base[0].description == t {
                base // identical — no-op
            } else {
                let mut result = vec![new_ann];
                result.extend_from_slice(&base[1..]);
                result
            }
        }
    }
}

// ---------------------------------------------------------------------------
// VTODO construction (TW → CalDAV)
// ---------------------------------------------------------------------------

/// Build a VTODO from a TW task, merging with the existing CalDAV VTODO when
/// available (to preserve fields we do not manage).
fn build_vtodo_from_tw(entry: &IREntry, tw: &TWTask, now: DateTime<Utc>) -> VTODO {
    let fields = tw_to_caldav_fields(tw, now);
    let status_result = tw_to_caldav_status(tw);

    let (status_str, completed_dt) = match &status_result {
        TwToCalDavStatus::NeedsAction => (Some("NEEDS-ACTION".to_string()), None),
        TwToCalDavStatus::NeedsActionWithWait(_) => (Some("NEEDS-ACTION".to_string()), None),
        TwToCalDavStatus::Completed(dt) => (Some("COMPLETED".to_string()), Some(*dt)),
        TwToCalDavStatus::TwStateDeleted => (Some("CANCELLED".to_string()), None),
        TwToCalDavStatus::Skip(_) => (None, None),
    };

    let uid = entry
        .caldav_uid
        .clone()
        .unwrap_or_else(|| tw.uuid.to_string());

    // Preserve extra_props from existing VTODO; strip props we'll re-write.
    let mut extra_props: Vec<IcalProp> = entry
        .fetched_vtodo
        .as_ref()
        .map(|fv| fv.vtodo.extra_props.clone())
        .unwrap_or_default()
        .into_iter()
        .filter(|p| !p.name.eq_ignore_ascii_case("X-TASKWARRIOR-WAIT"))
        .collect();

    // Write X-TASKWARRIOR-WAIT if wait is still future.
    if let Some(wait_prop) = fields.wait {
        extra_props.push(wait_prop);
    }

    // base is retained for future field-mapping phases that may need CalDAV data.
    let _base = entry.fetched_vtodo.as_ref().map(|fv| &fv.vtodo);

    VTODO {
        uid,
        summary: fields.summary,
        description: fields.annotations,
        status: status_str,
        last_modified: Some(tw.modified.unwrap_or(tw.entry)),
        dtstamp: None,
        dtstart: fields.dtstart,
        due: fields.due,
        completed: completed_dt,
        categories: tw.tags.clone().unwrap_or_default(),
        rrule: None, // TW tasks are never recurring (recurring entries are skipped pre-writeback)
        priority: fields.priority,
        // Use resolved_depends (CalDAV UIDs from IR resolution phase) rather than
        // tw_to_caldav_fields().depends (raw TW UUIDs) as specified by the AC.
        depends: entry
            .resolved_depends
            .iter()
            .map(|uid| (RelType::DependsOn, uid.clone()))
            .collect(),
        extra_props,
    }
}

// ---------------------------------------------------------------------------
// TW task construction (CalDAV → TW)
// ---------------------------------------------------------------------------

/// Build a TW task from a CalDAV VTODO, merging with existing TW data when
/// available.
///
/// `caldav_uid_to_tw_uuid` is the IR reverse index used to translate CalDAV
/// `RELATED-TO;RELTYPE=DEPENDS-ON` UIDs back to TW UUIDs (the reverse-mapping
/// required by the spec).
fn build_tw_task_from_caldav(
    entry: &IREntry,
    caldav_uid_to_tw_uuid: &HashMap<String, Uuid>,
    now: DateTime<Utc>,
) -> TWTask {
    let vtodo = &entry
        .fetched_vtodo
        .as_ref()
        .expect("build_tw_task_from_caldav: no fetched_vtodo")
        .vtodo;

    let fields = caldav_to_tw_fields(vtodo);

    let status = match vtodo.status.as_deref().unwrap_or("NEEDS-ACTION") {
        "COMPLETED" => "completed",
        "CANCELLED" => "deleted",
        _ => "pending",
    };

    // Reverse-map CalDAV DEPENDS-ON UIDs → TW UUIDs via IR index.
    let depends: Vec<Uuid> = vtodo
        .depends
        .iter()
        .filter(|(rel, _)| matches!(rel, RelType::DependsOn))
        .filter_map(|(_, uid)| caldav_uid_to_tw_uuid.get(uid).copied())
        .collect();

    let tw_uuid = entry
        .tw_uuid
        .expect("build_tw_task_from_caldav: entry has no tw_uuid");

    let base = entry.tw_task.as_ref();

    TWTask {
        uuid: tw_uuid,
        status: status.to_string(),
        description: fields.description,
        entry: base.map(|t| t.entry).unwrap_or(now),
        modified: base.and_then(|t| t.modified),
        due: fields.due,
        scheduled: fields.scheduled,
        wait: fields.wait,
        until: base.and_then(|t| t.until),
        end: vtodo.completed,
        caldavuid: entry.caldav_uid.clone(),
        priority: fields.priority,
        project: base.map_or_else(|| entry.project.clone(), |t| t.project.clone()),
        tags: base.and_then(|t| t.tags.clone()),
        recur: None,
        urgency: base.and_then(|t| t.urgency),
        id: base.and_then(|t| t.id),
        depends,
        annotations: merge_annotations(
            fields.annotations_text.as_deref(),
            base.map_or_else(Vec::new, |t| t.annotations.clone()),
            now,
        ),
    }
}

// ---------------------------------------------------------------------------
// Decision tree
// ---------------------------------------------------------------------------

/// Classify one IREntry into a PlannedOp.
/// Returns `None` only for degenerate entries with neither TW nor CalDAV data.
/// CalDAV-only terminal entries (COMPLETED/CANCELLED) now emit explicit
/// `PlannedOp::Skip { tw_uuid: None, reason }` so all SkipReason variants are used.
fn decide_op(entry: &IREntry, now: DateTime<Utc>) -> Option<PlannedOp> {
    let tw_opt = entry.tw_task.as_ref();
    let caldav_opt = entry.fetched_vtodo.as_ref().map(|fv| &fv.vtodo);

    match (tw_opt, caldav_opt) {
        // ── Paired: both TW and CalDAV exist ─────────────────────────────────
        (Some(tw), Some(vtodo)) => {
            let tw_uuid = entry.tw_uuid.expect("paired entry must have tw_uuid");
            let tw_status = tw.status.as_str();
            let caldav_status = vtodo.status.as_deref().unwrap_or("NEEDS-ACTION");

            // Both sides already at their terminal deleted/cancelled state.
            if tw_status == "deleted" && caldav_status == "CANCELLED" {
                return Some(PlannedOp::Skip {
                    tw_uuid: Some(tw_uuid),
                    reason: SkipReason::AlreadyDeleted,
                });
            }

            // TW was deleted → mark CalDAV CANCELLED.
            if tw_status == "deleted" {
                return Some(PlannedOp::ResolveConflict {
                    entry: entry.clone(),
                    winner: Side::Tw,
                    reason: UpdateReason::TwDeletedMarkCancelled,
                });
            }

            // CalDAV was cancelled → skip (do not propagate to TW).
            // If TW is also in a terminal state (completed), use CalDavDeletedTwTerminal
            // to distinguish from the case where TW is still active.
            if caldav_status == "CANCELLED" {
                let reason = if tw_status == "completed" {
                    SkipReason::CalDavDeletedTwTerminal
                } else {
                    SkipReason::Cancelled
                };
                return Some(PlannedOp::Skip {
                    tw_uuid: Some(tw_uuid),
                    reason,
                });
            }

            // TW was completed → mark CalDAV COMPLETED (if not already).
            if tw_status == "completed" && caldav_status != "COMPLETED" {
                return Some(PlannedOp::ResolveConflict {
                    entry: entry.clone(),
                    winner: Side::Tw,
                    reason: UpdateReason::TwCompletedMarkCompleted,
                });
            }

            // CalDAV COMPLETED → update TW to completed (if not already).
            if caldav_status == "COMPLETED" && tw_status != "completed" {
                return Some(PlannedOp::ResolveConflict {
                    entry: entry.clone(),
                    winner: Side::CalDav,
                    reason: UpdateReason::CalDavCompletedUpdateTw,
                });
            }

            // General case: LWW conflict resolution (includes 8-field identical check).
            Some(resolve_lww(entry.clone(), now))
        }

        // ── TW-only: push to CalDAV or handle orphan ─────────────────────────
        (Some(tw), None) => {
            let tw_uuid = entry.tw_uuid.expect("tw-only entry must have tw_uuid");

            if tw.status == "deleted" {
                return Some(PlannedOp::Skip {
                    tw_uuid: Some(tw_uuid),
                    reason: SkipReason::DeletedBeforeSync,
                });
            }
            if tw.status == "recurring" {
                return Some(PlannedOp::Skip {
                    tw_uuid: Some(tw_uuid),
                    reason: SkipReason::Recurring,
                });
            }
            // Orphaned: caldavuid is set but the CalDAV VTODO no longer exists.
            // The external deletion is treated as authoritative → delete from TW.
            if tw.caldavuid.is_some() {
                return Some(PlannedOp::DeleteFromTw(entry.clone()));
            }
            Some(PlannedOp::PushToCalDav(entry.clone()))
        }

        // ── CalDAV-only: create in TW (if active) ────────────────────────────
        (None, Some(vtodo)) => {
            let status = vtodo.status.as_deref().unwrap_or("NEEDS-ACTION");
            match status {
                // Terminal CalDAV-only entries have tw_uuid=None (never assigned by build_ir).
                "COMPLETED" => Some(PlannedOp::Skip {
                    tw_uuid: None,
                    reason: SkipReason::Completed,
                }),
                "CANCELLED" => Some(PlannedOp::Skip {
                    tw_uuid: None,
                    reason: SkipReason::Cancelled,
                }),
                // Active entries have a pre-assigned tw_uuid from build_ir.
                _ => Some(PlannedOp::PullFromCalDav(entry.clone())),
            }
        }

        (None, None) => None,
    }
}

// ---------------------------------------------------------------------------
// CalDAV write helper (with ETag retry)
// ---------------------------------------------------------------------------

/// Construct the CalDAV href for this entry.
/// For existing entries: use the stored href.
/// For new TW-only entries: `{calendar_url}/{caldav_uid}.ics`.
fn entry_href(entry: &IREntry) -> String {
    if let Some(fv) = &entry.fetched_vtodo {
        return fv.href.clone();
    }
    let cal_url = entry.calendar_url.as_deref().unwrap_or("");
    let uid = entry.caldav_uid.as_deref().unwrap_or("");
    format!("{}/{}.ics", cal_url.trim_end_matches('/'), uid)
}

/// Attempt a CalDAV PUT. Returns `Ok(true)` on success, `Ok(false)` on
/// EtagConflict (caller should retry after updating `entry.fetched_vtodo`).
fn try_put_caldav(
    entry: &mut IREntry,
    vtodo: VTODO,
    caldav: &dyn CalDavClient,
    dry_run: bool,
    result: &mut SyncResult,
) -> Result<bool, CaldaWarriorError> {
    if dry_run {
        result.written_caldav += 1;
        return Ok(true);
    }

    let href = entry_href(entry);
    let etag = entry.fetched_vtodo.as_ref().and_then(|fv| fv.etag.as_deref());
    let ical_content = ical::to_icalendar_string(&vtodo);

    match caldav.put_vtodo(&href, &ical_content, etag) {
        Ok(new_etag) => {
            // Update stored ETag if the server returned one.
            if let Some(fv) = &mut entry.fetched_vtodo {
                fv.etag = new_etag.clone();
                fv.vtodo = vtodo.clone();
            } else {
                entry.fetched_vtodo = Some(FetchedVTODO {
                    href: href.clone(),
                    etag: new_etag,
                    vtodo: vtodo.clone(),
                });
            }
            result.written_caldav += 1;
            Ok(true)
        }
        Err(CaldaWarriorError::EtagConflict { refetched_vtodo }) => {
            // Update entry with the server's current VTODO so the next
            // loop iteration re-evaluates with fresh data.
            entry.fetched_vtodo = Some(refetched_vtodo);
            Ok(false)
        }
        Err(e) => Err(e),
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Apply write-back operations for every entry in the IR.
///
/// Ownership of ETag retry lives exclusively here (not in the orchestrator).
/// Each entry is retried up to `MAX_ETAG_RETRIES` times on `EtagConflict`;
/// after exhaustion the entry's error is accumulated in `SyncResult.errors`
/// and the sync continues with the next entry.
///
/// `now` is injected for deterministic testing (avoids wall-clock dependency).
///
/// # CalDAV-only routing
/// - `NEEDS-ACTION` / `IN-PROCESS`: `tw.create()` with the pre-assigned `tw_uuid`
///   (the ONLY code path that calls task import).
/// - `COMPLETED` / `CANCELLED`: silently skipped (no TW task created).
///
/// # Depends reverse-mapping
/// When updating a TW task from CalDAV, `RELATED-TO;RELTYPE=DEPENDS-ON` UIDs
/// are mapped back to TW UUIDs via an IR index built from
/// `(entry.caldav_uid → entry.tw_uuid)` pairs.
pub fn apply_writeback<R: TaskRunner>(
    ir: &mut Vec<IREntry>,
    tw: &TwAdapter<R>,
    caldav: &dyn CalDavClient,
    dry_run: bool,
    fail_fast: bool,
    now: DateTime<Utc>,
) -> SyncResult {
    let mut result = SyncResult {
        planned_ops: vec![],
        warnings: vec![],
        errors: vec![],
        written_tw: 0,
        written_caldav: 0,
        skipped: 0,
    };

    // Build reverse index once for the full IR pass.
    let caldav_index = build_caldav_index(ir);

    for entry in ir.iter_mut() {
        apply_entry(entry, &caldav_index, tw, caldav, dry_run, now, &mut result);
        if fail_fast && !result.errors.is_empty() {
            break;
        }
    }

    result
}

fn apply_entry<R: TaskRunner>(
    entry: &mut IREntry,
    caldav_index: &HashMap<String, Uuid>,
    tw: &TwAdapter<R>,
    caldav: &dyn CalDavClient,
    dry_run: bool,
    now: DateTime<Utc>,
    result: &mut SyncResult,
) {
    // Cyclic entries sync normally but WITHOUT dependency relations.
    // Clear resolved_depends so build_vtodo_from_tw produces no RELATED-TO.
    // This is done before decide_op to cover ALL branches (paired, TW-only).
    if entry.cyclic {
        entry.resolved_depends.clear();
    }

    for attempt in 0..MAX_ETAG_RETRIES {
        let op = match decide_op(entry, now) {
            Some(op) => op,
            None => {
                // Degenerate entry (neither TW nor CalDAV data) — nothing to do.
                return;
            }
        };

        // Record the planned op in results (first attempt only to avoid duplicates).
        if attempt == 0 {
            result.planned_ops.push(op.clone());
        }

        let retry = execute_op(entry, op, caldav_index, tw, caldav, dry_run, now, result);

        match retry {
            Ok(true) => return,       // Success.
            Ok(false) => {
                // EtagConflict: entry.fetched_vtodo updated; retry decision.
                if attempt + 1 >= MAX_ETAG_RETRIES {
                    result.errors.push(format!(
                        "SyncConflict: ETag conflict unresolved after {} attempts \
                         (tw_uuid={:?}, caldav_uid={:?}, href={:?})",
                        MAX_ETAG_RETRIES,
                        entry.tw_uuid,
                        entry.caldav_uid,
                        entry.fetched_vtodo.as_ref().map(|fv| &fv.href),
                    ));
                    return;
                }
                // Continue to next attempt.
            }
            Err(e) => {
                result.errors.push(format!(
                    "{} (tw_uuid={:?}, caldav_uid={:?})",
                    e, entry.tw_uuid, entry.caldav_uid
                ));
                return;
            }
        }
    }
}

/// Execute a `PlannedOp`. Returns:
/// - `Ok(true)` on success or skip (no retry needed)
/// - `Ok(false)` on `EtagConflict` (caller should retry)
/// - `Err(e)` on non-retriable errors
fn execute_op<R: TaskRunner>(
    entry: &mut IREntry,
    op: PlannedOp,
    caldav_index: &HashMap<String, Uuid>,
    tw: &TwAdapter<R>,
    caldav: &dyn CalDavClient,
    dry_run: bool,
    now: DateTime<Utc>,
    result: &mut SyncResult,
) -> Result<bool, CaldaWarriorError> {
    match op {
        // ── Push TW → CalDAV ─────────────────────────────────────────────────
        PlannedOp::PushToCalDav(ref e) => {
            let tw_task = e.tw_task.as_ref().expect("PushToCalDav: no tw_task");
            let vtodo = build_vtodo_from_tw(entry, tw_task, now);

            // After CalDAV write, update TW task with the assigned caldavuid
            // (for newly created entries that didn't have caldavuid set yet).
            let needs_tw_caldavuid_update = tw_task.caldavuid.is_none();
            let caldav_uid = entry.caldav_uid.clone();

            let pushed = try_put_caldav(entry, vtodo, caldav, dry_run, result)?;
            if pushed && needs_tw_caldavuid_update {
                if let Some(ref mut tw_task_mut) = entry.tw_task {
                    tw_task_mut.caldavuid = caldav_uid;
                    if !dry_run {
                        tw.update(tw_task_mut)?;
                        result.written_tw += 1;
                    }
                }
            }
            Ok(pushed)
        }

        // ── Pull CalDAV → TW ─────────────────────────────────────────────────
        PlannedOp::PullFromCalDav(ref e) => {
            let tw_task = build_tw_task_from_caldav(e, caldav_index, now);
            if !dry_run {
                if e.tw_task.is_none() {
                    // CalDAV-only new: tw.create() — the ONLY path calling task import.
                    tw.create(&tw_task)?;
                } else {
                    // Existing TW task: tw.update() — NEVER calls task import.
                    tw.update(&tw_task)?;
                }
                result.written_tw += 1;
            }
            Ok(true)
        }

        // ── Delete from CalDAV ───────────────────────────────────────────────
        PlannedOp::DeleteFromCalDav(ref e) => {
            if !dry_run {
                let href = entry_href(e);
                let etag = e.fetched_vtodo.as_ref().and_then(|fv| fv.etag.as_deref());
                match caldav.delete_vtodo(&href, etag) {
                    Ok(()) => result.written_caldav += 1,
                    Err(CaldaWarriorError::EtagConflict { refetched_vtodo }) => {
                        entry.fetched_vtodo = Some(refetched_vtodo);
                        return Ok(false); // Retry.
                    }
                    Err(e) => return Err(e),
                }
            }
            Ok(true)
        }

        // ── Delete from TW ───────────────────────────────────────────────────
        PlannedOp::DeleteFromTw(ref e) => {
            if let Some(uuid) = e.tw_uuid {
                if !dry_run {
                    tw.delete(&uuid)?;
                    result.written_tw += 1;
                }
            }
            Ok(true)
        }

        // ── Resolve conflict ─────────────────────────────────────────────────
        PlannedOp::ResolveConflict {
            entry: ref e,
            ref winner,
            ref reason,
        } => {
            match (winner, reason) {
                // TW wins: push TW state to CalDAV.
                (Side::Tw, UpdateReason::LwwTwWins)
                | (Side::Tw, UpdateReason::TwCompletedMarkCompleted)
                | (Side::Tw, UpdateReason::TwDeletedMarkCancelled) => {
                    let tw_task = e
                        .tw_task
                        .as_ref()
                        .expect("ResolveConflict TwWins: no tw_task");
                    let vtodo = build_vtodo_from_tw(entry, tw_task, now);
                    try_put_caldav(entry, vtodo, caldav, dry_run, result)
                }

                // CalDAV wins: update TW from CalDAV data.
                (Side::CalDav, UpdateReason::LwwCalDavWins)
                | (Side::CalDav, UpdateReason::CalDavCompletedUpdateTw) => {
                    let tw_task = build_tw_task_from_caldav(e, caldav_index, now);
                    if !dry_run {
                        // Always `update` here: existing task on TW side.
                        tw.update(&tw_task)?;
                        result.written_tw += 1;
                    }
                    Ok(true)
                }

                // Fallback for any unhandled combination.
                _ => Ok(true),
            }
        }

        // ── Skip ─────────────────────────────────────────────────────────────
        PlannedOp::Skip { .. } => {
            result.skipped += 1;
            Ok(true)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::caldav_adapter::{CalDavCall, MockCalDavClient};
    use crate::tw_adapter::{MockTaskRunner, TwAdapter};
    use crate::types::{FetchedVTODO, IREntry, TWTask, VTODO};
    use chrono::TimeZone;
    use uuid::Uuid;

    fn t(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, s).unwrap()
    }

    fn make_tw_task(uuid: Uuid, status: &str, description: &str, modified: DateTime<Utc>) -> TWTask {
        TWTask {
            uuid,
            status: status.to_string(),
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

    fn make_vtodo(uid: &str, status: &str, last_modified: DateTime<Utc>) -> VTODO {
        VTODO {
            uid: uid.to_string(),
            summary: Some("Task".to_string()),
            description: None, // no annotations on test tasks
            status: Some(status.to_string()),
            last_modified: Some(last_modified),
            dtstamp: None,
            dtstart: None,
            due: None,
            completed: None,
            categories: vec![],
            rrule: None,
            priority: None,
            depends: vec![],
            extra_props: vec![],
        }
    }

    fn make_paired_entry(
        uuid: Uuid,
        caldav_uid: &str,
        tw_status: &str,
        tw_modified: DateTime<Utc>,
        caldav_status: &str,
        caldav_lm: DateTime<Utc>,
    ) -> IREntry {
        let vtodo = make_vtodo(caldav_uid, caldav_status, caldav_lm);
        IREntry {
            tw_uuid: Some(uuid),
            caldav_uid: Some(caldav_uid.to_string()),
            tw_task: Some(make_tw_task(uuid, tw_status, "Task", tw_modified)),
            fetched_vtodo: Some(FetchedVTODO {
                href: format!("/{}.ics", caldav_uid),
                etag: Some("\"etag1\"".to_string()),
                vtodo,
            }),
            resolved_depends: vec![],
            cyclic: false,
            calendar_url: Some("https://dav.example.com/cal/".to_string()),
            dirty_tw: false,
            dirty_caldav: false,
            project: None,
        }
    }

    fn make_tw_only_entry(uuid: Uuid, caldav_uid: &str, status: &str) -> IREntry {
        let mut task = make_tw_task(uuid, status, "TW-only task", t(2026, 2, 1, 10, 0, 0));
        task.caldavuid = None;
        IREntry {
            tw_uuid: Some(uuid),
            caldav_uid: Some(caldav_uid.to_string()),
            tw_task: Some(task),
            fetched_vtodo: None,
            resolved_depends: vec![],
            cyclic: false,
            calendar_url: Some("https://dav.example.com/cal/".to_string()),
            dirty_tw: false,
            dirty_caldav: false,
            project: None,
        }
    }

    fn make_caldav_only_entry(caldav_uid: &str, status: &str, tw_uuid: Option<Uuid>) -> IREntry {
        IREntry {
            tw_uuid,
            caldav_uid: Some(caldav_uid.to_string()),
            tw_task: None,
            fetched_vtodo: Some(FetchedVTODO {
                href: format!("/{}.ics", caldav_uid),
                etag: Some("\"etag2\"".to_string()),
                vtodo: make_vtodo(caldav_uid, status, t(2026, 2, 1, 8, 0, 0)),
            }),
            resolved_depends: vec![],
            cyclic: false,
            calendar_url: Some("https://dav.example.com/cal/".to_string()),
            dirty_tw: false,
            dirty_caldav: false,
            project: None,
        }
    }

    fn make_tw_adapter(mock: MockTaskRunner) -> TwAdapter<MockTaskRunner> {
        // Pre-load UDA registration responses.
        mock.push_run_response(Ok(String::new())); // uda type
        mock.push_run_response(Ok(String::new())); // uda label
        TwAdapter::new(mock).expect("TwAdapter::new")
    }

    // ── Tests ─────────────────────────────────────────────────────────────────

    // ── Unit tests for build_vtodo_from_tw / build_tw_task_from_caldav ────────

    #[test]
    fn build_vtodo_from_tw_uses_summary_not_description() {
        let uuid = Uuid::new_v4();
        let caldav_uid = Uuid::new_v4().to_string();
        let mut entry = make_tw_only_entry(uuid, &caldav_uid, "pending");
        let tw = entry.tw_task.as_mut().unwrap();
        tw.description = "Buy milk".to_string();
        tw.annotations = vec![]; // no annotations → vtodo.description should be None
        let tw_snapshot = entry.tw_task.clone().unwrap();

        let now = t(2026, 2, 2, 0, 0, 0);
        let vtodo = build_vtodo_from_tw(&entry, &tw_snapshot, now);

        assert_eq!(vtodo.summary, Some("Buy milk".to_string()));
        assert!(vtodo.description.is_none(), "no annotations → DESCRIPTION must be None");
    }

    #[test]
    fn build_tw_task_caldav_only_injects_project() {
        let uuid = Uuid::new_v4();
        let caldav_uid = Uuid::new_v4().to_string();
        let mut entry = make_caldav_only_entry(&caldav_uid, "NEEDS-ACTION", Some(uuid));
        entry.project = Some("work".to_string());
        // vtodo summary so description is mapped correctly
        if let Some(ref mut fv) = entry.fetched_vtodo {
            fv.vtodo.summary = Some("A task".to_string());
        }

        let now = t(2026, 2, 2, 0, 0, 0);
        let tw_task = build_tw_task_from_caldav(&entry, &HashMap::new(), now);

        assert_eq!(tw_task.project, Some("work".to_string()));
    }

    #[test]
    fn build_tw_task_reads_summary_as_description() {
        let uuid = Uuid::new_v4();
        let caldav_uid = Uuid::new_v4().to_string();
        let mut entry = make_caldav_only_entry(&caldav_uid, "NEEDS-ACTION", Some(uuid));
        if let Some(ref mut fv) = entry.fetched_vtodo {
            fv.vtodo.summary = Some("X".to_string());
        }

        let now = t(2026, 2, 2, 0, 0, 0);
        let tw_task = build_tw_task_from_caldav(&entry, &HashMap::new(), now);

        assert_eq!(tw_task.description, "X");
    }

    // ── Integration-style tests (apply_writeback) ─────────────────────────────

    #[test]
    fn tw_only_pending_pushes_to_caldav() {
        let uuid = Uuid::new_v4();
        let caldav_uid = Uuid::new_v4().to_string();
        let mut ir = vec![make_tw_only_entry(uuid, &caldav_uid, "pending")];

        let mock_tw = MockTaskRunner::new();
        mock_tw.push_run_response(Ok(String::new())); // uda type
        mock_tw.push_run_response(Ok(String::new())); // uda label
        mock_tw.push_run_response(Ok(String::new())); // tw.update() for caldavuid
        let tw = TwAdapter::new(mock_tw).expect("TwAdapter");

        let caldav = MockCalDavClient::new();
        let now = t(2026, 2, 2, 0, 0, 0);
        let result = apply_writeback(&mut ir, &tw, &caldav, false, false, now);

        assert_eq!(result.written_caldav, 1);
        let calls = caldav.calls.lock().unwrap();
        assert!(calls.iter().any(|c| matches!(c, CalDavCall::Put { .. })));
    }

    #[test]
    fn tw_only_deleted_skips_without_write() {
        let uuid = Uuid::new_v4();
        let caldav_uid = Uuid::new_v4().to_string();
        let mut ir = vec![make_tw_only_entry(uuid, &caldav_uid, "deleted")];

        let tw = make_tw_adapter(MockTaskRunner::new());
        let caldav = MockCalDavClient::new();
        let result = apply_writeback(&mut ir, &tw, &caldav, false, false, t(2026, 2, 2, 0, 0, 0));

        assert_eq!(result.skipped, 1);
        assert_eq!(result.written_caldav, 0);
    }

    #[test]
    fn caldav_only_needs_action_creates_tw_task() {
        let uuid = Uuid::new_v4();
        let caldav_uid = Uuid::new_v4().to_string();
        let mut ir = vec![make_caldav_only_entry(&caldav_uid, "NEEDS-ACTION", Some(uuid))];

        let mock_tw = MockTaskRunner::new();
        mock_tw.push_run_response(Ok(String::new())); // uda type
        mock_tw.push_run_response(Ok(String::new())); // uda label
        mock_tw.push_import_response(Ok(String::new())); // tw.create() → import
        let tw = TwAdapter::new(mock_tw).expect("TwAdapter");

        let caldav = MockCalDavClient::new();
        let result = apply_writeback(
            &mut ir,
            &tw,
            &caldav,
            false,
            false,
            t(2026, 2, 2, 0, 0, 0),
        );

        assert_eq!(result.written_tw, 1, "CalDAV-only NEEDS-ACTION must call tw.create()");
        assert_eq!(result.written_caldav, 0);
    }

    #[test]
    fn caldav_only_completed_silently_skipped() {
        let caldav_uid = Uuid::new_v4().to_string();
        // tw_uuid=None for COMPLETED entries (as per build_ir spec)
        let mut ir = vec![make_caldav_only_entry(&caldav_uid, "COMPLETED", None)];

        let tw = make_tw_adapter(MockTaskRunner::new());
        let caldav = MockCalDavClient::new();
        let result = apply_writeback(&mut ir, &tw, &caldav, false, false, t(2026, 2, 2, 0, 0, 0));

        assert_eq!(result.skipped, 1);
        assert_eq!(result.written_tw, 0);
    }

    #[test]
    fn paired_tw_wins_pushes_to_caldav() {
        let uuid = Uuid::new_v4();
        let caldav_uid = Uuid::new_v4().to_string();
        // TW modified more recently than CalDAV last_modified → TW wins
        let tw_modified = t(2026, 2, 1, 11, 0, 0);
        let caldav_lm = t(2026, 2, 1, 9, 0, 0);

        let mut entry = make_paired_entry(
            uuid, &caldav_uid, "pending", tw_modified,
            "NEEDS-ACTION", caldav_lm,
        );
        // Make content differ so Layer 2 doesn't short-circuit to Identical.
        if let Some(ref mut fv) = entry.fetched_vtodo {
            fv.vtodo.summary = Some("Old summary".to_string());
            fv.vtodo.description = Some("Old summary".to_string());
        }
        let mut ir = vec![entry];

        let tw = make_tw_adapter(MockTaskRunner::new());
        let caldav = MockCalDavClient::new();
        let result = apply_writeback(&mut ir, &tw, &caldav, false, false, t(2026, 2, 2, 0, 0, 0));

        assert_eq!(result.written_caldav, 1);
    }

    #[test]
    fn paired_caldav_wins_updates_tw() {
        let uuid = Uuid::new_v4();
        let caldav_uid = Uuid::new_v4().to_string();
        // CalDAV last_modified is newer than TW modified → CalDAV wins
        let tw_modified = t(2026, 2, 1, 8, 0, 0);
        let caldav_lm = t(2026, 2, 1, 11, 0, 0);

        let mut entry = make_paired_entry(
            uuid, &caldav_uid, "pending", tw_modified,
            "NEEDS-ACTION", caldav_lm,
        );
        // Content must differ to avoid Identical shortcut.
        if let Some(ref mut fv) = entry.fetched_vtodo {
            fv.vtodo.summary = Some("Updated CalDAV task".to_string());
            fv.vtodo.description = Some("Updated CalDAV task".to_string());
        }
        let mut ir = vec![entry];

        let mock_tw = MockTaskRunner::new();
        mock_tw.push_run_response(Ok(String::new())); // uda type
        mock_tw.push_run_response(Ok(String::new())); // uda label
        mock_tw.push_run_response(Ok(String::new())); // tw.update()
        let tw = TwAdapter::new(mock_tw).expect("TwAdapter");

        let caldav = MockCalDavClient::new();
        let result = apply_writeback(&mut ir, &tw, &caldav, false, false, t(2026, 2, 2, 0, 0, 0));

        assert_eq!(result.written_tw, 1);
        assert_eq!(result.written_caldav, 0);
    }

    #[test]
    fn paired_identical_skips() {
        let uuid = Uuid::new_v4();
        let caldav_uid = Uuid::new_v4().to_string();
        let ts = t(2026, 2, 1, 10, 0, 0);

        let entry = make_paired_entry(
            uuid, &caldav_uid, "pending", ts,
            "NEEDS-ACTION", ts,
        );
        // Both sides have same description "Task" — Layer 2 fires.
        let mut ir = vec![entry];

        let tw = make_tw_adapter(MockTaskRunner::new());
        let caldav = MockCalDavClient::new();
        let result = apply_writeback(&mut ir, &tw, &caldav, false, false, t(2026, 2, 2, 0, 0, 0));

        assert_eq!(result.skipped, 1);
        assert_eq!(result.written_caldav, 0);
        assert_eq!(result.written_tw, 0);
    }

    #[test]
    fn etag_conflict_retries_and_exhausts() {
        let uuid = Uuid::new_v4();
        let caldav_uid = Uuid::new_v4().to_string();
        let tw_modified = t(2026, 2, 1, 11, 0, 0);
        let caldav_lm = t(2026, 2, 1, 9, 0, 0);

        let mut entry = make_paired_entry(
            uuid, &caldav_uid, "pending", tw_modified,
            "NEEDS-ACTION", caldav_lm,
        );
        // Different content so TW wins (content not identical).
        if let Some(ref mut fv) = entry.fetched_vtodo {
            fv.vtodo.summary = Some("Old".to_string());
            fv.vtodo.description = Some("Old".to_string());
        }
        let mut ir = vec![entry];

        let tw = make_tw_adapter(MockTaskRunner::new());
        let caldav = MockCalDavClient::new();

        // Queue MAX_ETAG_RETRIES EtagConflict responses.
        let refetched = FetchedVTODO {
            href: format!("/{}.ics", caldav_uid),
            etag: Some("\"new-etag\"".to_string()),
            vtodo: {
                let mut v = make_vtodo(&caldav_uid, "NEEDS-ACTION", t(2026, 2, 1, 9, 0, 0));
                // Keep different content so each retry re-decides TW wins.
                v.summary = Some("Old".to_string());
                v.description = Some("Old".to_string());
                v
            },
        };
        for _ in 0..MAX_ETAG_RETRIES {
            caldav.put_responses.lock().unwrap().push(Err(
                CaldaWarriorError::EtagConflict {
                    refetched_vtodo: refetched.clone(),
                },
            ));
        }

        let result = apply_writeback(&mut ir, &tw, &caldav, false, false, t(2026, 2, 2, 0, 0, 0));

        assert_eq!(result.written_caldav, 0);
        assert_eq!(result.errors.len(), 1, "one SyncConflict error after retry exhaustion");
        assert!(result.errors[0].contains("SyncConflict") || result.errors[0].contains("ETag"));
    }

    #[test]
    fn dry_run_does_not_write() {
        let uuid = Uuid::new_v4();
        let caldav_uid = Uuid::new_v4().to_string();
        let mut ir = vec![make_tw_only_entry(uuid, &caldav_uid, "pending")];

        let tw = make_tw_adapter(MockTaskRunner::new());
        let caldav = MockCalDavClient::new();
        let result = apply_writeback(&mut ir, &tw, &caldav, true, false, t(2026, 2, 2, 0, 0, 0));

        // dry_run counts the write but doesn't actually PUT.
        assert_eq!(result.written_caldav, 1);
        let calls = caldav.calls.lock().unwrap();
        assert!(calls.is_empty(), "dry_run must not make real CalDAV calls");
    }

    #[test]
    fn orphaned_caldavuid_deletes_tw_task() {
        // TW task has caldavuid but no matching VTODO → orphaned → DeleteFromTw.
        let uuid = Uuid::new_v4();
        let caldav_uid = Uuid::new_v4().to_string();
        let mut entry = make_tw_only_entry(uuid, &caldav_uid, "pending");
        // Mark the task as already synced (caldavuid set on the TW task itself).
        if let Some(ref mut tw) = entry.tw_task {
            tw.caldavuid = Some(caldav_uid.clone());
        }
        let mut ir = vec![entry];

        let mock_tw = MockTaskRunner::new();
        mock_tw.push_run_response(Ok(String::new())); // uda type
        mock_tw.push_run_response(Ok(String::new())); // uda label
        mock_tw.push_run_response(Ok(String::new())); // tw.delete()
        let tw = TwAdapter::new(mock_tw).expect("TwAdapter");

        let caldav = MockCalDavClient::new();
        let result = apply_writeback(&mut ir, &tw, &caldav, false, false, t(2026, 2, 2, 0, 0, 0));

        assert_eq!(result.written_tw, 1, "orphaned task should be deleted from TW");
        assert_eq!(result.written_caldav, 0, "orphaned task must NOT be pushed to CalDAV");
        assert_eq!(result.errors.len(), 0);
        let calls = caldav.calls.lock().unwrap();
        assert!(calls.is_empty(), "no CalDAV calls for orphaned deletion");
    }

    #[test]
    fn tw_deleted_marks_caldav_cancelled() {
        let uuid = Uuid::new_v4();
        let caldav_uid = Uuid::new_v4().to_string();
        let entry = make_paired_entry(
            uuid, &caldav_uid, "deleted", t(2026, 2, 1, 10, 0, 0),
            "NEEDS-ACTION", t(2026, 2, 1, 9, 0, 0),
        );
        let mut ir = vec![entry];

        let tw = make_tw_adapter(MockTaskRunner::new());
        let caldav = MockCalDavClient::new();
        let result = apply_writeback(&mut ir, &tw, &caldav, false, false, t(2026, 2, 2, 0, 0, 0));

        assert_eq!(result.written_caldav, 1);
        let calls = caldav.calls.lock().unwrap();
        assert!(calls.iter().any(|c| matches!(c, CalDavCall::Put { .. })));
    }

    #[test]
    fn cyclic_entry_synced_without_deps() {
        let uuid = Uuid::new_v4();
        let caldav_uid = Uuid::new_v4().to_string();
        let mut entry = make_paired_entry(
            uuid, &caldav_uid, "pending", t(2026, 2, 1, 10, 0, 0),
            "NEEDS-ACTION", t(2026, 2, 1, 9, 0, 0),
        );
        entry.cyclic = true;
        entry.resolved_depends = vec!["some-uid".to_string()];
        // Make content differ so LWW doesn't short-circuit to Identical.
        if let Some(ref mut fv) = entry.fetched_vtodo {
            fv.vtodo.summary = Some("Old summary".to_string());
            fv.vtodo.description = Some("Old summary".to_string());
        }
        let mut ir = vec![entry];

        let tw = make_tw_adapter(MockTaskRunner::new());
        let caldav = MockCalDavClient::new();
        let result = apply_writeback(&mut ir, &tw, &caldav, false, false, t(2026, 2, 2, 0, 0, 0));

        // Cyclic entry IS written to CalDAV (not skipped).
        assert_eq!(result.written_caldav, 1, "cyclic entry must be written to CalDAV");
        assert_eq!(result.skipped, 0, "cyclic entry must NOT be skipped");

        // CalDAV mock received a Put call.
        let calls = caldav.calls.lock().unwrap();
        assert!(calls.iter().any(|c| matches!(c, CalDavCall::Put { .. })),
            "expected a Put call for the cyclic entry");

        // After writeback, resolved_depends should have been cleared (no RELATED-TO in VTODO).
        assert!(ir[0].resolved_depends.is_empty(),
            "resolved_depends must be cleared for cyclic entries");
    }

    #[test]
    fn cyclic_tw_only_entry_pushed_without_deps() {
        let uuid = Uuid::new_v4();
        let caldav_uid = Uuid::new_v4().to_string();
        let mut entry = make_tw_only_entry(uuid, &caldav_uid, "pending");
        entry.cyclic = true;
        entry.resolved_depends = vec!["some-uid".to_string()];
        let mut ir = vec![entry];

        let mock_tw = MockTaskRunner::new();
        mock_tw.push_run_response(Ok(String::new())); // uda type
        mock_tw.push_run_response(Ok(String::new())); // uda label
        mock_tw.push_run_response(Ok(String::new())); // tw.update() for caldavuid
        let tw = TwAdapter::new(mock_tw).expect("TwAdapter");

        let caldav = MockCalDavClient::new();
        let result = apply_writeback(&mut ir, &tw, &caldav, false, false, t(2026, 2, 2, 0, 0, 0));

        // TW-only cyclic entry IS pushed to CalDAV.
        assert_eq!(result.written_caldav, 1, "TW-only cyclic entry must be pushed to CalDAV");
        assert_eq!(result.skipped, 0, "TW-only cyclic entry must NOT be skipped");

        // CalDAV mock received a Put call.
        let calls = caldav.calls.lock().unwrap();
        assert!(calls.iter().any(|c| matches!(c, CalDavCall::Put { .. })),
            "expected a Put call for the TW-only cyclic entry");

        // resolved_depends should have been cleared.
        assert!(ir[0].resolved_depends.is_empty(),
            "resolved_depends must be cleared for TW-only cyclic entries");
    }

    #[test]
    fn non_cyclic_entry_preserves_resolved_depends() {
        let uuid = Uuid::new_v4();
        let caldav_uid = Uuid::new_v4().to_string();
        // TW modified more recently than CalDAV → TW wins LWW.
        let mut entry = make_paired_entry(
            uuid, &caldav_uid, "pending", t(2026, 2, 1, 11, 0, 0),
            "NEEDS-ACTION", t(2026, 2, 1, 9, 0, 0),
        );
        entry.cyclic = false;
        entry.resolved_depends = vec!["dep-uid".to_string()];
        // Make content differ so LWW doesn't short-circuit to Identical.
        if let Some(ref mut fv) = entry.fetched_vtodo {
            fv.vtodo.summary = Some("Old summary".to_string());
            fv.vtodo.description = Some("Old summary".to_string());
        }
        let mut ir = vec![entry];

        let tw = make_tw_adapter(MockTaskRunner::new());
        let caldav = MockCalDavClient::new();
        let result = apply_writeback(&mut ir, &tw, &caldav, false, false, t(2026, 2, 2, 0, 0, 0));

        // Non-cyclic entry should be written.
        assert_eq!(result.written_caldav, 1);

        // CalDAV mock received a Put call.
        let calls = caldav.calls.lock().unwrap();
        assert!(calls.iter().any(|c| matches!(c, CalDavCall::Put { .. })),
            "expected a Put call for the non-cyclic entry");

        // The VTODO stored on the entry should contain RELATED-TO with dep-uid.
        let vtodo = &ir[0].fetched_vtodo.as_ref().expect("should have fetched_vtodo after put").vtodo;
        assert!(!vtodo.depends.is_empty(), "non-cyclic entry must preserve RELATED-TO");
        assert_eq!(vtodo.depends[0].1, "dep-uid",
            "non-cyclic entry must have dep-uid in RELATED-TO");
    }

    #[test]
    fn tw_completed_marks_caldav_completed() {
        // TW status="completed", CalDAV status="NEEDS-ACTION" →
        // decide_op emits TwCompletedMarkCompleted → PUT to CalDAV with COMPLETED status.
        let uuid = Uuid::new_v4();
        let caldav_uid = Uuid::new_v4().to_string();
        let tw_modified = t(2026, 2, 1, 10, 0, 0);
        let caldav_lm = t(2026, 2, 1, 9, 0, 0);

        let mut ir = vec![make_paired_entry(
            uuid, &caldav_uid, "completed", tw_modified,
            "NEEDS-ACTION", caldav_lm,
        )];

        let tw = make_tw_adapter(MockTaskRunner::new());
        let caldav = MockCalDavClient::new();
        let result = apply_writeback(&mut ir, &tw, &caldav, false, false, t(2026, 2, 2, 0, 0, 0));

        assert_eq!(result.written_caldav, 1, "should PUT COMPLETED to CalDAV");
        assert_eq!(result.written_tw, 0);
        assert_eq!(result.errors.len(), 0);
    }

    #[test]
    fn caldav_completed_updates_tw() {
        // CalDAV status="COMPLETED", TW status="pending" →
        // decide_op emits CalDavCompletedUpdateTw → tw.update() called.
        let uuid = Uuid::new_v4();
        let caldav_uid = Uuid::new_v4().to_string();
        let tw_modified = t(2026, 2, 1, 8, 0, 0);
        let caldav_lm = t(2026, 2, 1, 10, 0, 0);

        let mut ir = vec![make_paired_entry(
            uuid, &caldav_uid, "pending", tw_modified,
            "COMPLETED", caldav_lm,
        )];

        let mock_tw = MockTaskRunner::new();
        mock_tw.push_run_response(Ok(String::new())); // uda type
        mock_tw.push_run_response(Ok(String::new())); // uda label
        mock_tw.push_run_response(Ok(String::new())); // tw.update()
        let tw = TwAdapter::new(mock_tw).expect("TwAdapter");

        let caldav = MockCalDavClient::new();
        let result = apply_writeback(&mut ir, &tw, &caldav, false, false, t(2026, 2, 2, 0, 0, 0));

        assert_eq!(result.written_tw, 1, "should update TW task to completed");
        assert_eq!(result.written_caldav, 0);
        assert_eq!(result.errors.len(), 0);
    }

    // ── AUDIT-01: TW tags → VTODO categories mapping ─────────────────────

    #[test]
    fn test_build_vtodo_uses_tw_tags() {
        let uuid = Uuid::new_v4();
        let caldav_uid = Uuid::new_v4().to_string();
        let mut entry = make_tw_only_entry(uuid, &caldav_uid, "pending");
        // Set TW tags
        if let Some(ref mut tw) = entry.tw_task {
            tw.tags = Some(vec!["work".to_string(), "urgent".to_string()]);
        }
        let tw_snapshot = entry.tw_task.clone().unwrap();
        let now = t(2026, 2, 2, 0, 0, 0);
        let vtodo = build_vtodo_from_tw(&entry, &tw_snapshot, now);

        assert_eq!(
            vtodo.categories,
            vec!["work", "urgent"],
            "VTODO categories must come from TW tags, not stale CalDAV data"
        );
    }

    #[test]
    fn test_build_vtodo_empty_tags() {
        let uuid = Uuid::new_v4();
        let caldav_uid = Uuid::new_v4().to_string();
        let mut entry = make_tw_only_entry(uuid, &caldav_uid, "pending");
        // tags = None → categories should be empty
        if let Some(ref mut tw) = entry.tw_task {
            tw.tags = None;
        }
        let tw_snapshot = entry.tw_task.clone().unwrap();
        let now = t(2026, 2, 2, 0, 0, 0);
        let vtodo = build_vtodo_from_tw(&entry, &tw_snapshot, now);

        assert!(
            vtodo.categories.is_empty(),
            "VTODO categories must be empty when TW has no tags"
        );
    }

    #[test]
    fn test_build_vtodo_tags_with_comma() {
        let uuid = Uuid::new_v4();
        let caldav_uid = Uuid::new_v4().to_string();
        let mut entry = make_tw_only_entry(uuid, &caldav_uid, "pending");
        if let Some(ref mut tw) = entry.tw_task {
            tw.tags = Some(vec!["Smith, John".to_string()]);
        }
        let tw_snapshot = entry.tw_task.clone().unwrap();
        let now = t(2026, 2, 2, 0, 0, 0);
        let vtodo = build_vtodo_from_tw(&entry, &tw_snapshot, now);

        assert_eq!(
            vtodo.categories,
            vec!["Smith, John"],
            "Tag with comma must be preserved as single category"
        );
    }
}
