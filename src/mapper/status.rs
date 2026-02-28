//! Status mapper: TaskWarrior status → CalDAV/sync status descriptor.

use chrono::{DateTime, Utc};

use crate::types::{TWTask, Warning};

// ---------------------------------------------------------------------------
// TwToCalDavStatus
// ---------------------------------------------------------------------------

/// Describes how a TW task's status maps to the CalDAV/sync layer.
///
/// `TwStateDeleted` is a state descriptor, NOT a hard-delete instruction.
/// The sync orchestrator dispatches it as:
/// - `CalDavCancelled` when both TW and CalDAV sides exist
/// - `AlreadyDeleted` when the entry is TW-only
#[derive(Debug, Clone)]
pub enum TwToCalDavStatus {
    /// TW status "pending" — maps to VTODO STATUS:NEEDS-ACTION.
    NeedsAction,
    /// TW status "waiting" — NEEDS-ACTION with a wait timestamp.
    NeedsActionWithWait(DateTime<Utc>),
    /// TW status "completed" — maps to VTODO STATUS:COMPLETED.
    Completed(DateTime<Utc>),
    /// TW status "deleted" — NOT a direct CalDAV delete; caller dispatches.
    TwStateDeleted,
    /// TW status "recurring" — skip with a warning; recurring tasks are not synced.
    Skip(Warning),
}

// ---------------------------------------------------------------------------
// tw_to_caldav_status
// ---------------------------------------------------------------------------

/// Map a TaskWarrior task's status to a [`TwToCalDavStatus`] variant.
pub fn tw_to_caldav_status(task: &TWTask) -> TwToCalDavStatus {
    match task.status.as_str() {
        "pending" => TwToCalDavStatus::NeedsAction,

        "waiting" => {
            let wait_dt = task.wait.unwrap_or(task.entry);
            TwToCalDavStatus::NeedsActionWithWait(wait_dt)
        }

        "recurring" => TwToCalDavStatus::Skip(Warning {
            tw_uuid: Some(task.uuid),
            message: format!(
                "recurring task {} skipped (recur: {:?})",
                task.uuid,
                task.recur.as_deref().unwrap_or("unknown")
            ),
        }),

        "completed" => {
            let end_dt = task.end.unwrap_or(task.entry);
            TwToCalDavStatus::Completed(end_dt)
        }

        "deleted" => TwToCalDavStatus::TwStateDeleted,

        // Unknown status — treat as NEEDS-ACTION (safe fallback).
        _ => TwToCalDavStatus::NeedsAction,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn make_task(status: &str) -> TWTask {
        TWTask {
            uuid: Uuid::new_v4(),
            status: status.to_string(),
            description: "test".to_string(),
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
        }
    }

    #[test]
    fn pending_maps_to_needs_action() {
        let task = make_task("pending");
        assert!(matches!(
            tw_to_caldav_status(&task),
            TwToCalDavStatus::NeedsAction
        ));
    }

    #[test]
    fn waiting_maps_to_needs_action_with_wait() {
        let mut task = make_task("waiting");
        let wait_time = Utc::now();
        task.wait = Some(wait_time);

        match tw_to_caldav_status(&task) {
            TwToCalDavStatus::NeedsActionWithWait(dt) => assert_eq!(dt, wait_time),
            other => panic!("expected NeedsActionWithWait, got {:?}", other),
        }
    }

    #[test]
    fn waiting_without_wait_field_falls_back_to_entry() {
        let task = make_task("waiting"); // wait = None
        match tw_to_caldav_status(&task) {
            TwToCalDavStatus::NeedsActionWithWait(dt) => assert_eq!(dt, task.entry),
            other => panic!("expected NeedsActionWithWait, got {:?}", other),
        }
    }

    #[test]
    fn recurring_maps_to_skip_with_warning() {
        let mut task = make_task("recurring");
        task.recur = Some("weekly".to_string());

        match tw_to_caldav_status(&task) {
            TwToCalDavStatus::Skip(w) => {
                assert_eq!(w.tw_uuid, Some(task.uuid));
                assert!(w.message.contains("recurring"));
            }
            other => panic!("expected Skip, got {:?}", other),
        }
    }

    #[test]
    fn completed_maps_to_completed_with_end_time() {
        let mut task = make_task("completed");
        let end_time = Utc::now();
        task.end = Some(end_time);

        match tw_to_caldav_status(&task) {
            TwToCalDavStatus::Completed(dt) => assert_eq!(dt, end_time),
            other => panic!("expected Completed, got {:?}", other),
        }
    }

    #[test]
    fn completed_without_end_falls_back_to_entry() {
        let task = make_task("completed"); // end = None
        match tw_to_caldav_status(&task) {
            TwToCalDavStatus::Completed(dt) => assert_eq!(dt, task.entry),
            other => panic!("expected Completed, got {:?}", other),
        }
    }

    #[test]
    fn deleted_maps_to_tw_state_deleted() {
        let task = make_task("deleted");
        assert!(matches!(
            tw_to_caldav_status(&task),
            TwToCalDavStatus::TwStateDeleted
        ));
    }

    #[test]
    fn unknown_status_falls_back_to_needs_action() {
        let task = make_task("bogus_status");
        assert!(matches!(
            tw_to_caldav_status(&task),
            TwToCalDavStatus::NeedsAction
        ));
    }
}
