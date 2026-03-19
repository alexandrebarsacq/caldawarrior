# caldawarrior

Bidirectional sync between [TaskWarrior](https://taskwarrior.org/) and CalDAV VTODO calendars.

caldawarrior is a CLI tool written in Rust that keeps your TaskWarrior tasks in sync with any
CalDAV-compatible server (Nextcloud, Radicale, Fastmail, iCloud, Baikal, etc.).
Each configured calendar collection maps to a TaskWarrior project; the tool performs a
three-step pipeline — IR construction, dependency resolution, write-back — using last-write-wins
conflict resolution.

## Features

- Bidirectional sync: changes on either side are propagated
- Project-to-calendar mapping via config (one or more calendars)
- Last-write-wins conflict resolution on a per-task basis
- Dry-run mode (`--dry-run`) to preview changes without writing
- Custom `caldavuid` UDA tracks the CalDAV identity of each task
- TLS strict by default (rustls); optional insecure mode for self-signed certificates
- Password override via environment variable for CI/scripting
- Runtime warning when config file permissions exceed 0600

## Installation

### Pre-built Binary (Recommended)

Download the latest release for x86_64 Linux:

```bash
# Download binary and checksum
curl -LO https://github.com/alexandrebarsacq/caldawarrior/releases/latest/download/caldawarrior-v1.0.0-x86_64-linux
curl -LO https://github.com/alexandrebarsacq/caldawarrior/releases/latest/download/caldawarrior-v1.0.0-x86_64-linux.sha256

# Verify checksum
sha256sum -c caldawarrior-v1.0.0-x86_64-linux.sha256

# Install
chmod +x caldawarrior-v1.0.0-x86_64-linux
sudo mv caldawarrior-v1.0.0-x86_64-linux /usr/local/bin/caldawarrior
```

Check the [Releases page](https://github.com/alexandrebarsacq/caldawarrior/releases) for the latest version.

### cargo install

```bash
cargo install --git https://github.com/alexandrebarsacq/caldawarrior.git
```

### Build from Source

```bash
git clone https://github.com/alexandrebarsacq/caldawarrior.git
cd caldawarrior
cargo build --release
# binary is at target/release/caldawarrior
```

## Quick Start

**Step 1 — Configure** (with security note)

Create the config directory and file:

```bash
mkdir -p ~/.config/caldawarrior
touch ~/.config/caldawarrior/config.toml
chmod 0600 ~/.config/caldawarrior/config.toml   # IMPORTANT: restrict permissions
```

Edit `~/.config/caldawarrior/config.toml`:

```toml
server_url = "https://dav.example.com"
username   = "alice"
password   = "hunter2"

[[calendar]]
project = "default"
url     = "https://dav.example.com/alice/default/"

[[calendar]]
project = "work"
url     = "https://dav.example.com/alice/work/"
```

The tool emits a `[WARN]` to stderr at startup if the config file is more permissive than
`0600` on Unix systems. Do not store this file in version control.

**Step 2 — Register the TaskWarrior UDA**

caldawarrior uses a custom User Defined Attribute (`caldavuid`) to track which CalDAV VTODO each
task corresponds to. Register it once:

```bash
task config uda.caldavuid.type  string
task config uda.caldavuid.label CalDAVUID
```

**Step 3 — Preview with dry-run**

Run a dry-run first to see what would happen without making any changes:

```bash
caldawarrior sync --dry-run
```

Review the output. No tasks or VTODOs are created, modified, or deleted in dry-run mode.

**Step 4 — First live sync**

```bash
caldawarrior sync
```

The tool exits with status 0 on success and non-zero if any errors occurred during sync.

## Config Reference

### Options

| Option | Type | Default | Required | Description |
|--------|------|---------|----------|-------------|
| `server_url` | string | -- | Yes | Base URL of the CalDAV server |
| `username` | string | -- | Yes | CalDAV username for authentication |
| `password` | string | -- | Yes | CalDAV password (see Environment Variables below) |
| `completed_cutoff_days` | integer | `90` | No | Number of days of completed/deleted task history to include in sync |
| `allow_insecure_tls` | boolean | `false` | No | Skip TLS certificate verification (for self-signed certificates) |
| `caldav_timeout_seconds` | integer | `30` | No | HTTP request timeout in seconds for CalDAV operations |

### Calendar Entries

Each `[[calendar]]` section maps a TaskWarrior project to a CalDAV collection:

| Option | Type | Required | Description |
|--------|------|----------|-------------|
| `project` | string | Yes | TaskWarrior project name |
| `url` | string | Yes | Full URL of the CalDAV calendar collection |

### Environment Variables

| Variable | Description |
|----------|-------------|
| `CALDAWARRIOR_PASSWORD` | Overrides the `password` field in config.toml |
| `CALDAWARRIOR_CONFIG` | Path to config file (overrides default `~/.config/caldawarrior/config.toml`) |

### Config Path Resolution

1. `--config` CLI flag (highest priority)
2. `CALDAWARRIOR_CONFIG` environment variable
3. `~/.config/caldawarrior/config.toml` (default)

## CLI Reference

```
caldawarrior [--config PATH] <SUBCOMMAND>

Options:
  --config PATH   Path to config file (overrides CALDAWARRIOR_CONFIG env var and default)

Subcommands:
  sync            Sync TaskWarrior tasks with CalDAV
    --dry-run     Preview changes without writing anything
    --fail-fast   Stop on first sync error instead of continuing
```

## Scheduling

### Cron

```bash
# Sync every 15 minutes with flock to prevent overlapping runs
*/15 * * * * /usr/bin/flock -n /tmp/caldawarrior.lock /usr/local/bin/caldawarrior sync >> /var/log/caldawarrior.log 2>&1
```

### Systemd Timer

Create two files:

**`~/.config/systemd/user/caldawarrior.service`**
```ini
[Unit]
Description=Sync TaskWarrior with CalDAV

[Service]
Type=oneshot
ExecStart=/usr/local/bin/caldawarrior sync
```

**`~/.config/systemd/user/caldawarrior.timer`**
```ini
[Unit]
Description=Run caldawarrior sync periodically

[Timer]
OnBootSec=1min
OnUnitActiveSec=15min
Persistent=true

[Install]
WantedBy=timers.target
```

Enable and start:
```bash
systemctl --user enable --now caldawarrior.timer
```

## Field Mapping

| TaskWarrior Field | CalDAV / VTODO Field         | Notes                        |
|-------------------|------------------------------|------------------------------|
| `description`     | `SUMMARY`                    | Primary task title           |
| `due`             | `DUE`                        | Datetime                     |
| `scheduled`       | `DTSTART`                    | Datetime                     |
| `wait`            | `X-TASKWARRIOR-WAIT`         | Custom property              |
| `end`             | `COMPLETED`                  | Set when task is completed   |
| `depends`         | `RELATED-TO;RELTYPE=DEPENDS-ON` | Task dependencies         |
| `tags`            | `CATEGORIES`                 |                              |
| `priority`        | `PRIORITY`                   |                              |
| `project`         | Calendar collection URL      | Mapped via `calendars[]` config |

### Status Mapping

| TaskWarrior Status | CalDAV Status   |
|--------------------|-----------------|
| `pending` / `waiting` | `NEEDS-ACTION` |
| `completed`        | `COMPLETED`     |
| `deleted`          | VTODO deleted   |
| `recurring`        | Skipped (warning emitted) |

## Compatibility

### Servers

| Server | Tier | Notes |
|--------|------|-------|
| Radicale | Tested | Full E2E test suite runs against Radicale. All features verified. |
| Nextcloud | Expected | XML parser handles Nextcloud namespaces. Weak ETag normalization implemented. Not E2E tested. |
| Baikal | Expected | Standard CalDAV compliance. Not E2E tested. |

### Clients

| Client | Tier | Notes |
|--------|------|-------|
| TaskWarrior 3.x | Tested | Primary sync target. Full bidirectional sync verified. |
| tasks.org + DAVx5 | Tested* | Basic VTODO sync works. See DEPENDS-ON note below. |
| Thunderbird | Expected | CalDAV VTODO support available. Not tested with caldawarrior. |

**Tiers:** *Tested* = verified with E2E test suite. *Expected* = should work based on standards compliance, not E2E tested. *Unknown* = not evaluated.

*\*DEPENDS-ON note:* caldawarrior syncs task dependencies using `RELATED-TO;RELTYPE=DEPENDS-ON` ([RFC 9253](https://datatracker.ietf.org/doc/html/rfc9253)). This property is preserved through sync by Radicale and DAVx5, but no tested CalDAV client currently renders DEPENDS-ON relationships in its UI. Dependencies work fully between TaskWarrior instances syncing through a CalDAV server. See [Known Limitation #15](#15-depends-on-relations-invisible-to-caldav-clients).

## v1 Known Limitations

The following limitations apply to the current v1 release. Each entry includes a workaround.

### 1. Recurring tasks not synced

Tasks with an `RRULE` property on the CalDAV side are detected and skipped with a warning.
TaskWarrior recurring tasks are also skipped.

**Workaround:** Manage recurring tasks directly in your CalDAV client or in TaskWarrior without
expecting them to be synced across. Remove `RRULE` from the VTODO if you want a single instance
to be synced.

---

### 2. No sync token support

caldawarrior performs a full collection list-fetch on every sync run. CalDAV sync-collection
(RFC 6578 / `sync-token`) is not implemented, so every sync fetches all VTODOs in each
configured calendar collection.

**Workaround:** Keep calendar collections small. Use `completed_cutoff_days` to limit how far
back completed and deleted tasks are exported, which reduces the volume fetched each run.

---

### 3. Task-level last-write-wins only (no field-level merge)

Conflict resolution picks an all-or-nothing winner based on modification time. There is no
field-level merging; if both sides changed different fields of the same task since the last sync,
one side's changes will overwrite the other's entirely.

**Workaround:** Avoid editing the same task on both the CalDAV side and in TaskWarrior
simultaneously between sync runs. Pick a single primary editing interface and treat the other
as read-mostly.

---

### 4. Single CalDAV server only

Only one `server_url` is supported per config file. You cannot sync to multiple CalDAV servers
from a single caldawarrior invocation.

**Workaround:** Maintain separate config files for each server and invoke caldawarrior once per
config:

```bash
caldawarrior --config ~/.config/caldawarrior/work.toml  sync
caldawarrior --config ~/.config/caldawarrior/home.toml  sync
```

---

### 5. HTTP Basic Auth only

Only HTTP Basic Authentication is supported. DIGEST auth, OAuth2, bearer tokens, and
certificate-based auth are not implemented.

**Workaround:** Most modern CalDAV servers (Nextcloud, Radicale, Baikal) support Basic Auth.
Use `CALDAWARRIOR_PASSWORD` env var or a 0600-protected config file to avoid storing the
password in plaintext in less-secure locations.

---

### 6. No keyring / secret store integration

The password is read from the config file or the `CALDAWARRIOR_PASSWORD` environment variable.
There is no integration with system keyrings (libsecret, macOS Keychain, Windows Credential
Manager).

**Workaround:** Set config file permissions to `0600`. For scripting or CI, inject the password
via `CALDAWARRIOR_PASSWORD` sourced from a secrets manager (Vault, AWS Secrets Manager, etc.)
rather than writing it to disk.

---

### 7. No concurrent sync safety

Running two caldawarrior processes simultaneously against the same TaskWarrior database can
corrupt state. TaskWarrior itself does not provide file locking for external tools.

**Workaround:** Use `flock` in cron jobs:

```bash
/usr/bin/flock -n /tmp/caldawarrior.lock caldawarrior sync
```

Or use a `systemd` timer with `RemainAfterExit=yes` to prevent overlapping runs.

---

### 8. CalDAV-only CANCELLED VTODOs are never imported

A VTODO that exists only on the CalDAV side with `STATUS:CANCELLED` is treated as a deletion
signal and never creates a TaskWarrior task.

**Workaround:** Change the VTODO's status to `NEEDS-ACTION` or `COMPLETED` in your CalDAV
client before running a sync if you want it imported as a TaskWarrior task.

---

### 9. TW task restoration hazard

If a previously synced TaskWarrior task is deleted and then un-deleted (restored) in
TaskWarrior, it retains its `caldavuid` UDA value. The next sync will re-delete the task
because caldawarrior sees a task with a `caldavuid` pointing to a VTODO that was deleted.

**Workaround:** Before restoring a deleted task, clear its `caldavuid` UDA:

```bash
task <UUID> modify caldavuid:
```

---

### 10. Orphaned caldavuid on hard deletion

If a TaskWarrior task with a `caldavuid` is hard-deleted (bypassing `task delete`) — for
example via direct JSON manipulation — the orphaned UID causes a benign skip warning on the
next sync but leaves the corresponding VTODO untouched.

**Workaround:** Always delete tasks using `task delete <UUID>` so the deletion is recorded in
TaskWarrior's undo log and picked up by the next sync.

---

### 11. Floating timestamps treated as UTC

CalDAV VTODO entries that use floating timestamps (no `Z` UTC suffix and no `TZID` parameter)
are treated as UTC with a logged warning. This can cause tasks to appear with incorrect due
dates if the CalDAV server emits local-time timestamps without timezone context.

**Workaround:** Configure your CalDAV server or client to emit UTC timestamps (`Z` suffix) or
include a `TZID` parameter. Radicale and Nextcloud both do this correctly by default.

---

### 12. No description or annotation sync

TaskWarrior annotations are not mapped to any CalDAV field. The CalDAV `DESCRIPTION` property
is not imported into TaskWarrior. Only the `SUMMARY` field (mapped to `description`) is synced.

**Workaround:** Keep extended notes in one system only. Neither side will overwrite or delete
the other's extended notes; they simply remain invisible across the sync boundary.

---

### 13. Static project-to-calendar mapping

The project ↔ calendar mapping is defined once in config. If a task's `project` attribute
changes after it has been synced, the next sync will create a new VTODO in the new calendar but
will not delete the old VTODO from the previous calendar.

**Workaround:** After changing a task's project, manually delete the old VTODO from its
original calendar collection in your CalDAV client. The new VTODO will be created automatically
on the next sync.

---

### 14. No VTODO CANCEL recovery

If a CalDAV VTODO is set to `CANCELLED` and then re-opened (status changed back to
`NEEDS-ACTION`) on the CalDAV side, caldawarrior may not re-import it into TaskWarrior because
it may have already been treated as a historical deletion.

**Workaround:** Delete the VTODO in your CalDAV client and create a fresh one, or manually
import it into TaskWarrior using `task import`. Then run a normal sync.

---

### 15. DEPENDS-ON relations invisible to CalDAV clients

Task dependencies synced via `RELATED-TO;RELTYPE=DEPENDS-ON` (RFC 9253) are preserved on the
CalDAV server but not displayed by any tested client (tasks.org, Thunderbird). Dependencies
work correctly between TaskWarrior instances syncing through the same CalDAV server.

**Workaround:** Use TaskWarrior directly to view and manage task dependencies. CalDAV clients
will show tasks individually without dependency relationships.

---

## Testing

caldawarrior has two test layers. Both require Docker Compose v2 (`docker compose`, not `docker-compose`).

### Rust Integration Tests (white-box)

Unit and integration tests written in Rust that test internal logic directly against a real Radicale server in Docker:

```bash
make test-integration
# equivalent: cargo test --test integration
```

### Robot Framework Blackbox Tests (black-box)

Behavioral end-to-end tests driven by the compiled `caldawarrior` binary. The suite is fully
self-contained — no host Rust, Python, or TaskWarrior installation required. Everything runs
inside Docker.

```bash
make test-robot
```

Results are written to `tests/robot/results/`. Open `tests/robot/results/report.html` in a
browser to see per-scenario pass/fail/skip status.

To rebuild the Docker images (e.g. after changing `Dockerfile`):

```bash
make build-robot
```

To run both suites in sequence:

```bash
make test-all
```

Scenario documentation and traceability: [`tests/robot/docs/CATALOG.md`](tests/robot/docs/CATALOG.md)

---

## v2 Roadmap

The following capabilities are planned for the v2 release:

| Feature | Description |
|---------|-------------|
| **Sync token (RFC 6578)** | Efficient incremental sync using CalDAV `sync-collection` reports; eliminates full collection fetches on every run |
| **Keyring integration** | Store passwords in the system keyring (libsecret, macOS Keychain) instead of plaintext config files |
| **DIGEST auth** | Support HTTP DIGEST authentication in addition to Basic Auth |
| **Multi-server support** | Configure and sync multiple CalDAV servers from a single config file / invocation |
| **CalDAV CANCEL recovery** | Detect when a previously CANCELLED VTODO has been re-opened and re-import it into TaskWarrior |
| **Field-level conflict merging** | Merge non-conflicting field changes from both sides rather than picking an all-or-nothing winner |
| **Annotation / DESCRIPTION sync** | Map TaskWarrior annotations to CalDAV `DESCRIPTION` and vice versa |

## License

caldawarrior is released under the MIT License. See `LICENSE` for details.
