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
    /// TW `description` → VTODO `SUMMARY`.
    pub summary: Option<String>,
    /// TW `annotations` → VTODO `DESCRIPTION` (newline-joined).
    pub annotations: Option<String>,
    /// TW `priority` → VTODO `PRIORITY` (iCal integer: 1=high, 5=medium, 9=low).
    pub priority: Option<u8>,
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
    /// VTODO `SUMMARY` → TW `description`. Empty string when absent.
    pub description: String,
    /// VTODO `DESCRIPTION` → TW `annotations` (raw text, split by caller).
    pub annotations_text: Option<String>,
    /// VTODO `PRIORITY` → TW `priority` (TW format: "H", "M", "L").
    pub priority: Option<String>,
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
    // "(no title)" is the reverse sentinel used when no SUMMARY was present on
    // the CalDAV side — map back to None so we don't push a literal sentinel.
    let summary = if task.description == "(no title)" {
        None
    } else {
        Some(task.description.clone())
    };

    // Map the first annotation's text to VTODO DESCRIPTION.
    let annotations = task.annotations.first().map(|a| a.description.clone());

    // Map TW priority letter to iCal integer (RFC 5545: 1=high, 5=medium, 9=low).
    let priority = task.priority.as_deref().and_then(|p| match p {
        "H" => Some(1u8),
        "M" => Some(5u8),
        "L" => Some(9u8),
        _ => None,
    });

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
        summary,
        annotations,
        priority,
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
    // VTODO SUMMARY → TW description; fall back to sentinel when absent.
    let description = vtodo
        .summary
        .clone()
        .unwrap_or_else(|| "(no title)".to_string());

    // VTODO DESCRIPTION → TW annotations_text; treat empty/whitespace as absent.
    let annotations_text = vtodo.description.as_deref().and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });

    // VTODO PRIORITY → TW priority letter.
    let priority = vtodo.priority.and_then(|p| match p {
        1..=4 => Some("H".to_string()),
        5 => Some("M".to_string()),
        6..=9 => Some("L".to_string()),
        _ => None,
    });

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
        annotations_text,
        priority,
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
    use crate::types::TwAnnotation;
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
            annotations: vec![],
        }
    }

    fn make_vtodo() -> VTODO {
        VTODO {
            uid: Uuid::new_v4().to_string(),
            summary: Some("Buy milk".to_string()),
            ..Default::default()
        }
    }

    // ── TW → CalDAV ──────────────────────────────────────────────────────────

    #[test]
    fn tw_description_mapped() {
        let task = make_tw_task();
        let fields = tw_to_caldav_fields(&task, Utc::now());
        assert_eq!(fields.summary.as_deref(), Some("Buy milk"));
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
    fn caldav_absent_summary_becomes_no_title_sentinel() {
        let mut vtodo = make_vtodo();
        vtodo.summary = None;
        let fields = caldav_to_tw_fields(&vtodo);
        assert_eq!(fields.description, "(no title)");
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
        assert_eq!(
            fields.depends.len(),
            2,
            "only DEPENDS-ON entries should be included"
        );
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

    // ── Summary / annotations / priority — CalDAV → TW ───────────────────────

    #[test]
    fn caldav_summary_mapped_to_tw_description() {
        let mut vtodo = make_vtodo();
        vtodo.summary = Some("Task X".to_string());
        vtodo.description = None;
        let fields = caldav_to_tw_fields(&vtodo);
        assert_eq!(fields.description, "Task X");
        assert!(fields.annotations_text.is_none());
    }

    #[test]
    fn caldav_both_summary_and_description_present() {
        let mut vtodo = make_vtodo();
        vtodo.summary = Some("Task X".to_string());
        vtodo.description = Some("a note".to_string());
        let fields = caldav_to_tw_fields(&vtodo);
        assert_eq!(fields.description, "Task X");
        assert_eq!(fields.annotations_text.as_deref(), Some("a note"));
    }

    #[test]
    fn caldav_description_mapped_to_annotations_text() {
        let mut vtodo = make_vtodo();
        vtodo.description = Some("check expiry".to_string());
        let fields = caldav_to_tw_fields(&vtodo);
        assert_eq!(fields.annotations_text.as_deref(), Some("check expiry"));
    }

    #[test]
    fn caldav_no_summary_gives_no_title_sentinel() {
        let mut vtodo = make_vtodo();
        vtodo.summary = None;
        let fields = caldav_to_tw_fields(&vtodo);
        assert_eq!(fields.description, "(no title)");
    }

    #[test]
    fn priority_caldav_to_tw_1_gives_h() {
        let mut vtodo = make_vtodo();
        vtodo.priority = Some(1);
        let fields = caldav_to_tw_fields(&vtodo);
        assert_eq!(fields.priority.as_deref(), Some("H"));
    }

    #[test]
    fn priority_caldav_2_3_4_give_h() {
        for p in [2u8, 3, 4] {
            let mut vtodo = make_vtodo();
            vtodo.priority = Some(p);
            let fields = caldav_to_tw_fields(&vtodo);
            assert_eq!(
                fields.priority.as_deref(),
                Some("H"),
                "priority {p} should map to H"
            );
        }
    }

    #[test]
    fn priority_caldav_5_gives_m_9_gives_l_0_gives_none() {
        let cases: &[(u8, Option<&str>)] = &[(5, Some("M")), (9, Some("L")), (0, None)];
        for &(p, expected) in cases {
            let mut vtodo = make_vtodo();
            vtodo.priority = Some(p);
            let fields = caldav_to_tw_fields(&vtodo);
            assert_eq!(fields.priority.as_deref(), expected, "priority {p}");
        }
        // None input
        let mut vtodo = make_vtodo();
        vtodo.priority = None;
        let fields = caldav_to_tw_fields(&vtodo);
        assert!(fields.priority.is_none());
    }

    // ── Summary / annotations / priority — TW → CalDAV ───────────────────────

    #[test]
    fn tw_description_becomes_summary() {
        let task = make_tw_task(); // description = "Buy milk"
        let fields = tw_to_caldav_fields(&task, Utc::now());
        assert_eq!(fields.summary.as_deref(), Some("Buy milk"));
    }

    #[test]
    fn tw_no_title_becomes_absent_summary() {
        let mut task = make_tw_task();
        task.description = "(no title)".to_string();
        let fields = tw_to_caldav_fields(&task, Utc::now());
        assert!(fields.summary.is_none(), "sentinel should collapse to None");
    }

    #[test]
    fn tw_annotations_become_description() {
        let mut task = make_tw_task();
        task.annotations = vec![TwAnnotation {
            entry: Utc::now(),
            description: "check expiry date".to_string(),
        }];
        let fields = tw_to_caldav_fields(&task, Utc::now());
        assert_eq!(fields.annotations.as_deref(), Some("check expiry date"));
    }

    #[test]
    fn priority_tw_to_caldav_h() {
        let mut task = make_tw_task();
        task.priority = Some("H".to_string());
        let fields = tw_to_caldav_fields(&task, Utc::now());
        assert_eq!(fields.priority, Some(1u8));

        task.priority = Some("M".to_string());
        let fields = tw_to_caldav_fields(&task, Utc::now());
        assert_eq!(fields.priority, Some(5u8));

        task.priority = Some("L".to_string());
        let fields = tw_to_caldav_fields(&task, Utc::now());
        assert_eq!(fields.priority, Some(9u8));

        task.priority = None;
        let fields = tw_to_caldav_fields(&task, Utc::now());
        assert!(fields.priority.is_none());
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
