import glob
import json
import os
import re
import shutil
import subprocess


class TaskWarriorLibrary:
    """Robot Framework keyword library for TaskWarrior operations.

    Provides keywords for managing TaskWarrior tasks in isolated test environments.
    All operations use a dedicated data directory to avoid contaminating system-level
    TaskWarrior data.

    Scope: SUITE — one instance per test suite.
    """

    ROBOT_LIBRARY_SCOPE = 'SUITE'

    def __init__(self):
        """Initialize the library with no active data directory."""
        self._tw_env = None
        self._data_dir = None

    def set_tw_data_dir(self, data_dir):
        """Configure TaskWarrior to use an isolated data directory.

        Creates the directory if it does not exist, writes a .taskrc file with
        the required UDA definitions, and sets the environment variables used
        by all subsequent task commands.

        Args:
            data_dir: Absolute path to the directory for TaskWarrior data files.

        Returns:
            None
        """
        os.makedirs(data_dir, exist_ok=True)
        taskrc_path = os.path.join(data_dir, ".taskrc")
        taskrc_content = (
            f"data.location={data_dir}\n"
            "uda.caldavuid.type=string\n"
            "uda.caldavuid.label=CaldavUID\n"
            "confirmation=no\n"
            "json.array=on\n"
        )
        with open(taskrc_path, "w") as f:
            f.write(taskrc_content)
        self._tw_env = {
            **os.environ,
            "TASKDATA": data_dir,
            "TASKRC": taskrc_path,
        }
        self._data_dir = data_dir

    def add_tw_task(self, description, project=None, due=None):
        """Add a new task to TaskWarrior and return its UUID.

        Runs 'task add' with the given description and optional attributes,
        then exports the created task to retrieve its UUID.

        Args:
            description: The task description string.
            project:     Optional project name to assign to the task.
            due:         Optional due date string (e.g. '2026-03-15').

        Returns:
            The UUID string (36-character UUID4) of the newly created task.
        """
        args = ["add", description]
        if project is not None:
            args.append(f"project:{project}")
        if due is not None:
            args.append(f"due:{due}")

        result = subprocess.run(
            ["task"] + args,
            env=self._tw_env,
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode != 0:
            raise AssertionError(
                f"task {' '.join(args)} failed (exit {result.returncode}): "
                f"{result.stderr.strip()}"
            )

        match = re.search(r"Created task (\d+)\.", result.stdout)
        if not match:
            raise AssertionError(
                f"Could not parse task number from output: {result.stdout.strip()}"
            )
        task_number = match.group(1)

        export_result = subprocess.run(
            ["task", task_number, "export"],
            env=self._tw_env,
            capture_output=True,
            text=True,
            check=False,
        )
        if export_result.returncode != 0:
            raise AssertionError(
                f"task export {task_number} failed (exit {export_result.returncode}): "
                f"{export_result.stderr.strip()}"
            )

        tasks = json.loads(export_result.stdout)
        if not tasks:
            raise AssertionError(f"Task number {task_number} not found after creation")

        return tasks[0]["uuid"]

    def get_tw_task(self, uuid):
        """Retrieve a task's data dict by UUID.

        Runs 'task export uuid:<uuid>' and returns the parsed task dictionary.

        Args:
            uuid: The UUID string of the task to retrieve.

        Returns:
            A dict containing the task's fields as exported by TaskWarrior.
        """
        result = subprocess.run(
            ["task", f"uuid:{uuid}", "export"],
            env=self._tw_env,
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode != 0:
            raise AssertionError(
                f"task export uuid:{uuid} failed (exit {result.returncode}): "
                f"{result.stderr.strip()}"
            )

        try:
            tasks = json.loads(result.stdout)
        except json.JSONDecodeError as e:
            raise AssertionError(
                f"Failed to parse JSON from 'task export uuid:{uuid}': {e}\n"
                f"Output was: {result.stdout.strip()}"
            )

        if not tasks:
            raise AssertionError(f"Task {uuid} not found")

        return tasks[0]

    def modify_tw_task(self, uuid, **kwargs):
        """Modify one or more fields on a TaskWarrior task.

        Runs 'task <uuid> modify key1:value1 key2:value2 ...' for each keyword
        argument provided.

        Args:
            uuid:    The UUID string of the task to modify.
            **kwargs: Field names and their new values (e.g. project='work').

        Returns:
            None
        """
        if not kwargs:
            return

        modifications = [f"{key}:{value}" for key, value in kwargs.items()]
        args = [uuid, "modify"] + modifications

        result = subprocess.run(
            ["task"] + args,
            env=self._tw_env,
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode != 0:
            raise AssertionError(
                f"task {' '.join(args)} failed (exit {result.returncode}): "
                f"{result.stderr.strip()}"
            )

    def complete_tw_task(self, uuid):
        """Mark a TaskWarrior task as completed.

        Runs 'task <uuid> done'.

        Args:
            uuid: The UUID string of the task to complete.

        Returns:
            None
        """
        args = [uuid, "done"]
        result = subprocess.run(
            ["task"] + args,
            env=self._tw_env,
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode != 0:
            raise AssertionError(
                f"task {' '.join(args)} failed (exit {result.returncode}): "
                f"{result.stderr.strip()}"
            )

    def delete_tw_task(self, uuid):
        """Delete a TaskWarrior task.

        Runs 'task <uuid> delete'.

        Args:
            uuid: The UUID string of the task to delete.

        Returns:
            None
        """
        args = [uuid, "delete"]
        result = subprocess.run(
            ["task"] + args,
            env=self._tw_env,
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode != 0:
            raise AssertionError(
                f"task {' '.join(args)} failed (exit {result.returncode}): "
                f"{result.stderr.strip()}"
            )

    def tw_task_count(self):
        """Count all pending tasks in the configured data directory.

        Runs 'task count status:pending'.

        Args:
            None

        Returns:
            Integer count of pending tasks.
        """
        args = ["count", "status:pending"]
        result = subprocess.run(
            ["task"] + args,
            env=self._tw_env,
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode != 0:
            raise AssertionError(
                f"task {' '.join(args)} failed (exit {result.returncode}): "
                f"{result.stderr.strip()}"
            )
        return int(result.stdout.strip())

    def tw_task_should_have_caldavuid(self, uuid):
        """Assert that a task has a non-empty caldavuid UDA field.

        Args:
            uuid: The UUID string of the task to inspect.

        Returns:
            None

        Raises:
            AssertionError: If the caldavuid field is absent or empty.
        """
        task = self.get_tw_task(uuid)
        caldavuid = task.get("caldavuid", "")
        if not caldavuid:
            raise AssertionError(
                f"Task {uuid} does not have a caldavuid field set. "
                f"Task data: {task}"
            )

    def tw_task_should_have_status(self, uuid, expected_status):
        """Assert that a task has the expected status value.

        Args:
            uuid:            The UUID string of the task to inspect.
            expected_status: The expected value of the 'status' field
                             (e.g. 'pending', 'completed', 'deleted').

        Returns:
            None

        Raises:
            AssertionError: If the actual status does not match expected_status.
        """
        task = self.get_tw_task(uuid)
        actual_status = task.get("status", "")
        if actual_status != expected_status:
            raise AssertionError(
                f"Task {uuid} status mismatch: "
                f"expected '{expected_status}' but got '{actual_status}'"
            )

    def tw_task_should_have_field(self, uuid, field, expected_value):
        """Assert that a task field equals the expected value.

        Args:
            uuid:           The UUID string of the task to inspect.
            field:          The name of the task field to check.
            expected_value: The expected value of the field.

        Returns:
            None

        Raises:
            AssertionError: If the field value does not match expected_value.
        """
        task = self.get_tw_task(uuid)
        actual_value = task.get(field)
        if actual_value != expected_value:
            raise AssertionError(
                f"Task {uuid} field '{field}' mismatch: "
                f"expected {expected_value!r} but got {actual_value!r}"
            )

    def tw_task_should_depend_on(self, uuid, expected_dependency_uuid):
        """Assert that a task has a specific UUID in its depends field.

        Args:
            uuid: The UUID string of the task to inspect.
            expected_dependency_uuid: The UUID that should appear in depends.

        Returns:
            None

        Raises:
            AssertionError: If the depends field is absent or does not contain
                expected_dependency_uuid.
        """
        task = self.get_tw_task(uuid)
        depends = task.get('depends', [])
        if isinstance(depends, str):
            depends = [depends]
        if expected_dependency_uuid not in depends:
            raise AssertionError(
                f"Task {uuid} does not depend on {expected_dependency_uuid}. "
                f"Current depends: {depends}"
            )

    def clear_tw_data(self):
        """Remove all TaskWarrior data from the configured data directory.

        Removes the entire data directory and recreates it with a fresh .taskrc.
        This works for both TW 2.x (.data files) and TW 3.x (SQLite .db files).

        Args:
            None

        Returns:
            None
        """
        if not self._data_dir:
            return
        shutil.rmtree(self._data_dir, ignore_errors=True)
        self.set_tw_data_dir(self._data_dir)
