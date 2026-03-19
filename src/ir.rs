use std::collections::HashMap;

use uuid::Uuid;

use crate::config::Config;
use crate::types::{FetchedVTODO, IREntry, TWTask, Warning};

/// Resolves a project name to a calendar URL.
///
/// Lookup order:
///   1. First `calendars` entry whose `project` matches `project`.
///   2. First `calendars` entry whose `project` is `"default"` (fallback).
///   3. `None` — caller should emit an `UnmappedProject` warning.
fn resolve_calendar_url(project: Option<&str>, config: &Config) -> Option<String> {
    if let Some(proj) = project {
        if let Some(entry) = config.calendars.iter().find(|c| c.project == proj) {
            return Some(entry.url.clone());
        }
    }
    config
        .calendars
        .iter()
        .find(|c| c.project == "default")
        .map(|c| c.url.clone())
}

/// The sentinel project name used when a calendar has no specific project assignment.
const DEFAULT_PROJECT: &str = "default";

/// Resolves a calendar URL back to the project name configured for that calendar.
///
/// Returns `None` when:
/// - No calendar entry matches the URL (unknown calendar).
/// - The matching calendar's project is `DEFAULT_PROJECT` (callers treat this as "no project").
///
/// URL comparison is done after stripping trailing slashes from both sides.
fn resolve_project_from_url(url: &str, config: &Config) -> Option<String> {
    let normalized = url.trim_end_matches('/');
    config
        .calendars
        .iter()
        .find(|c| c.url.trim_end_matches('/') == normalized)
        .and_then(|c| {
            if c.project == DEFAULT_PROJECT {
                None
            } else {
                Some(c.project.clone())
            }
        })
}

