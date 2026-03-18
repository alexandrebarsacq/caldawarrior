# Architecture

**Analysis Date:** 2026-03-18

## Pattern Overview

**Overall:** Three-Stage Sync Pipeline with Bidirectional Field Mapping

**Key Characteristics:**
- Intermediate Representation (IR) as unified reconciliation layer
- Adapter pattern for external system abstraction (TaskWarrior CLI, CalDAV HTTP)
- Last-Write-Wins (LWW) conflict resolution with two-layer decision logic
- Pluggable trait-based interfaces (`TaskRunner`, `CalDavClient`) for testability
- Timestamp-based conflict detection using `LAST-MODIFIED` (CalDAV) vs `modified` (TW)

## Layers

**Presentation (CLI/Output):**
- Purpose: Command-line interface and user-facing output formatting
- Location: `src/main.rs`, `src/output.rs`
- Contains: Argument parsing (clap), result formatting, dry-run/live mode output
- Depends on: Config, sync orchestration, types
- Used by: External users invoking `caldawarrior sync`

**Configuration:**
- Purpose: Load and validate application settings from TOML files
- Location: `src/config.rs`
- Contains: Configuration structs (`Config`, `CalendarEntry`), path resolution, permission checks
- Depends on: Error types
- Used by: Main entry point, sync orchestration

**External System Adapters:**
- Purpose: Abstract TaskWarrior and CalDAV interactions behind trait interfaces
- Location: `src/tw_adapter.rs`, `src/caldav_adapter.rs`
- Contains:
  - `TaskRunner` trait (run, import, modify) + `RealTaskRunner` (shell invocation) + `MockTaskRunner` (testing)
  - `CalDavClient` trait (list_vtodos, put_vtodo, delete_vtodo) + `RealCalDavClient` (HTTP/WebDAV) + `MockCalDavClient` (testing)
  - HTTP basic auth, ETag handling, REPORT parsing
- Depends on: Error types, types (TWTask, VTODO, FetchedVTODO)
- Used by: Sync orchestration, writeback execution

**Serialization/Format:**
- Purpose: Parse and generate iCalendar (RFC 5545) and TaskWarrior JSON
- Location: `src/ical.rs`, `src/types.rs` (tw_date, tw_date_opt, tw_depends custom serializers)
- Contains: VTODO parser (unfold lines, extract properties), VTODO builder, TW date format converters
- Depends on: chrono, serde, chrono-tz
- Used by: CalDAV adapter, sync logic, types serialization

**Intermediate Representation (IR) Construction:**
- Purpose: Pair TW tasks with CalDAV VTODOs, assign UIDs, classify entry types
- Location: `src/ir.rs`
- Contains:
  - `build_ir()` function — three-way classification (TW-only NEW, PAIRED, ORPHANED, CalDAV-only)
  - Project-to-calendar URL resolution
  - RRULE filtering (recurring VTODOs excluded with warning)
- Depends on: Config, types, error handling
- Used by: Sync orchestration

