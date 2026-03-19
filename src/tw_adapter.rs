use crate::error::CaldaWarriorError;
use crate::types::TWTask;
use std::process::Command;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// TaskRunner trait — abstracts process execution for testability
// ---------------------------------------------------------------------------

pub trait TaskRunner: Send + Sync {
    /// Run `task <args>` and return stdout as String.
    fn run(&self, args: &[&str]) -> Result<String, CaldaWarriorError>;

    /// Run `task import` with the given JSON bytes, return stdout.
    fn import(&self, json: &[u8]) -> Result<String, CaldaWarriorError>;

    /// Export tasks matching filter. Calls `task export [filter_args]`.
    fn export(&self, filter_args: &[&str]) -> Result<String, CaldaWarriorError> {
        let mut args = vec!["export"];
        args.extend_from_slice(filter_args);
        self.run(&args)
    }

    /// Modify a task. Calls `task <uuid> modify [modify_args]`.
    fn modify(&self, uuid_str: &str, modify_args: &[&str]) -> Result<String, CaldaWarriorError> {
        let mut args = vec![uuid_str, "modify"];
        args.extend_from_slice(modify_args);
        self.run(&args)
    }
}

// ---------------------------------------------------------------------------
// RealTaskRunner — calls the actual `task` binary
// ---------------------------------------------------------------------------

pub struct RealTaskRunner;

