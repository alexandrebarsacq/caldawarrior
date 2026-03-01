use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// tw_date — serialize/deserialize DateTime<Utc> using TW's compact format
// YYYYMMDDTHHMMSSZ  (no hyphens, no colons)
// ---------------------------------------------------------------------------

pub mod tw_date {
    use chrono::{DateTime, NaiveDateTime, Utc};
    use serde::{self, Deserialize, Deserializer, Serializer};

    const FMT: &str = "%Y%m%dT%H%M%SZ";

    pub fn serialize<S>(dt: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&dt.format(FMT).to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        NaiveDateTime::parse_from_str(&s, FMT)
            .map(|ndt| ndt.and_utc())
            .map_err(serde::de::Error::custom)
    }
}

// ---------------------------------------------------------------------------
// tw_date_opt — same but for Option<DateTime<Utc>>
// ---------------------------------------------------------------------------

pub mod tw_date_opt {
    use chrono::{DateTime, NaiveDateTime, Utc};
    use serde::{self, Deserialize, Deserializer, Serializer};

    const FMT: &str = "%Y%m%dT%H%M%SZ";

    pub fn serialize<S>(opt: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match opt {
            Some(dt) => serializer.serialize_str(&dt.format(FMT).to_string()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt = Option::<String>::deserialize(deserializer)?;
        match opt {
            None => Ok(None),
            Some(s) => NaiveDateTime::parse_from_str(&s, FMT)
                .map(|ndt| Some(ndt.and_utc()))
                .map_err(serde::de::Error::custom),
        }
    }
}

// ---------------------------------------------------------------------------
// tw_depends — TW serializes depends as a comma-separated string OR an array.
// We always deserialize into Vec<Uuid> and serialize back as a comma-separated
// string (matching TW export format).
// ---------------------------------------------------------------------------

pub mod tw_depends {
    use serde::{self, Deserialize, Deserializer, Serializer};
    use uuid::Uuid;

    pub fn serialize<S>(uuids: &Vec<Uuid>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if uuids.is_empty() {
            // This branch is never reached when skip_serializing_if = "Vec::is_empty"
            // is used, but we handle it for completeness.
            serializer.serialize_str("")
        } else {
            let s: Vec<String> = uuids.iter().map(|u| u.to_string()).collect();
            serializer.serialize_str(&s.join(","))
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Uuid>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum StringOrVec {
            Str(String),
            Vec(Vec<String>),
        }

        let v = StringOrVec::deserialize(deserializer)?;
        match v {
            StringOrVec::Str(s) => {
                if s.is_empty() {
                    return Ok(vec![]);
                }
                s.split(',')
                    .map(|part| {
                        part.trim()
                            .parse::<Uuid>()
                            .map_err(serde::de::Error::custom)
                    })
                    .collect()
            }
            StringOrVec::Vec(arr) => arr
                .iter()
                .map(|part| {
                    part.trim()
                        .parse::<Uuid>()
                        .map_err(serde::de::Error::custom)
                })
                .collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// TWTask — mirrors the JSON produced by `task export`
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TWTask {
    pub uuid: Uuid,
    pub status: String,
    pub description: String,

    #[serde(with = "tw_date")]
    pub entry: DateTime<Utc>,

    #[serde(default, skip_serializing_if = "Option::is_none", with = "tw_date_opt")]
    pub modified: Option<DateTime<Utc>>,

    #[serde(default, skip_serializing_if = "Option::is_none", with = "tw_date_opt")]
    pub due: Option<DateTime<Utc>>,

    #[serde(default, skip_serializing_if = "Option::is_none", with = "tw_date_opt")]
    pub scheduled: Option<DateTime<Utc>>,

    #[serde(default, skip_serializing_if = "Option::is_none", with = "tw_date_opt")]
    pub wait: Option<DateTime<Utc>>,

    #[serde(default, skip_serializing_if = "Option::is_none", with = "tw_date_opt")]
    pub until: Option<DateTime<Utc>>,

    #[serde(default, skip_serializing_if = "Option::is_none", with = "tw_date_opt")]
    pub end: Option<DateTime<Utc>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caldavuid: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recur: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub urgency: Option<f64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,

    #[serde(default, skip_serializing_if = "Vec::is_empty", with = "tw_depends")]
    pub depends: Vec<Uuid>,
}

// ---------------------------------------------------------------------------
// RelType — dependency relationship type for VTODO RELATED-TO properties
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelType {
    DependsOn,
    Other(String),
}

// ---------------------------------------------------------------------------
// IcalProp — an arbitrary iCalendar property (name, params, value)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IcalProp {
    pub name: String,
    pub params: Vec<(String, String)>,
    pub value: String,
}

// ---------------------------------------------------------------------------
// VTODO — CalDAV/iCalendar VTODO component (parsed representation)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VTODO {
    pub uid: String,
    pub summary: Option<String>,
    pub description: Option<String>,
    /// NEEDS-ACTION | COMPLETED | CANCELLED | IN-PROCESS
    pub status: Option<String>,
    pub last_modified: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dtstamp: Option<DateTime<Utc>>,
    pub dtstart: Option<DateTime<Utc>>,
    pub due: Option<DateTime<Utc>>,
    pub completed: Option<DateTime<Utc>>,
    #[serde(default)]
    pub categories: Vec<String>,
    pub rrule: Option<String>,
    #[serde(default)]
    pub depends: Vec<(RelType, String)>,
    #[serde(default)]
    pub extra_props: Vec<IcalProp>,
}

// ---------------------------------------------------------------------------
// FetchedVTODO — a VTODO retrieved from the CalDAV server, with its href/etag
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchedVTODO {
    pub href: String,
    pub etag: Option<String>,
    pub vtodo: VTODO,
}

// ---------------------------------------------------------------------------
// IREntry — intermediate representation pairing TW + CalDAV data
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IREntry {
    /// Present for TW tasks and CalDAV-only NEEDS-ACTION/IN-PROCESS entries (pre-assigned UUID4).
    /// None for CalDAV-only CANCELLED/COMPLETED entries.
    pub tw_uuid: Option<Uuid>,
    /// CalDAV UID from the `caldavuid` UDA or directly from VTODO UID.
    pub caldav_uid: Option<String>,
    /// None if this entry originates solely from CalDAV.
    pub tw_task: Option<TWTask>,
    /// None if this entry originates solely from TaskWarrior.
    pub fetched_vtodo: Option<FetchedVTODO>,
    /// CalDAV UIDs of resolved dependencies (populated by resolve_dependencies).
    #[serde(default)]
    pub resolved_depends: Vec<String>,
    /// True if this entry is part of a dependency cycle.
    #[serde(default)]
    pub cyclic: bool,
    /// Resolved calendar URL for this entry (populated at IR construction time).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calendar_url: Option<String>,
    /// True if the TW side needs to be written back after sync decision.
    #[serde(default)]
    pub dirty_tw: bool,
    /// True if the CalDAV side needs to be written back after sync decision.
    #[serde(default)]
    pub dirty_caldav: bool,
}

// ---------------------------------------------------------------------------
// Side — which system "won" during conflict resolution
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Side {
    Tw,
    CalDav,
}

// ---------------------------------------------------------------------------
// UpdateReason — exactly 5 variants
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UpdateReason {
    LwwTwWins,
    LwwCalDavWins,
    TwDeletedMarkCancelled,
    TwCompletedMarkCompleted,
    CalDavCompletedUpdateTw,
}

// ---------------------------------------------------------------------------
// SkipReason — exactly 8 variants
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SkipReason {
    Cancelled,
    Completed,
    Recurring,
    Cyclic,
    /// Both sides match on all 8 fields:
    /// uuid, description, status, due, scheduled, priority, project, tags.
    Identical,
    DeletedBeforeSync,
    AlreadyDeleted,
    CalDavDeletedTwTerminal,
}

// ---------------------------------------------------------------------------
// PlannedOp — the operations the sync engine plans to execute
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlannedOp {
    PushToCalDav(IREntry),
    PullFromCalDav(IREntry),
    DeleteFromCalDav(IREntry),
    DeleteFromTw(IREntry),
    ResolveConflict {
        entry: IREntry,
        winner: Side,
        reason: UpdateReason,
    },
    Skip {
        tw_uuid: Option<Uuid>,
        reason: SkipReason,
    },
}

// ---------------------------------------------------------------------------
// Warning — a non-fatal diagnostic emitted during sync
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Warning {
    pub tw_uuid: Option<Uuid>,
    pub message: String,
}

// ---------------------------------------------------------------------------
// SyncResult — summary of a sync run
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub planned_ops: Vec<PlannedOp>,
    pub warnings: Vec<Warning>,
    pub errors: Vec<String>,
    pub written_tw: usize,
    pub written_caldav: usize,
    pub skipped: usize,
}

// ---------------------------------------------------------------------------
// CyclicEntry — a TW task detected as cyclic (e.g., dependency loop)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CyclicEntry {
    pub tw_task: TWTask,
    pub reason: String,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tw_task_roundtrip_minimal() {
        let json = r#"{
            "uuid": "550e8400-e29b-41d4-a716-446655440000",
            "status": "pending",
            "description": "Test task",
            "entry": "20260226T140818Z"
        }"#;
        let task: TWTask = serde_json::from_str(json).expect("deserialize");
        assert_eq!(task.status, "pending");
        assert_eq!(task.description, "Test task");
        assert!(task.due.is_none());
        assert!(task.caldavuid.is_none());
        assert!(task.depends.is_empty());
        // round-trip
        let back = serde_json::to_string(&task).expect("serialize");
        let task2: TWTask = serde_json::from_str(&back).expect("re-deserialize");
        assert_eq!(task.uuid, task2.uuid);
    }
}
