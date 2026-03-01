//! Bidirectional field mapper between TaskWarrior and CalDAV/iCalendar fields.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::types::{IcalProp, RelType, TWTask, VTODO};

// ---------------------------------------------------------------------------
// Output structs
// ---------------------------------------------------------------------------

/// Fields produced when mapping a [`TWTask`] → CalDAV direction.
#[derive(Debug, Clone)]
pub struct TwCalDavFields {
    /// TW `description` → VTODO `DESCRIPTION`.
    pub description: Option<String>,
    /// TW `due` → VTODO `DUE`.
    pub due: Option<DateTime<Utc>>,
    /// TW `scheduled` → VTODO `DTSTART`.
    pub dtstart: Option<DateTime<Utc>>,
    /// TW `wait` → `X-TASKWARRIOR-WAIT` extra property.
    /// `None` when the wait datetime has already passed (expired-wait collapse,
    /// Phase 0 finding #6: TW may still export `status:waiting` with an expired
    /// wait datetime; we drop the property so CalDAV reflects the effective
    /// pending state).
    pub wait: Option<IcalProp>,
    /// TW `depends` (Vec<Uuid>) → `RELATED-TO;RELTYPE=DEPENDS-ON`.
    pub depends: Vec<(RelType, String)>,
}

/// Fields produced when mapping a CalDAV [`VTODO`] → TW direction.
#[derive(Debug, Clone)]
pub struct CalDavTwFields {
    /// VTODO `DESCRIPTION` → TW `description`. Empty string when absent.
    pub description: String,
    /// VTODO `DUE` → TW `due`.
    pub due: Option<DateTime<Utc>>,
    /// VTODO `DTSTART` → TW `scheduled`.
    pub scheduled: Option<DateTime<Utc>>,
    /// `X-TASKWARRIOR-WAIT` extra property → TW `wait`.
    pub wait: Option<DateTime<Utc>>,
    /// `RELATED-TO;RELTYPE=DEPENDS-ON` → TW `depends` (parsed as Uuid).
    pub depends: Vec<Uuid>,
}

// ---------------------------------------------------------------------------
// tw_to_caldav_fields
// ---------------------------------------------------------------------------

/// Map a [`TWTask`]'s fields to their CalDAV equivalents.
///
/// The caller is responsible for merging these fields into a [`VTODO`];
/// status mapping is handled separately in [`crate::mapper::status`].
pub fn tw_to_caldav_fields(task: &TWTask, now: DateTime<Utc>) -> TwCalDavFields {
    let description = Some(task.description.clone());

    let due = task.due;
    let dtstart = task.scheduled;

    // Phase 0 finding #6: expired-wait collapse.
    // Only include the X-TASKWARRIOR-WAIT property when the wait datetime is
    // still in the future. If it has passed, TW will have (or will soon)
    // transition the task to "pending"; omitting the property avoids
    // re-setting a stale wait on the CalDAV side.
    let wait = task.wait.and_then(|w| {
        if w > now {
            Some(IcalProp {
                name: "X-TASKWARRIOR-WAIT".to_string(),
                params: vec![],
                value: w.format("%Y%m%dT%H%M%SZ").to_string(),
            })
        } else {
            None
        }
    });

    // Map each dependency UUID to a RELATED-TO;RELTYPE=DEPENDS-ON entry.
    // TW UUIDs are used directly as CalDAV UIDs (they are identical per the
    // caldavuid UDA design).
    let depends = task
        .depends
        .iter()
        .map(|uuid| (RelType::DependsOn, uuid.to_string()))
        .collect();

    TwCalDavFields {
        description,
        due,
        dtstart,
        wait,
        depends,
    }
}

// ---------------------------------------------------------------------------
// caldav_to_tw_fields
// ---------------------------------------------------------------------------

