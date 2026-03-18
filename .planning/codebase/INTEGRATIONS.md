# External Integrations

**Analysis Date:** 2026-03-18

## APIs & External Services

**CalDAV (WebDAV-based):**
- CalDAV servers (Nextcloud, Radicale, FastMail, iCloud, Baikal, etc.)
  - SDK/Client: `reqwest::blocking::Client` with HTTP Basic Auth
  - Operations: PROPFIND/REPORT (list VTODOs), GET/PUT/DELETE (individual VTODOs)
  - Auth: HTTP Basic Auth (username + password from config)
  - Protocol: WebDAV methods (REPORT, PUT, DELETE) + standard HTTP (GET, POST)
  - Location: `src/caldav_adapter.rs` (trait `CalDavClient` implemented by `RealCalDavClient`)

**TaskWarrior CLI:**
- TaskWarrior 3.x binary (`task` command)
  - SDK/Client: Direct `std::process::Command` invocation
  - Operations: `task export`, `task import`, `task modify`, `task delete`, custom UDA registration
  - Auth: File system access to `~/.local/share/task/` database
  - Invocation: subprocess execution with JSON stdio for import, text parsing for export
  - Location: `src/tw_adapter.rs` (trait `TaskRunner` implemented by `RealTaskRunner`)

## Data Storage

**Databases:**
- TaskWarrior local database
  - Type: JSON-based file system storage
  - Connection: Local file system access (no network)
  - Client: TaskWarrior binary via CLI
  - Location: `~/.local/share/task/` (default)
  - Format: JSON (.data/ directory + undo/ log)

- CalDAV server (remote)
  - Type: iCalendar (RFC 5545) VTODO components
  - Connection: HTTP/HTTPS with Basic Auth
  - Client: `reqwest::blocking::Client`
  - Format: VCALENDAR containing VTODO items
  - Server URL: Configured in `~/.config/caldawarrior/config.toml`

**File Storage:**
- Local filesystem only (no external blob storage)
- Config file: `~/.config/caldawarrior/config.toml` (must have 0600 permissions)
- Temp storage: Uses standard OS temp directory (via `tempfile` crate in tests)

**Caching:**
- None (stateless sync on each invocation)

## Authentication & Identity

**Auth Provider:**
- Custom HTTP Basic Auth
  - Implementation: Built-in `reqwest::blocking::Client` with `.basic_auth(username, password)`
  - Credentials source: TOML config file or `CALDAWARRIOR_PASSWORD` env var override
  - TLS: Strict by default (rustls); optional insecure mode via `allow_insecure_tls` config flag

**CalDAV Task Identity:**
- TaskWarrior User Defined Attribute (UDA) `caldavuid` - tracks the CalDAV UID of paired tasks
- Registration: One-time `task config uda.caldavuid.type string`
- Purpose: Enables bidirectional sync by matching TW task to VTODO by UID

## Monitoring & Observability

**Error Tracking:**
- None (no external error tracking service)
- Error handling: `anyhow::Result` with context chains via `.context()` and `.with_context()`
- Error types: `CaldaWarriorError` enum with variants for Config, Tw, CalDAV, Auth, IcalParse, SyncConflict, EtagConflict
- Location: `src/error.rs`

**Logs:**
- stdout/stderr only (no external logging service)
- Approach: Direct eprintln!() for errors/warnings to stderr, structured output module in `src/output.rs`
- Warnings: Config file permission checks, RRULE skips, sync operations (create/update/delete counts)
- Location: `src/output.rs` (formats sync results for human consumption)

## CI/CD & Deployment

**Hosting:**
- Self-hosted CLI binary
- Distribution: GitHub releases or `cargo install --path .`
- No cloud platform dependency

**CI Pipeline:**
- GitHub Actions (inferred from test setup, not yet examined)
- Test runners:
  - Cargo test suite: `cargo test --test integration`
  - Robot Framework suite: Docker Compose-based (see Makefile)
- Makefile targets: `test-integration`, `test-robot`, `test-all`, `build-robot`, `help`
- Location: `Makefile`

## Environment Configuration

**Required env vars:**
- `CALDAWARRIOR_PASSWORD` - Override config file password (optional, highest precedence)
- `CALDAWARRIOR_CONFIG` - Override default config path (optional)
- `HOME` - User home directory (required for default config location)
- `TZ` - Timezone (used in test containers; chrono-tz handles TZID parameters)

**Secrets location:**
- Config file: `~/.config/caldawarrior/config.toml`
- Permissions: Must be 0600 (owner read+write only); runtime warning if more permissive
- Alternative: Env var `CALDAWARRIOR_PASSWORD` (sourced from external secret manager in CI)
- Warning issued at startup if permissions exceed 0600 on Unix

## Webhooks & Callbacks

**Incoming:**
- None (stateless CLI tool; no server component)

**Outgoing:**
- None (direct HTTP requests only; no webhook push)

## CalDAV Specifics

**RFC Compliance:**
- RFC 5545 - iCalendar VTODO components (parsing and serialization in `src/ical.rs`)
- RFC 4918 - WebDAV HTTP methods (PROPFIND, REPORT, PUT, DELETE)
- RFC 6578 - CalDAV sync-token NOT implemented (full collection fetch on every sync)
- HTTP Basic Auth (RFC 7617)

**VTODO Field Mapping:**
- TaskWarrior ↔ CalDAV field pairs (see README.md for full mapping)
- SUMMARY ← description
- DTSTART ← scheduled
- DUE ← due
- COMPLETED ← end (when task completed)
- CATEGORIES ← tags
- PRIORITY ← priority (numeric: 1=high, 5=medium, 9=low)
- RELATED-TO;RELTYPE=DEPENDS-ON ← depends (task dependencies)
- X-TASKWARRIOR-WAIT ← wait (custom property)
- DESCRIPTION ← annotations (newline-joined)
- STATUS → NEEDS-ACTION / COMPLETED / CANCELLED / IN-PROCESS

**CalDAV Server Compatibility:**
- Tested: Radicale 3.3, Nextcloud, FastMail, iCloud, Baikal
- Requirement: Support for REPORT method (RFC 4918) with calendar-query
- TLS: Strict by default; insecure mode available for self-signed certs

## Third-Party Service Dependencies

**None** - caldawarrior has zero external API dependencies beyond CalDAV and TaskWarrior.

---

*Integration audit: 2026-03-18*