**Field Mapping:**
- Purpose: Bidirectional transformation of task fields between TW and CalDAV schemas
- Location: `src/mapper/fields.rs`, `src/mapper/status.rs`
- Contains:
  - `tw_to_caldav_fields()` — maps TW fields to `TwCalDavFields` struct (SUMMARY, DESCRIPTION, PRIORITY, DUE, DTSTART, X-TASKWARRIOR-WAIT, RELATED-TO)
  - `caldav_to_tw_fields()` — maps VTODO fields to `CalDavTwFields` struct
  - `tw_to_caldav_status()` — maps TW status to status descriptor enum (`NeedsAction`, `Completed`, `TwStateDeleted`)
  - Expired-wait collapse logic (Phase 0 finding #6)
- Depends on: Types, chrono
- Used by: Writeback execution (field assembly), status decision logic

**Dependency Resolution:**
- Purpose: Map TW dependency UUIDs to CalDAV UIDs, detect cycles
- Location: `src/sync/deps.rs`
- Contains:
  - `resolve_dependencies()` — iterative mapping + DFS cycle detection
  - Three-colour DFS (white/gray/black) to mark cyclic nodes
  - Warning generation for unresolvable/cyclic dependencies
- Depends on: Types
- Used by: Sync orchestration (step 2 of 3)

**Conflict Resolution (Last-Write-Wins):**
- Purpose: Decide which system wins when both TW and CalDAV changed
- Location: `src/sync/lww.rs`
- Contains:
  - `resolve_lww()` function (public API for paired entries)
  - Two-layer decision logic:
    1. Layer 1: Compare `LAST-MODIFIED` (CalDAV) vs `modified` (TW) timestamps
    2. Layer 2 (loop prevention): Content-identical check on 8 tracked fields
  - Timestamp normalization (second precision)
  - Status normalization (NEEDS-ACTION/IN-PROCESS → pending, COMPLETED → completed, CANCELLED → deleted)
- Depends on: Types, chrono
- Used by: Writeback execution (operation planning)

**Write-Back Execution:**
- Purpose: Execute planned operations, handle ETag conflicts, apply TW and CalDAV writes
- Location: `src/sync/writeback.rs`
- Contains:
  - `apply_writeback()` function (owns ETag retry logic up to 3 attempts)
  - VTODO construction from TW (preserve unmanaged fields via `extra_props`)
  - TW task construction from CalDAV (annotation slot invariant: slot 0 = sync, 1+ = user)
  - Operation planning (`PlannedOp` enum) and execution
  - Reverse index building (CalDAV UID → TW UUID for depends remapping)
  - ETag retry with exponential backoff
- Depends on: All other modules (adapters, mappers, types)
- Used by: Sync orchestration

**Sync Orchestration:**
- Purpose: Orchestrate the three-step pipeline and merge warnings
- Location: `src/sync/mod.rs`
- Contains: `run_sync()` function coordinating IR → dependencies → writeback
- Depends on: All sync submodules, adapters, config, types
- Used by: Main entry point

**Error Handling:**
- Purpose: Define error types with context
- Location: `src/error.rs`
- Contains: `CaldaWarriorError` enum (Config, Tw, CalDav, Auth, IcalParse, SyncConflict, EtagConflict)
- Used by: All modules

## Data Flow

**Full Sync Cycle:**

1. **Load Configuration** (`main.rs`)
   - Parse CLI args (--config, --dry-run)
   - Load TOML config, validate, check file permissions

2. **Fetch Source Data** (`main.rs`)
   - TW adapter lists all tasks via `task export`
   - CalDAV client performs REPORT query for each configured calendar
   - Results: `Vec<TWTask>` and `HashMap<String, Vec<FetchedVTODO>>`

3. **Filter Old Completed Tasks** (`sync/mod.rs`)
   - Remove TW completed/deleted tasks without `caldavuid` older than `completed_cutoff_days`
   - Purpose: Prevent old stale deletions from being pushed to CalDAV

4. **Build IR** (`ir.rs`)
   - Pair TW tasks with CalDAV VTODOs via `caldavuid` UDA
   - Three-way classify: TW-only NEW (assign UUID4), PAIRED, ORPHANED
   - Filter CalDAV: skip RRULE (recurring), assign UUID4 to NEEDS-ACTION/IN-PROCESS, ignore COMPLETED/CANCELLED
   - Resolve calendar URL for each entry from config
   - Output: `Vec<IREntry>` + warnings

5. **Resolve Dependencies** (`sync/deps.rs`)
   - Map each TW task's `depends` (UUIDs) to CalDAV UIDs via IR index
   - Detect cycles using DFS, mark cyclic entries
   - Output: Populate `entry.resolved_depends` + warnings

6. **Plan Operations** (`sync/lww.rs` + `sync/writeback.rs`)
   - For TW-only entries: plan `PushToCalDav`
   - For CalDAV-only entries: plan `PullFromCalDav`
   - For paired entries: check if content identical (loop prevention)
     - If identical: plan `Skip` (no-op)
     - If different: apply LWW via timestamp comparison
       - Winner writes to loser's system: `ResolveConflict` or `DeleteFromX`

7. **Execute Write-Back** (`sync/writeback.rs`)
   - For each planned operation:
     - TW writes: `modify` or `import` commands
     - CalDAV writes: PUT with conditional ETag (If-Match or If-None-Match)
     - ETag conflict retry (refetch + replan, up to 3 attempts)
   - Count successes and failures

8. **Print Results** (`output.rs`)
   - Dry-run mode: list each planned operation + summary count
   - Live mode: "Synced: X created, Y updated in CalDAV; A created, B updated in TW"
   - Errors and warnings to stderr
   - Exit code 1 if any errors

## Key Abstractions

**IREntry:**
- Purpose: Unified representation pairing TW + CalDAV data for a single task
- Examples: `src/types.rs` lines 263-292
- Pattern: Contains optional sides (`tw_task`, `fetched_vtodo`), classification markers (`dirty_tw`, `dirty_caldav`, `cyclic`), resolved dependencies, and calendar URL
- Used to answer "what operation should be performed on this task?"

**PlannedOp:**
- Purpose: Describes the exact action to execute for an entry
- Examples: `PushToCalDav`, `PullFromCalDav`, `ResolveConflict { winner, reason }`, `DeleteFromX`, `Skip { reason }`
- Pattern: Enums with attached data (the entry and metadata)
- Execution is independent of planning (separation of concerns)

**CalDavClient trait:**
- Purpose: Abstract CalDAV HTTP interactions; enables mocking and testing without network
- Examples: `list_vtodos()`, `put_vtodo()` (with ETag), `delete_vtodo()` (with ETag)
- Pattern: Blocking HTTP client via reqwest; trait methods return `Result<T, CaldaWarriorError>`

**TaskRunner trait:**
- Purpose: Abstract TaskWarrior CLI invocation; enables mocking and subprocess control
- Examples: `run()`, `import()`, `export()`, `modify()`
- Pattern: Wrapper around `std::process::Command`; methods map to `task <args>`

**Mapped Field Structs:**
- Purpose: Intermediate representations for field transformation during write-back
- Examples: `TwCalDavFields`, `CalDavTwFields`
- Pattern: Contain only the fields that are mapped; callers merge these into complete entities

## Entry Points

**CLI Entry Point:**
- Location: `src/main.rs` fn `main()` + `run()`
- Triggers: User invokes `caldawarrior sync [--dry-run] [--config PATH]`
- Responsibilities:
  1. Parse CLI args
  2. Load config file
  3. Create adapters (TW, CalDAV)
  4. Invoke `sync::run_sync()` orchestrator
  5. Print results via `output::print_result()`
  6. Exit with appropriate code

**Integration Tests:**
- Location: `tests/integration/mod.rs`, `test_first_sync.rs`, `test_lww.rs`, `test_scenarios.rs`
- Triggers: `cargo test --test integration` or `cargo test`
- Responsibilities: Unit + integration testing of sync logic with mocked adapters

**Robot Framework Tests:**
- Location: `tests/robot/suites/` (Robot Framework files)
- Triggers: Docker-based blackbox tests via Makefile
- Responsibilities: End-to-end testing with real TaskWarrior and Radicale instances

## Error Handling

**Strategy:** Result-based error propagation with context via `anyhow::Context`

**Patterns:**
- All fallible operations return `Result<T, CaldaWarriorError>` or `Result<T, anyhow::Error>`
- Config loading errors: `CaldaWarriorError::Config` with descriptive message
- TaskWarrior failures: `CaldaWarriorError::Tw { code, stderr }` captures exit code and output
- CalDAV failures: `CaldaWarriorError::CalDav { status, body }` captures HTTP status and response body
- Authentication failures: Special `CaldaWarriorError::Auth` variant guides user to check credentials
- ETag conflicts: `CaldaWarriorError::EtagConflict { refetched_vtodo }` carries the refetched data for retry
- iCalendar parse errors: `CaldaWarriorError::IcalParse` with error context

**Warnings (non-fatal):**
- Accumulated in `Vec<Warning>` during IR construction, dependency resolution, and writeback
- Each warning carries optional `tw_uuid` for identification
- Printed to stderr with `[WARN]` prefix
- Include: `UnmappedProject`, `RecurringCalDavSkipped`, `UnresolvableDependency`, `CyclicEntry`

## Cross-Cutting Concerns

**Logging:**
- No structured logging framework; uses `eprintln!()` and `println!()` for output
- `[ERROR]` prefix for errors (stderr)
- `[WARN]` prefix for warnings (stderr)
- `[DRY-RUN]` prefix for planned operations (stdout)
- Sync summary to stdout in live mode

**Validation:**
- Config file validation in `config::load()`: checks for duplicate calendar URLs, required fields
- Field mapping validation in mappers: rejects invalid priority letters, handles date parsing errors
- TaskWarrior command validation: captures exit codes and stderr
- CalDAV HTTP validation: checks status codes, parses error responses

**Authentication:**
- Basic auth for CalDAV via `username` and `password` from config
- Password override via `CALDAWARRIOR_PASSWORD` env var (for CI/scripting)
- File permission check on config file (0600 recommended, warns if more permissive)

**Timestamps:**
- All times normalized to UTC via `chrono::Utc`
- Serialization: TW uses compact format `YYYYMMDDTHHMMSSZ`; CalDAV uses RFC 5545 format
- Comparison: Second-level precision for conflict detection (subsecond ignored)

**Dependency Management:**
- TaskWarrior dependencies are UUIDs; CalDAV uses `RELATED-TO` with UID strings
- Bidirectional mapping via IR reverse index
- Cycle detection prevents infinite loops in dependency graph
- Unresolvable dependencies reported as warnings, not fatal errors

---

*Architecture analysis: 2026-03-18*