/// Map a [`VTODO`]'s fields to their TaskWarrior equivalents.
///
/// The caller merges these fields into a [`TWTask`]; status mapping is
/// handled separately in [`crate::mapper::status`].
pub fn caldav_to_tw_fields(vtodo: &VTODO) -> CalDavTwFields {
    let description = vtodo.description.clone().unwrap_or_default();

    let due = vtodo.due;
    let scheduled = vtodo.dtstart;

    // Extract X-TASKWARRIOR-WAIT from extra_props and parse as a DateTime.
    let wait = vtodo
        .extra_props
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case("X-TASKWARRIOR-WAIT"))
        .and_then(|p| parse_ical_datetime_utc(&p.value));

    // Collect RELATED-TO;RELTYPE=DEPENDS-ON entries and parse UIDs as Uuid.
    let depends = vtodo
        .depends
        .iter()
        .filter_map(|(rel, uid_str)| {
            if matches!(rel, RelType::DependsOn) {
                uid_str.parse::<Uuid>().ok()
            } else {
                None
            }
        })
        .collect();

    CalDavTwFields {
        description,
        due,
        scheduled,
        wait,
        depends,
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Parse an iCalendar datetime string (`YYYYMMDDTHHMMSSZ`) into UTC.
fn parse_ical_datetime_utc(s: &str) -> Option<DateTime<Utc>> {
    use chrono::NaiveDateTime;
    let s = s.trim().trim_end_matches('Z');
    NaiveDateTime::parse_from_str(s, "%Y%m%dT%H%M%S")
        .ok()
        .map(|ndt| ndt.and_utc())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use uuid::Uuid;

    fn make_tw_task() -> TWTask {
        TWTask {
            uuid: Uuid::new_v4(),
            status: "pending".to_string(),
            description: "Buy milk".to_string(),
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

    fn make_vtodo() -> VTODO {
        VTODO {
            uid: Uuid::new_v4().to_string(),
            summary: Some("Buy milk".to_string()),
            description: None,
            status: None,
            last_modified: None,
            dtstamp: None,
            dtstart: None,
            due: None,
            completed: None,
            categories: vec![],
            rrule: None,
            depends: vec![],
            extra_props: vec![],
        }
    }

    // ── TW → CalDAV ──────────────────────────────────────────────────────────

    #[test]
    fn tw_description_mapped() {
        let task = make_tw_task();
        let fields = tw_to_caldav_fields(&task, Utc::now());
        assert_eq!(fields.description.as_deref(), Some("Buy milk"));
    }

    #[test]
    fn tw_due_mapped() {
        let mut task = make_tw_task();
        let due = Utc::now();
        task.due = Some(due);
        let fields = tw_to_caldav_fields(&task, Utc::now());
        assert_eq!(fields.due, Some(due));
    }

    #[test]
    fn tw_scheduled_mapped_to_dtstart() {
        let mut task = make_tw_task();
        let sched = Utc::now();
        task.scheduled = Some(sched);
        let fields = tw_to_caldav_fields(&task, Utc::now());
        assert_eq!(fields.dtstart, Some(sched));
    }

    #[test]
    fn tw_future_wait_included_as_ical_prop() {
        let mut task = make_tw_task();
        task.wait = Some(Utc::now() + Duration::hours(24));
        let fields = tw_to_caldav_fields(&task, Utc::now());
        let prop = fields.wait.expect("wait prop should be present");
        assert_eq!(prop.name, "X-TASKWARRIOR-WAIT");
        assert!(!prop.value.is_empty());
    }

    #[test]
    fn tw_expired_wait_collapsed_to_none() {
        let mut task = make_tw_task();
        // wait in the past
        task.wait = Some(Utc::now() - Duration::hours(1));
        let fields = tw_to_caldav_fields(&task, Utc::now());
        assert!(
            fields.wait.is_none(),
            "expired wait should be dropped (Phase 0 finding #6)"
        );
    }

    #[test]
    fn tw_no_wait_produces_no_prop() {
        let task = make_tw_task();
        let fields = tw_to_caldav_fields(&task, Utc::now());
        assert!(fields.wait.is_none());
    }

    #[test]
    fn tw_depends_mapped_to_related_to() {
        let mut task = make_tw_task();
        let dep1 = Uuid::new_v4();
        let dep2 = Uuid::new_v4();
        task.depends = vec![dep1, dep2];

        let fields = tw_to_caldav_fields(&task, Utc::now());
        assert_eq!(fields.depends.len(), 2);
        for (rel, uid_str) in &fields.depends {
            assert!(matches!(rel, RelType::DependsOn));
            assert!(uid_str.parse::<Uuid>().is_ok());
        }
        let uids: Vec<Uuid> = fields
            .depends
            .iter()
            .map(|(_, s)| s.parse().unwrap())
            .collect();
        assert!(uids.contains(&dep1));
        assert!(uids.contains(&dep2));
    }

    #[test]
    fn tw_none_fields_produce_none() {
        let task = make_tw_task(); // due/scheduled/wait all None
        let fields = tw_to_caldav_fields(&task, Utc::now());
        assert!(fields.due.is_none());
        assert!(fields.dtstart.is_none());
        assert!(fields.wait.is_none());
        assert!(fields.depends.is_empty());
    }

    // ── CalDAV → TW ──────────────────────────────────────────────────────────

    #[test]
    fn caldav_description_mapped() {
        let mut vtodo = make_vtodo();
        vtodo.description = Some("Buy milk".to_string());
        let fields = caldav_to_tw_fields(&vtodo);
        assert_eq!(fields.description, "Buy milk");
    }

    #[test]
    fn caldav_absent_description_becomes_empty_string() {
        let vtodo = make_vtodo(); // description = None
        let fields = caldav_to_tw_fields(&vtodo);
        assert_eq!(fields.description, "");
    }

    #[test]
    fn caldav_due_mapped() {
        let mut vtodo = make_vtodo();
        let due = Utc::now();
        vtodo.due = Some(due);
        let fields = caldav_to_tw_fields(&vtodo);
        assert_eq!(fields.due, Some(due));
    }

    #[test]
    fn caldav_dtstart_mapped_to_scheduled() {
        let mut vtodo = make_vtodo();
        let start = Utc::now();
        vtodo.dtstart = Some(start);
        let fields = caldav_to_tw_fields(&vtodo);
        assert_eq!(fields.scheduled, Some(start));
    }

    #[test]
    fn caldav_wait_prop_parsed() {
        let mut vtodo = make_vtodo();
        vtodo.extra_props.push(IcalProp {
            name: "X-TASKWARRIOR-WAIT".to_string(),
            params: vec![],
            value: "20271231T120000Z".to_string(),
        });
        let fields = caldav_to_tw_fields(&vtodo);
        let wait = fields.wait.expect("wait should be parsed");
        assert_eq!(wait.format("%Y").to_string(), "2027");
    }

    #[test]
    fn caldav_depends_round_trip() {
        let dep1 = Uuid::new_v4();
        let dep2 = Uuid::new_v4();

        let mut vtodo = make_vtodo();
        vtodo.depends = vec![
            (RelType::DependsOn, dep1.to_string()),
            (RelType::DependsOn, dep2.to_string()),
            (RelType::Other("CHILD".to_string()), "some-uid".to_string()), // should be ignored
        ];

        let fields = caldav_to_tw_fields(&vtodo);
        assert_eq!(fields.depends.len(), 2, "only DEPENDS-ON entries should be included");
        assert!(fields.depends.contains(&dep1));
        assert!(fields.depends.contains(&dep2));
    }

    #[test]
    fn caldav_none_fields_produce_none() {
        let vtodo = make_vtodo(); // all optional fields absent
        let fields = caldav_to_tw_fields(&vtodo);
        assert!(fields.due.is_none());
        assert!(fields.scheduled.is_none());
        assert!(fields.wait.is_none());
        assert!(fields.depends.is_empty());
    }

    // ── Round-trip ────────────────────────────────────────────────────────────

    #[test]
    fn depends_round_trip_tw_to_caldav_to_tw() {
        let dep = Uuid::new_v4();
        let mut task = make_tw_task();
        task.depends = vec![dep];

        // TW → CalDAV
        let caldav_fields = tw_to_caldav_fields(&task, Utc::now());
        let mut vtodo = make_vtodo();
        vtodo.depends = caldav_fields.depends;

        // CalDAV → TW
        let tw_fields = caldav_to_tw_fields(&vtodo);
        assert_eq!(tw_fields.depends, vec![dep]);
    }
}