/// Builds the Intermediate Representation (IR) from TaskWarrior tasks and CalDAV VTODOs.
///
/// Classification rules:
///
/// **TW tasks (three-way):**
/// - `caldavuid = None` → TW-only *new*: assigns a fresh UUID4 as `caldav_uid`.
/// - `caldavuid = Some(uid)` and a matching VTODO exists → *paired*.
/// - `caldavuid = Some(uid)` and no matching VTODO → *orphaned* (uid preserved).
///
/// **CalDAV-only VTODOs (after TW pass):**
/// - `RRULE` set → skipped; emits `RecurringCalDavSkipped` warning.
/// - status `NEEDS-ACTION` or `IN-PROCESS` → fresh UUID4 pre-assigned as `tw_uuid`.
/// - status `COMPLETED` or `CANCELLED` (or other terminal) → `tw_uuid = None`.
///
/// **calendar_url** is resolved from config at construction time for TW-only/orphaned entries.
/// Emits `UnmappedProject` warning when no calendar entry matches the task's project.
///
/// All `dirty_tw`, `dirty_caldav`, and `cyclic` fields are `false` after construction.
pub fn build_ir(
    tw_tasks: &[TWTask],
    vtodos_by_calendar: &HashMap<String, Vec<FetchedVTODO>>,
    config: &Config,
) -> (Vec<IREntry>, Vec<Warning>) {
    let mut entries: Vec<IREntry> = Vec::new();
    let mut warnings: Vec<Warning> = Vec::new();

    // Build lookup: CalDAV UID -> (calendar_url, FetchedVTODO).
    // RRULE VTODOs are skipped here with a warning.
    let mut caldav_map: HashMap<String, (String, FetchedVTODO)> = HashMap::new();
    for (calendar_url, fetched_list) in vtodos_by_calendar {
        for fetched in fetched_list {
            if fetched.vtodo.rrule.is_some() {
                warnings.push(Warning {
                    tw_uuid: None,
                    message: format!(
                        "RecurringCalDavSkipped: VTODO '{}' has RRULE and will not be synced",
                        fetched.vtodo.uid
                    ),
                });
                continue;
            }
            caldav_map.insert(
                fetched.vtodo.uid.clone(),
                (calendar_url.clone(), fetched.clone()),
            );
        }
    }

    // --- TW pass: three-way classification ---
    for task in tw_tasks {
        match &task.caldavuid {
            None => {
                // TW-only NEW: assign a fresh UUID4 as caldav_uid.
                let new_uid = Uuid::new_v4().to_string();
                let calendar_url = resolve_calendar_url(task.project.as_deref(), config);
                if calendar_url.is_none() {
                    warnings.push(Warning {
                        tw_uuid: Some(task.uuid),
                        message: format!(
                            "UnmappedProject: task {} project={:?} has no matching calendar",
                            task.uuid,
                            task.project.as_deref().unwrap_or("<none>")
                        ),
                    });
                }
                entries.push(IREntry {
                    tw_uuid: Some(task.uuid),
                    caldav_uid: Some(new_uid),
                    tw_task: Some(task.clone()),
                    fetched_vtodo: None,
                    resolved_depends: vec![],
                    cyclic: false,
                    calendar_url,
                    dirty_tw: false,
                    dirty_caldav: false,
                    project: None,
                });
            }
            Some(uid) => {
                if let Some((calendar_url, fetched)) = caldav_map.remove(uid) {
                    // PAIRED: TW task matches a CalDAV VTODO.
                    entries.push(IREntry {
                        tw_uuid: Some(task.uuid),
                        caldav_uid: Some(uid.clone()),
                        tw_task: Some(task.clone()),
                        fetched_vtodo: Some(fetched),
                        resolved_depends: vec![],
                        cyclic: false,
                        calendar_url: Some(calendar_url),
                        dirty_tw: false,
                        dirty_caldav: false,
                        project: None,
                    });
                } else {
                    // ORPHANED: caldavuid set but VTODO not found.
                    let calendar_url = resolve_calendar_url(task.project.as_deref(), config);
                    if calendar_url.is_none() {
                        warnings.push(Warning {
                            tw_uuid: Some(task.uuid),
                            message: format!(
                                "UnmappedProject: task {} project={:?} has no matching calendar",
                                task.uuid,
                                task.project.as_deref().unwrap_or("<none>")
                            ),
                        });
                    }
                    entries.push(IREntry {
                        tw_uuid: Some(task.uuid),
                        caldav_uid: Some(uid.clone()),
                        tw_task: Some(task.clone()),
                        fetched_vtodo: None,
                        resolved_depends: vec![],
                        cyclic: false,
                        calendar_url,
                        dirty_tw: false,
                        dirty_caldav: false,
                        project: None,
                    });
                }
            }
        }
    }

    // --- CalDAV-only pass: remaining unmatched VTODOs ---
    for (_uid, (calendar_url, fetched)) in caldav_map {
        let status = fetched.vtodo.status.as_deref().unwrap_or("NEEDS-ACTION");
        // Assign a fresh UUID4 for active entries; terminal entries stay None.
        let tw_uuid = match status {
            "COMPLETED" | "CANCELLED" => None,
            _ => Some(Uuid::new_v4()), // NEEDS-ACTION, IN-PROCESS, or unrecognised
        };
        let project = resolve_project_from_url(&calendar_url, config);
        entries.push(IREntry {
            tw_uuid,
            caldav_uid: Some(fetched.vtodo.uid.clone()),
            tw_task: None,
            fetched_vtodo: Some(fetched),
            resolved_depends: vec![],
            cyclic: false,
            calendar_url: Some(calendar_url),
            dirty_tw: false,
            dirty_caldav: false,
            project,
        });
    }

    (entries, warnings)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CalendarEntry;
    use crate::types::{FetchedVTODO, TWTask, VTODO};
    use chrono::Utc;

    fn make_config(calendars: Vec<(&str, &str)>) -> Config {
        Config {
            server_url: "https://dav.example.com".to_string(),
            username: "alice".to_string(),
            password: "secret".to_string(),
            completed_cutoff_days: 90,
            allow_insecure_tls: false,
            caldav_timeout_seconds: 30,
            calendars: calendars
                .into_iter()
                .map(|(proj, url)| CalendarEntry {
                    project: proj.to_string(),
                    url: url.to_string(),
                })
                .collect(),
        }
    }

    fn make_tw_task(
        uuid: Uuid,
        caldavuid: Option<&str>,
        project: Option<&str>,
    ) -> TWTask {
        TWTask {
            uuid,
            status: "pending".to_string(),
            description: format!("task-{}", uuid),
            entry: Utc::now(),
            modified: None,
            due: None,
            scheduled: None,
            wait: None,
            until: None,
            end: None,
            caldavuid: caldavuid.map(str::to_owned),
            priority: None,
            project: project.map(str::to_owned),
            tags: None,
            recur: None,
            urgency: None,
            id: None,
            depends: vec![],
            annotations: vec![],
        }
    }

    fn make_vtodo(uid: &str, status: Option<&str>, rrule: Option<&str>) -> FetchedVTODO {
        FetchedVTODO {
            href: format!("/{}.ics", uid),
            etag: None,
            vtodo: VTODO {
                uid: uid.to_string(),
                status: status.map(str::to_owned),
                rrule: rrule.map(str::to_owned),
                ..Default::default()
            },
        }
    }

    #[test]
    fn tw_only_new_gets_fresh_caldav_uid() {
        let config = make_config(vec![("default", "https://dav.example.com/cal/")]);
        let uuid = Uuid::new_v4();
        let tasks = vec![make_tw_task(uuid, None, None)];

        let (entries, warnings) = build_ir(&tasks, &HashMap::new(), &config);

        assert!(warnings.is_empty());
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.tw_uuid, Some(uuid));
        assert!(e.caldav_uid.is_some(), "fresh caldav_uid should be assigned");
        assert!(e.tw_task.is_some());
        assert!(e.fetched_vtodo.is_none());
        assert_eq!(e.calendar_url.as_deref(), Some("https://dav.example.com/cal/"));
        assert!(!e.dirty_tw);
        assert!(!e.dirty_caldav);
        assert!(!e.cyclic);
    }

    #[test]
    fn paired_tw_and_caldav() {
        let config = make_config(vec![("default", "https://dav.example.com/cal/")]);
        let uuid = Uuid::new_v4();
        let caldav_uid = "existing-uid-123";
        let tasks = vec![make_tw_task(uuid, Some(caldav_uid), None)];
        let vtodo = make_vtodo(caldav_uid, Some("NEEDS-ACTION"), None);
        let mut vtodos = HashMap::new();
        vtodos.insert(
            "https://dav.example.com/cal/".to_string(),
            vec![vtodo],
        );

        let (entries, warnings) = build_ir(&tasks, &vtodos, &config);

        assert!(warnings.is_empty());
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.tw_uuid, Some(uuid));
        assert_eq!(e.caldav_uid.as_deref(), Some(caldav_uid));
        assert!(e.tw_task.is_some());
        assert!(e.fetched_vtodo.is_some());
    }

    #[test]
    fn orphaned_entry_preserves_caldav_uid() {
        let config = make_config(vec![("default", "https://dav.example.com/cal/")]);
        let uuid = Uuid::new_v4();
        let orphan_uid = "orphan-uid-456";
        let tasks = vec![make_tw_task(uuid, Some(orphan_uid), None)];

        let (entries, warnings) = build_ir(&tasks, &HashMap::new(), &config);

        assert!(warnings.is_empty());
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.tw_uuid, Some(uuid));
        assert_eq!(e.caldav_uid.as_deref(), Some(orphan_uid));
        assert!(e.fetched_vtodo.is_none());
    }

    #[test]
    fn caldav_only_needs_action_gets_fresh_tw_uuid() {
        let config = make_config(vec![("default", "https://dav.example.com/cal/")]);
        let vtodo = make_vtodo("needs-action-uid", Some("NEEDS-ACTION"), None);
        let mut vtodos = HashMap::new();
        vtodos.insert(
            "https://dav.example.com/cal/".to_string(),
            vec![vtodo],
        );

        let (entries, warnings) = build_ir(&[], &vtodos, &config);

        assert!(warnings.is_empty());
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert!(e.tw_uuid.is_some(), "NEEDS-ACTION entry must get a fresh UUID4");
        assert!(e.tw_task.is_none());
        assert!(e.fetched_vtodo.is_some());
    }

    #[test]
    fn caldav_only_completed_has_no_tw_uuid() {
        let config = make_config(vec![("default", "https://dav.example.com/cal/")]);
        let vtodo = make_vtodo("completed-uid", Some("COMPLETED"), None);
        let mut vtodos = HashMap::new();
        vtodos.insert(
            "https://dav.example.com/cal/".to_string(),
            vec![vtodo],
        );

        let (entries, _warnings) = build_ir(&[], &vtodos, &config);

        assert_eq!(entries.len(), 1);
        assert!(entries[0].tw_uuid.is_none(), "COMPLETED entry must have tw_uuid=None");
    }

    #[test]
    fn caldav_only_cancelled_has_no_tw_uuid() {
        let config = make_config(vec![("default", "https://dav.example.com/cal/")]);
        let vtodo = make_vtodo("cancelled-uid", Some("CANCELLED"), None);
        let mut vtodos = HashMap::new();
        vtodos.insert(
            "https://dav.example.com/cal/".to_string(),
            vec![vtodo],
        );

        let (entries, _warnings) = build_ir(&[], &vtodos, &config);

        assert_eq!(entries.len(), 1);
        assert!(entries[0].tw_uuid.is_none(), "CANCELLED entry must have tw_uuid=None");
    }

    #[test]
    fn rrule_vtodo_skipped_with_warning() {
        let config = make_config(vec![("default", "https://dav.example.com/cal/")]);
        let vtodo = make_vtodo("recurring-uid", Some("NEEDS-ACTION"), Some("FREQ=WEEKLY"));
        let mut vtodos = HashMap::new();
        vtodos.insert(
            "https://dav.example.com/cal/".to_string(),
            vec![vtodo],
        );

        let (entries, warnings) = build_ir(&[], &vtodos, &config);

        assert!(entries.is_empty(), "RRULE VTODO must be skipped");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("RecurringCalDavSkipped"));
    }

    #[test]
    fn unmapped_project_emits_warning() {
        let config = make_config(vec![]); // no calendars
        let uuid = Uuid::new_v4();
        let tasks = vec![make_tw_task(uuid, None, Some("work"))];

        let (entries, warnings) = build_ir(&tasks, &HashMap::new(), &config);

        assert_eq!(entries.len(), 1);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("UnmappedProject"));
        assert_eq!(warnings[0].tw_uuid, Some(uuid));
        assert!(entries[0].calendar_url.is_none());
    }

    #[test]
    fn project_mapped_to_correct_calendar() {
        let config = make_config(vec![
            ("work", "https://dav.example.com/work/"),
            ("default", "https://dav.example.com/default/"),
        ]);
        let uuid = Uuid::new_v4();
        let tasks = vec![make_tw_task(uuid, None, Some("work"))];

        let (entries, warnings) = build_ir(&tasks, &HashMap::new(), &config);

        assert!(warnings.is_empty());
        assert_eq!(entries[0].calendar_url.as_deref(), Some("https://dav.example.com/work/"));
    }

    #[test]
    fn caldav_only_entry_gets_project_from_config() {
        let config = make_config(vec![("work", "http://dav/work/")]);
        let uid = "test-uid-work";
        let fetched = make_vtodo(uid, Some("NEEDS-ACTION"), None);
        let mut vtodos: HashMap<String, Vec<FetchedVTODO>> = HashMap::new();
        vtodos.insert("http://dav/work/".to_string(), vec![fetched]);

        let (entries, _) = build_ir(&[], &vtodos, &config);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].project, Some("work".to_string()));
    }

    #[test]
    fn caldav_only_entry_with_default_project_gets_none() {
        let config = make_config(vec![("default", "http://dav/cal/")]);
        let uid = "test-uid-default";
        let fetched = make_vtodo(uid, Some("NEEDS-ACTION"), None);
        let mut vtodos: HashMap<String, Vec<FetchedVTODO>> = HashMap::new();
        vtodos.insert("http://dav/cal/".to_string(), vec![fetched]);

        let (entries, _) = build_ir(&[], &vtodos, &config);

        assert_eq!(entries.len(), 1);
        assert!(entries[0].project.is_none());
    }

    #[test]
    fn all_dirty_and_cyclic_false_after_construction() {
        let config = make_config(vec![("default", "https://dav.example.com/cal/")]);
        let uuid = Uuid::new_v4();
        let tasks = vec![make_tw_task(uuid, None, None)];

        let (entries, _) = build_ir(&tasks, &HashMap::new(), &config);

        assert!(!entries[0].dirty_tw);
        assert!(!entries[0].dirty_caldav);
        assert!(!entries[0].cyclic);
    }
}
