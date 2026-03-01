# caldawarrior Configuration Reference

This document describes every configuration option accepted by caldawarrior, the environment
variables that influence its behaviour, and the CLI flags available at runtime.

## Table of Contents

- [Config File Location](#config-file-location)
- [Config File Format](#config-file-format)
- [Top-Level Fields](#top-level-fields)
  - [server_url](#server_url)
  - [username](#username)
  - [password](#password)
  - [completed_cutoff_days](#completed_cutoff_days)
  - [allow_insecure_tls](#allow_insecure_tls)
  - [caldav_timeout_seconds](#caldav_timeout_seconds)
- [Calendar Entries (`[[calendar]]`)](#calendar-entries-calendar)
- [Complete Example Config](#complete-example-config)
- [Environment Variables](#environment-variables)
  - [CALDAWARRIOR_PASSWORD](#caldawarrior_password)
  - [CALDAWARRIOR_CONFIG](#caldawarrior_config)
  - [HOME](#home)
- [CLI Flags](#cli-flags)
- [Security Considerations](#security-considerations)

---

## Config File Location

caldawarrior resolves the config file path in the following priority order:

1. `--config PATH` CLI flag (highest priority)
2. `CALDAWARRIOR_CONFIG` environment variable
3. `~/.config/caldawarrior/config.toml` (default)

If none of the above resolves to a readable file, caldawarrior exits with an error.

---

## Config File Format

The config file uses [TOML](https://toml.io/) syntax. Required fields must be present;
optional fields fall back to their documented defaults if omitted.

---

## Top-Level Fields

### `server_url`

| Attribute | Value |
|-----------|-------|
| Type      | `String` (URL) |
| Required  | Yes |
| Default   | — |

The base URL of the CalDAV server. This is used to construct the HTTP client and for
informational purposes; the actual calendar collection URLs are specified individually in
`[[calendar]]` entries.

```toml
server_url = "https://dav.example.com"
```

Include the scheme (`https://` or `http://`). Do not include a trailing path component that
belongs to a specific collection — use `[[calendar]]` entries for that.

---

### `username`

| Attribute | Value |
|-----------|-------|
| Type      | `String` |
| Required  | Yes |
| Default   | — |

The username for HTTP Basic Authentication against the CalDAV server.

```toml
username = "alice"
```

---

### `password`

| Attribute | Value |
|-----------|-------|
| Type      | `String` |
| Required  | Yes (unless overridden by `CALDAWARRIOR_PASSWORD`) |
| Default   | — |

The password for HTTP Basic Authentication. This value is used as-is unless the
`CALDAWARRIOR_PASSWORD` environment variable is set and non-empty, in which case the
environment variable takes precedence.

```toml
password = "hunter2"
```

**Security note:** Restrict the config file to mode `0600` on Unix so that only the owning
user can read the password. caldawarrior emits a `[WARN]` at startup if the file permissions
are more permissive than `0600`.

To avoid storing the password on disk at all, omit it from the config file and set
`CALDAWARRIOR_PASSWORD` at runtime (see [Environment Variables](#environment-variables)).
The field must still be present in the config file in that case (use an empty string); the
environment variable will override it.

---

### `completed_cutoff_days`

| Attribute | Value |
|-----------|-------|
| Type      | `u32` (unsigned 32-bit integer) |
| Required  | No |
| Default   | `90` |

Limits how far back (in days) completed and deleted tasks are considered during export from
TaskWarrior to CalDAV. Tasks completed or deleted more than this many days ago are excluded
from the sync window, which reduces the number of VTODOs fetched and updated on each run.

```toml
completed_cutoff_days = 30
```

Increase this value if you need to keep a longer history visible on your CalDAV server.
Set it to a very large number (e.g. `36500`) to include all completed tasks regardless of age.

---

### `allow_insecure_tls`

| Attribute | Value |
|-----------|-------|
| Type      | `bool` |
| Required  | No |
| Default   | `false` |

When set to `true`, TLS certificate verification is disabled. This allows caldawarrior to
connect to CalDAV servers that present self-signed or otherwise untrusted certificates.

```toml
allow_insecure_tls = true
```

**Warning:** Disabling TLS verification exposes your credentials and task data to
man-in-the-middle attacks. Only use this option on private, trusted networks (e.g., a local
Radicale instance on localhost or a home LAN). Do not use it against internet-facing servers.

By default (`false`), caldawarrior uses `rustls` with the system certificate store and rejects
any server certificate that cannot be verified.

---

### `caldav_timeout_seconds`

| Attribute | Value |
|-----------|-------|
| Type      | `u64` (unsigned 64-bit integer, seconds) |
| Required  | No |
| Default   | `30` |

The HTTP request timeout in seconds. Each individual HTTP request to the CalDAV server
(PROPFIND, GET, PUT, DELETE) must complete within this many seconds, or it is aborted and
treated as an error.

```toml
caldav_timeout_seconds = 60
```

Increase this value if your CalDAV server is slow to respond (e.g., a low-powered home server
or a server behind a high-latency network link). Decrease it if you want sync failures to be
detected quickly.

---

## Calendar Entries (`[[calendar]]`)

Each `[[calendar]]` entry maps a TaskWarrior project to a CalDAV calendar collection URL.
You may define zero or more entries. Tasks that do not match any named project entry are
matched against the `"default"` entry if one exists.

Each entry has two fields:

### `project`

| Attribute | Value |
|-----------|-------|
| Type      | `String` |
| Required  | Yes (per entry) |
| Default   | — |

The TaskWarrior project name. Use the special value `"default"` to capture tasks that have no
project or whose project does not match any other entry.

### `url`

| Attribute | Value |
|-----------|-------|
| Type      | `String` (URL) |
| Required  | Yes (per entry) |
| Default   | — |

The full URL of the CalDAV calendar collection for this project. This must be a collection URL
(ending in `/` is conventional but not required by caldawarrior), not a principal or
discovery URL.

**Validation:** caldawarrior will refuse to start if two non-`"default"` entries share the
same `url` (duplicate calendar URL error). Multiple entries may all point to the same URL only
if they are all `"default"` project entries (which is an unusual configuration).

```toml
[[calendar]]
project = "work"
url     = "https://dav.example.com/alice/work/"

[[calendar]]
project = "personal"
url     = "https://dav.example.com/alice/personal/"

[[calendar]]
project = "default"
url     = "https://dav.example.com/alice/inbox/"
```

---

## Complete Example Config

The following is a complete example demonstrating all available fields:

```toml
# CalDAV server base URL (required)
server_url = "https://dav.example.com"

# HTTP Basic Auth credentials (required)
username = "alice"
password = "hunter2"   # Override at runtime with CALDAWARRIOR_PASSWORD env var

# Only export completed/deleted tasks from the last 60 days (default: 90)
completed_cutoff_days = 60

# Set to true only for self-signed / untrusted certs on private networks (default: false)
allow_insecure_tls = false

# Abort HTTP requests after 45 seconds (default: 30)
caldav_timeout_seconds = 45

# Map the "work" TW project to a dedicated calendar collection
[[calendar]]
project = "work"
url     = "https://dav.example.com/alice/work/"

# Map the "personal" TW project to another collection
[[calendar]]
project = "personal"
url     = "https://dav.example.com/alice/personal/"

# Catch-all: tasks with no project (or unknown project) go here
[[calendar]]
project = "default"
url     = "https://dav.example.com/alice/inbox/"
```

Minimal config (only required fields, one calendar):

```toml
server_url = "https://dav.example.com"
username   = "alice"
password   = "hunter2"

[[calendar]]
project = "default"
url     = "https://dav.example.com/alice/tasks/"
```

---

## Environment Variables

### `CALDAWARRIOR_PASSWORD`

When set to a non-empty string, this variable overrides the `password` field in the config
file. The override happens after the config file is parsed and before validation, so the
`password` field in the config file is still required to be present (it can be an empty string
or a placeholder).

**Use cases:**

- CI/CD pipelines where the password is injected as a secret
- Scripting environments where writing a password to disk is not acceptable
- Systems managed by a secrets manager (Vault, AWS Secrets Manager, 1Password CLI, etc.)

**Example (bash):**

```bash
export CALDAWARRIOR_PASSWORD="$(vault kv get -field=password secret/caldav)"
caldawarrior sync
```

**Example (systemd service with credentials):**

```ini
[Service]
LoadCredential=caldav-password:/run/secrets/caldav-password
ExecStartPre=/bin/sh -c 'export CALDAWARRIOR_PASSWORD=$(cat ${CREDENTIALS_DIRECTORY}/caldav-password)'
ExecStart=caldawarrior sync
```

---

### `CALDAWARRIOR_CONFIG`

When set, this variable specifies the path to the config file. It is checked after the
`--config` CLI flag and before the default path (`~/.config/caldawarrior/config.toml`).

**Use cases:**

- Running multiple instances with different configurations without using CLI flags in every
  invocation
- Pointing to a config file in a non-standard location from a wrapper script
- Testing with a temporary config file

**Example:**

```bash
CALDAWARRIOR_CONFIG=/etc/caldawarrior/work.toml caldawarrior sync
```

---

### `HOME`

Used only to resolve the default config path (`~/.config/caldawarrior/config.toml`). If `HOME`
is not set and no config path is provided via `--config` or `CALDAWARRIOR_CONFIG`, caldawarrior
exits with an error.

This variable is set automatically by the shell and system on all standard Unix environments;
you do not normally need to set it explicitly.

---

## CLI Flags

```
caldawarrior [OPTIONS] <SUBCOMMAND>

OPTIONS:
  --config <PATH>
      Path to the configuration file.
      Overrides CALDAWARRIOR_CONFIG env var and the default path.

SUBCOMMANDS:
  sync
      Synchronise TaskWarrior tasks with CalDAV.

      OPTIONS:
        --dry-run
            Preview all changes (creates, updates, deletes) without writing
            anything to TaskWarrior or CalDAV. Prints a summary of what would
            happen. Exit code is 0 unless an error was encountered during the
            preview itself.
```

---

## Security Considerations

### Config file permissions

The config file contains your CalDAV password in plaintext. Restrict read access to the owning
user:

```bash
chmod 0600 ~/.config/caldawarrior/config.toml
```

caldawarrior checks file permissions at startup on Unix systems and prints a `[WARN]` message
to stderr if the file is readable by group or world (permissions more permissive than `0600`).
This warning is non-fatal; sync proceeds regardless.

### TLS

TLS verification is enabled by default using `rustls`. Only set `allow_insecure_tls = true`
on private, trusted networks. Never use it against internet-facing CalDAV servers.

### Password storage alternatives

Prefer `CALDAWARRIOR_PASSWORD` over storing the password in the config file when the config
file might be read by other processes, backed up to cloud storage, or committed to version
control. Inject the variable from a secrets manager rather than hardcoding it in shell
profiles.