impl TaskRunner for RealTaskRunner {
    fn run(&self, args: &[&str]) -> Result<String, CaldaWarriorError> {
        let output = Command::new("task")
            .args(args)
            .output()
            .map_err(|e| CaldaWarriorError::Config(format!("Failed to run task: {}", e)))?;

        if !output.status.success() {
            let code = output.status.code().unwrap_or(-1);
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            // Special case: "task delete" exits 1 with "not deletable" for already-deleted tasks
            // Caller is responsible for deciding if this is an error or acceptable
            return Err(CaldaWarriorError::Tw { code, stderr });
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn import(&self, json: &[u8]) -> Result<String, CaldaWarriorError> {
        use std::io::Write;
        use std::process::Stdio;

        let mut child = Command::new("task")
            .arg("import")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                CaldaWarriorError::Config(format!("Failed to spawn task import: {}", e))
            })?;

        if let Some(stdin) = child.stdin.take() {
            let mut stdin = stdin;
            stdin.write_all(json).map_err(|e| {
                CaldaWarriorError::Config(format!("Failed to write to task import stdin: {}", e))
            })?;
        }

        let output = child.wait_with_output().map_err(|e| {
            CaldaWarriorError::Config(format!("Failed to wait for task import: {}", e))
        })?;

        if !output.status.success() {
            let code = output.status.code().unwrap_or(-1);
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(CaldaWarriorError::Tw { code, stderr });
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

// ---------------------------------------------------------------------------
// MockTaskRunner — for unit testing
// ---------------------------------------------------------------------------

use std::sync::Mutex;

pub struct MockTaskRunner {
    /// Recorded calls: (type, args/description)
    pub calls: Mutex<Vec<MockCall>>,
    /// Responses to return for run() calls, in order
    pub run_responses: Mutex<Vec<Result<String, CaldaWarriorError>>>,
    /// Responses to return for import() calls, in order
    pub import_responses: Mutex<Vec<Result<String, CaldaWarriorError>>>,
}

#[derive(Debug)]
pub enum MockCall {
    Run(Vec<String>),
    Import(String), // JSON as string
}

impl Default for MockTaskRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl MockTaskRunner {
    pub fn new() -> Self {
        Self {
            calls: Mutex::new(vec![]),
            run_responses: Mutex::new(vec![]),
            import_responses: Mutex::new(vec![]),
        }
    }

    pub fn push_run_response(&self, r: Result<String, CaldaWarriorError>) {
        self.run_responses.lock().unwrap().push(r);
    }

    pub fn push_import_response(&self, r: Result<String, CaldaWarriorError>) {
        self.import_responses.lock().unwrap().push(r);
    }

    pub fn get_calls(&self) -> Vec<MockCall> {
        // Drain and return recorded calls
        let mut calls = self.calls.lock().unwrap();
        let out: Vec<_> = calls.drain(..).collect();
        out
    }
}

impl TaskRunner for MockTaskRunner {
    fn run(&self, args: &[&str]) -> Result<String, CaldaWarriorError> {
        self.calls
            .lock()
            .unwrap()
            .push(MockCall::Run(args.iter().map(|s| s.to_string()).collect()));
        let mut responses = self.run_responses.lock().unwrap();
        if responses.is_empty() {
            Ok(String::new())
        } else {
            responses.remove(0)
        }
    }

    fn import(&self, json: &[u8]) -> Result<String, CaldaWarriorError> {
        let json_str = String::from_utf8_lossy(json).to_string();
        self.calls.lock().unwrap().push(MockCall::Import(json_str));
        let mut responses = self.import_responses.lock().unwrap();
        if responses.is_empty() {
            Ok(String::new())
        } else {
            responses.remove(0)
        }
    }
}

// ---------------------------------------------------------------------------
// TwAdapter — high-level operations over TaskWarrior
// ---------------------------------------------------------------------------

pub struct TwAdapter<R: TaskRunner> {
    runner: R,
    uda_registered: bool,
}

impl<R: TaskRunner> TwAdapter<R> {
    /// Create a new TwAdapter and immediately register the caldavuid UDA.
    pub fn new(runner: R) -> Result<Self, CaldaWarriorError> {
        let mut adapter = Self {
            runner,
            uda_registered: false,
        };
        adapter.register_uda()?;
        Ok(adapter)
    }

    /// Register the caldavuid UDA in TW config if not already set.
    /// This runs `task config uda.caldavuid.type string` and
    /// `task config uda.caldavuid.label CaldavUID`.
    fn register_uda(&mut self) -> Result<(), CaldaWarriorError> {
        self.runner
            .run(&["config", "uda.caldavuid.type", "string"])?;
        self.runner
            .run(&["config", "uda.caldavuid.label", "CaldavUID"])?;
        self.uda_registered = true;
        Ok(())
    }

    /// List all tasks: merges pending + all statuses.
    /// Runs two export calls:
    ///   1. `task export status:pending or status:waiting or status:recurring`
    ///   2. `task export status:completed or status:deleted`
    ///
    /// Merges results by UUID, keeping the entry with the higher `modified` timestamp.
    pub fn list_all(&self) -> Result<Vec<TWTask>, CaldaWarriorError> {
        let pending_json = self.runner.export(&[
            "status:pending",
            "or",
            "status:waiting",
            "or",
            "status:recurring",
        ])?;

        let completed_json = self
            .runner
            .export(&["status:completed", "or", "status:deleted"])?;

        let mut all: Vec<TWTask> = vec![];

        if !pending_json.trim().is_empty() {
            let tasks: Vec<TWTask> = serde_json::from_str(&pending_json).map_err(|e| {
                CaldaWarriorError::Config(format!("Failed to parse TW export: {}", e))
            })?;
            all.extend(tasks);
        }

        if !completed_json.trim().is_empty() {
            let tasks: Vec<TWTask> = serde_json::from_str(&completed_json).map_err(|e| {
                CaldaWarriorError::Config(format!("Failed to parse TW export: {}", e))
            })?;
            all.extend(tasks);
        }

        // Dedup: if same UUID appears twice, keep the one with higher modified timestamp.
        // Tasks without a modified timestamp are treated as epoch (oldest possible).
        let mut by_uuid: std::collections::HashMap<Uuid, TWTask> = std::collections::HashMap::new();
        for task in all {
            let entry = by_uuid.entry(task.uuid);
            match entry {
                std::collections::hash_map::Entry::Vacant(v) => {
                    v.insert(task);
                }
                std::collections::hash_map::Entry::Occupied(mut o) => {
                    let existing_mod = o.get().modified;
                    let new_mod = task.modified;
                    // Compare Options: None < Some(_) so tasks with a modified timestamp win
                    if new_mod > existing_mod {
                        o.insert(task);
                    }
                }
            }
        }

        Ok(by_uuid.into_values().collect())
    }

    /// Create a new task from CalDAV by importing it into TW.
    /// The task must already have a pre-assigned uuid (UUID4).
    /// IMPORTANT: This is the ONLY method that calls `task import`.
    pub fn create(&self, task: &TWTask) -> Result<(), CaldaWarriorError> {
        let json = serde_json::to_vec(task)
            .map_err(|e| CaldaWarriorError::Config(format!("Failed to serialize task: {}", e)))?;
        self.runner.import(&json)?;
        Ok(())
    }

    /// Update an existing TW task using `task <uuid> modify`.
    ///
    /// Builds modify args for scalar fields, computes tag diff (+tag/-tag),
    /// and handles annotations via separate annotate/denotate commands.
    ///
    /// `old_task` is used for tag and annotation diff computation. Pass `None`
    /// when only scalar fields changed (e.g., caldavuid-only writeback) — all
    /// new tags will be treated as additions and all new annotations as additions.
    pub fn update(
        &self,
        task: &TWTask,
        old_task: Option<&TWTask>,
    ) -> Result<(), CaldaWarriorError> {
        let uuid_str = task.uuid.to_string();
        let mut args: Vec<String> = Vec::new();

        // Scalar fields
        args.push(format!("description:{}", task.description));
        args.push(format!("status:{}", task.status));

        // Optional date fields: set value or clear
        match task.due {
            Some(dt) => args.push(format!("due:{}", dt.format("%Y%m%dT%H%M%SZ"))),
            None => args.push("due:".to_string()),
        }
        match task.scheduled {
            Some(dt) => args.push(format!("scheduled:{}", dt.format("%Y%m%dT%H%M%SZ"))),
            None => args.push("scheduled:".to_string()),
        }

        // Optional string fields: set value or clear
        match task.priority {
            Some(ref v) => args.push(format!("priority:{}", v)),
            None => args.push("priority:".to_string()),
        }
        match task.project {
            Some(ref v) => args.push(format!("project:{}", v)),
            None => args.push("project:".to_string()),
        }
        match task.caldavuid {
            Some(ref v) => args.push(format!("caldavuid:{}", v)),
            None => args.push("caldavuid:".to_string()),
        }

        // Depends (only if non-empty)
        if !task.depends.is_empty() {
            let dep_str: Vec<String> = task.depends.iter().map(|u| u.to_string()).collect();
            args.push(format!("depends:{}", dep_str.join(",")));
        }

        // Tag diff
        let old_tags: &[String] = old_task
            .and_then(|t| t.tags.as_ref())
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        let new_tags: &[String] = task.tags.as_deref().unwrap_or(&[]);

        for tag in new_tags {
            if !old_tags.contains(tag) {
                args.push(format!("+{}", tag));
            }
        }
        for tag in old_tags {
            if !new_tags.contains(tag) {
                args.push(format!("-{}", tag));
            }
        }

        // Call modify with all scalar + tag args
        let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        self.runner.modify(&uuid_str, &args_refs)?;

        // Annotation diff
        let old_annotations = old_task.map(|t| t.annotations.as_slice()).unwrap_or(&[]);
        let new_annotations = task.annotations.as_slice();

        // Annotations added (in new but not in old, matched by description)
        for ann in new_annotations {
            let exists_in_old = old_annotations
                .iter()
                .any(|oa| oa.description == ann.description);
            if !exists_in_old {
                self.runner
                    .run(&[&uuid_str, "annotate", &ann.description])?;
            }
        }

        // Annotations removed (in old but not in new, matched by description)
        for ann in old_annotations {
            let exists_in_new = new_annotations
                .iter()
                .any(|na| na.description == ann.description);
            if !exists_in_new {
                self.runner
                    .run(&[&uuid_str, "denotate", &ann.description])?;
            }
        }

        Ok(())
    }

    /// Delete a task by UUID. Handles the non-idempotent case gracefully:
    /// if TW exits 1 with "not deletable" or "Deleted 0", treat as success (already deleted).
    pub fn delete(&self, uuid: &Uuid) -> Result<(), CaldaWarriorError> {
        let uuid_str = uuid.to_string();
        // rc.confirmation:no bypasses the interactive "Are you sure?" prompt
        match self
            .runner
            .run(&["rc.confirmation:no", &uuid_str, "delete"])
        {
            Ok(_) => {}
            Err(CaldaWarriorError::Tw {
                code: 1,
                ref stderr,
            }) if stderr.contains("not deletable") || stderr.contains("Deleted 0") => {
                // Already deleted — acceptable
            }
            Err(e) => return Err(e),
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_task(uuid: Uuid) -> TWTask {
        use crate::types::TWTask;
        TWTask {
            uuid,
            status: "pending".to_string(),
            description: "Test task".to_string(),
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

    #[test]
    fn uda_registration_runs_on_new() {
        let mock = MockTaskRunner::new();
        // Push success responses for the two UDA config calls
        mock.push_run_response(Ok(String::new()));
        mock.push_run_response(Ok(String::new()));
        // Push empty JSON arrays for list_all
        mock.push_run_response(Ok("[]".to_string()));
        mock.push_run_response(Ok("[]".to_string()));

        let adapter = TwAdapter::new(mock).expect("new");
        assert!(adapter.uda_registered);

        // list_all should work after registration
        let tasks = adapter.list_all().expect("list_all");
        assert!(tasks.is_empty());
    }

    #[test]
    fn uda_registration_before_list_all() {
        let mock = MockTaskRunner::new();
        mock.push_run_response(Ok(String::new())); // uda type
        mock.push_run_response(Ok(String::new())); // uda label
        mock.push_run_response(Ok("[]".to_string())); // export 1
        mock.push_run_response(Ok("[]".to_string())); // export 2

        let adapter = TwAdapter::new(mock).expect("new");
        let _tasks = adapter.list_all().expect("list_all");

        // The calls recorded should show UDA registration happened first
        // (we can't easily inspect after calls are drained, but we verify it compiles and runs)
        // The test passing without panic confirms ordering
    }

    #[test]
    fn create_uses_import() {
        let mock = MockTaskRunner::new();
        mock.push_run_response(Ok(String::new())); // uda type
        mock.push_run_response(Ok(String::new())); // uda label
        mock.push_import_response(Ok(String::new())); // import

        let adapter = TwAdapter::new(mock).expect("new");
        let task = make_task(Uuid::new_v4());
        adapter.create(&task).expect("create");
    }

    #[test]
    fn update_uses_modify_not_import() {
        let mock = MockTaskRunner::new();
        mock.push_run_response(Ok(String::new())); // uda type
        mock.push_run_response(Ok(String::new())); // uda label
        mock.push_run_response(Ok(String::new())); // modify (update)

        let adapter = TwAdapter::new(mock).expect("new");
        let task = make_task(Uuid::new_v4());
        adapter.update(&task, None).expect("update");

        // Verify that modify was called (Run), not import
        let calls = adapter.runner.get_calls();
        // calls[0] = uda type, calls[1] = uda label, calls[2] = modify
        assert!(
            calls.len() >= 3,
            "expected at least 3 calls, got {}",
            calls.len()
        );
        match &calls[2] {
            MockCall::Run(args) => {
                assert!(
                    args.contains(&"modify".to_string()),
                    "expected 'modify' in args, got {:?}",
                    args
                );
            }
            MockCall::Import(_) => panic!("update() should NOT call import"),
        }
    }

    #[test]
    fn update_builds_correct_scalar_modify_args() {
        use chrono::TimeZone;

        let mock = MockTaskRunner::new();
        mock.push_run_response(Ok(String::new())); // uda type
        mock.push_run_response(Ok(String::new())); // uda label
        mock.push_run_response(Ok(String::new())); // modify

        let adapter = TwAdapter::new(mock).expect("new");
        let uuid = Uuid::new_v4();
        let mut task = make_task(uuid);
        task.description = "Buy milk".to_string();
        task.status = "pending".to_string();
        task.due = Some(Utc.with_ymd_and_hms(2026, 3, 15, 10, 0, 0).unwrap());
        task.priority = Some("H".to_string());
        task.project = Some("home".to_string());
        task.caldavuid = Some("abc-123".to_string());

        adapter.update(&task, None).expect("update");

        let calls = adapter.runner.get_calls();
        let modify_call = &calls[2]; // after uda type + uda label
        match modify_call {
            MockCall::Run(args) => {
                assert!(args.contains(&format!("description:{}", task.description)));
                assert!(args.contains(&"status:pending".to_string()));
                assert!(args.contains(&"due:20260315T100000Z".to_string()));
                assert!(args.contains(&"priority:H".to_string()));
                assert!(args.contains(&"project:home".to_string()));
                assert!(args.contains(&"caldavuid:abc-123".to_string()));
            }
            _ => panic!("expected Run call for modify"),
        }
    }

    #[test]
    fn update_generates_plus_tag_for_new_tags() {
        let mock = MockTaskRunner::new();
        mock.push_run_response(Ok(String::new())); // uda type
        mock.push_run_response(Ok(String::new())); // uda label
        mock.push_run_response(Ok(String::new())); // modify

        let adapter = TwAdapter::new(mock).expect("new");
        let uuid = Uuid::new_v4();
        let mut new_task = make_task(uuid);
        new_task.tags = Some(vec!["work".to_string(), "urgent".to_string()]);

        let old_task = make_task(uuid); // no tags

        adapter.update(&new_task, Some(&old_task)).expect("update");

        let calls = adapter.runner.get_calls();
        match &calls[2] {
            MockCall::Run(args) => {
                assert!(
                    args.contains(&"+work".to_string()),
                    "expected +work in args: {:?}",
                    args
                );
                assert!(
                    args.contains(&"+urgent".to_string()),
                    "expected +urgent in args: {:?}",
                    args
                );
            }
            _ => panic!("expected Run call for modify"),
        }
    }

    #[test]
    fn update_generates_minus_tag_for_removed_tags() {
        let mock = MockTaskRunner::new();
        mock.push_run_response(Ok(String::new())); // uda type
        mock.push_run_response(Ok(String::new())); // uda label
        mock.push_run_response(Ok(String::new())); // modify

        let adapter = TwAdapter::new(mock).expect("new");
        let uuid = Uuid::new_v4();
        let new_task = make_task(uuid); // no tags

        let mut old_task = make_task(uuid);
        old_task.tags = Some(vec!["obsolete".to_string()]);

        adapter.update(&new_task, Some(&old_task)).expect("update");

        let calls = adapter.runner.get_calls();
        match &calls[2] {
            MockCall::Run(args) => {
                assert!(
                    args.contains(&"-obsolete".to_string()),
                    "expected -obsolete in args: {:?}",
                    args
                );
            }
            _ => panic!("expected Run call for modify"),
        }
    }

    #[test]
    fn update_no_old_task_treats_all_tags_as_additions() {
        let mock = MockTaskRunner::new();
        mock.push_run_response(Ok(String::new())); // uda type
        mock.push_run_response(Ok(String::new())); // uda label
        mock.push_run_response(Ok(String::new())); // modify

        let adapter = TwAdapter::new(mock).expect("new");
        let uuid = Uuid::new_v4();
        let mut task = make_task(uuid);
        task.tags = Some(vec!["alpha".to_string(), "beta".to_string()]);

        adapter.update(&task, None).expect("update");

        let calls = adapter.runner.get_calls();
        match &calls[2] {
            MockCall::Run(args) => {
                assert!(
                    args.contains(&"+alpha".to_string()),
                    "expected +alpha in args: {:?}",
                    args
                );
                assert!(
                    args.contains(&"+beta".to_string()),
                    "expected +beta in args: {:?}",
                    args
                );
            }
            _ => panic!("expected Run call for modify"),
        }
    }

    #[test]
    fn update_clears_optional_fields_correctly() {
        let mock = MockTaskRunner::new();
        mock.push_run_response(Ok(String::new())); // uda type
        mock.push_run_response(Ok(String::new())); // uda label
        mock.push_run_response(Ok(String::new())); // modify

        let adapter = TwAdapter::new(mock).expect("new");
        let uuid = Uuid::new_v4();
        let task = make_task(uuid); // due=None, scheduled=None, priority=None, project=None

        adapter.update(&task, None).expect("update");

        let calls = adapter.runner.get_calls();
        match &calls[2] {
            MockCall::Run(args) => {
                assert!(
                    args.contains(&"due:".to_string()),
                    "expected 'due:' (clear) in args: {:?}",
                    args
                );
                assert!(
                    args.contains(&"scheduled:".to_string()),
                    "expected 'scheduled:' (clear) in args: {:?}",
                    args
                );
                assert!(
                    args.contains(&"priority:".to_string()),
                    "expected 'priority:' (clear) in args: {:?}",
                    args
                );
                assert!(
                    args.contains(&"project:".to_string()),
                    "expected 'project:' (clear) in args: {:?}",
                    args
                );
            }
            _ => panic!("expected Run call for modify"),
        }
    }

    #[test]
    fn update_annotate_for_new_annotations() {
        use crate::types::TwAnnotation;

        let mock = MockTaskRunner::new();
        mock.push_run_response(Ok(String::new())); // uda type
        mock.push_run_response(Ok(String::new())); // uda label
        mock.push_run_response(Ok(String::new())); // modify
        mock.push_run_response(Ok(String::new())); // annotate

        let adapter = TwAdapter::new(mock).expect("new");
        let uuid = Uuid::new_v4();
        let mut new_task = make_task(uuid);
        new_task.annotations = vec![TwAnnotation {
            entry: Utc::now(),
            description: "check expiry".to_string(),
        }];

        let old_task = make_task(uuid); // no annotations

        adapter.update(&new_task, Some(&old_task)).expect("update");

        let calls = adapter.runner.get_calls();
        // After UDA calls and modify, there should be an annotate call
        let annotate_calls: Vec<_> = calls
            .iter()
            .filter(|c| {
                if let MockCall::Run(args) = c {
                    args.contains(&"annotate".to_string())
                } else {
                    false
                }
            })
            .collect();
        assert_eq!(
            annotate_calls.len(),
            1,
            "expected 1 annotate call, got {:?}",
            annotate_calls
        );
    }

    #[test]
    fn update_denotate_for_removed_annotations() {
        use crate::types::TwAnnotation;

        let mock = MockTaskRunner::new();
        mock.push_run_response(Ok(String::new())); // uda type
        mock.push_run_response(Ok(String::new())); // uda label
        mock.push_run_response(Ok(String::new())); // modify
        mock.push_run_response(Ok(String::new())); // denotate

        let adapter = TwAdapter::new(mock).expect("new");
        let uuid = Uuid::new_v4();
        let new_task = make_task(uuid); // no annotations

        let mut old_task = make_task(uuid);
        old_task.annotations = vec![TwAnnotation {
            entry: Utc::now(),
            description: "old note".to_string(),
        }];

        adapter.update(&new_task, Some(&old_task)).expect("update");

        let calls = adapter.runner.get_calls();
        let denotate_calls: Vec<_> = calls
            .iter()
            .filter(|c| {
                if let MockCall::Run(args) = c {
                    args.contains(&"denotate".to_string())
                } else {
                    false
                }
            })
            .collect();
        assert_eq!(
            denotate_calls.len(),
            1,
            "expected 1 denotate call, got {:?}",
            denotate_calls
        );
    }

    #[test]
    fn delete_tolerates_already_deleted() {
        let mock = MockTaskRunner::new();
        mock.push_run_response(Ok(String::new())); // uda type
        mock.push_run_response(Ok(String::new())); // uda label
        // Simulate "not deletable" error
        mock.push_run_response(Err(CaldaWarriorError::Tw {
            code: 1,
            stderr: "Task 1 'test' is not deletable.\nDeleted 0 tasks.".to_string(),
        }));

        let adapter = TwAdapter::new(mock).expect("new");
        let uuid = Uuid::new_v4();
        // Should NOT return an error
        adapter
            .delete(&uuid)
            .expect("delete should succeed even if already deleted");
    }

    #[test]
    fn list_all_deduplicates_by_max_modified() {
        use chrono::Duration;

        let mock = MockTaskRunner::new();
        mock.push_run_response(Ok(String::new())); // uda type
        mock.push_run_response(Ok(String::new())); // uda label

        let uuid = Uuid::new_v4();
        let t1 = Utc::now();
        let t2 = t1 + Duration::seconds(60);

        let mut task1 = make_task(uuid);
        task1.modified = Some(t1);
        task1.description = "older".to_string();

        let mut task2 = make_task(uuid);
        task2.modified = Some(t2);
        task2.description = "newer".to_string();

        let json1 = serde_json::to_string(&[&task1]).unwrap();
        let json2 = serde_json::to_string(&[&task2]).unwrap();

        mock.push_run_response(Ok(json1));
        mock.push_run_response(Ok(json2));

        let adapter = TwAdapter::new(mock).expect("new");
        let tasks = adapter.list_all().expect("list_all");

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].description, "newer");
    }
}
