# Codebase Structure

**Analysis Date:** 2026-03-18

## Directory Layout

```
caldawarrior/
├── Cargo.toml              # Rust project manifest; edition 2024, dependencies (serde, reqwest, chrono, clap, toml, thiserror, anyhow)
├── Cargo.lock              # Locked dependency versions
├── CLAUDE.md               # Project workflow instructions (uses foundry-spec/implement/review skills)
├── README.md               # User-facing documentation (features, field mapping, quick start)
├── Makefile                # Test targets (cargo test, make coverage, etc.)
├── src/                    # Main source code
│   ├── main.rs             # CLI entry point (130 lines); parses args, loads config, orchestrates sync, prints results
│   ├── lib.rs              # Crate root; exports all public modules
│   ├── types.rs            # Core domain types (450 lines): TWTask, VTODO, IREntry, FetchedVTODO, PlannedOp, SyncResult, custom serde modules
│   ├── error.rs            # Error enum (86 lines): CaldaWarriorError variants with Display implementations
│   ├── config.rs           # Configuration loading (232 lines): TOML parsing, path resolution, permission checks, validation
│   ├── output.rs           # Result formatting (632 lines): dry-run operation listing, summary counts, stderr warnings
│   ├── ir.rs               # IR construction (482 lines): build_ir() — TW/CalDAV pairing, three-way classification, calendar URL resolution
│   ├── ical.rs             # iCalendar parser/builder (754 lines): from_icalendar_string(), to_icalendar_string(), RFC 5545 compliance
│   ├── tw_adapter.rs       # TaskWarrior interface (484 lines): TaskRunner trait, RealTaskRunner (shell), MockTaskRunner (testing)
│   ├── caldav_adapter.rs   # CalDAV interface (783 lines): CalDavClient trait, RealCalDavClient (HTTP/WebDAV), MockCalDavClient (testing), REPORT query
│   ├── mapper/             # Field mapping logic
│   │   ├── mod.rs          # Module exports
│   │   ├── fields.rs       # Field transformation (551 lines): TwCalDavFields, CalDavTwFields, bidirectional mappers, expired-wait collapse
│   │   └── status.rs       # Status mapping (182 lines): TwToCalDavStatus enum, pending/completed/deleted/recurring/waiting mappings
│   └── sync/               # Sync orchestration and execution (7085 lines total for src/)
│       ├── mod.rs          # Orchestrator (270 lines): run_sync() — IR → dependencies → writeback pipeline
│       ├── deps.rs         # Dependency resolution (288 lines): resolve_dependencies() — UUID→UID mapping, DFS cycle detection
│       ├── lww.rs          # Conflict resolution (599 lines): resolve_lww() — two-layer LWW decision logic, timestamp comparison, content-identical check
│       └── writeback.rs    # Operation execution (1139 lines): apply_writeback() — plan operations, execute writes, ETag retry logic, TW/CalDAV command building
├── tests/                  # Test suites
│   ├── integration/        # Rust integration tests
│   │   ├── mod.rs          # Common test utilities (31 KB): fixtures, helpers, mocked adapters
│   │   ├── test_first_sync.rs    # First sync scenarios (2.5 KB)
│   │   ├── test_lww.rs           # LWW conflict tests (6.7 KB)
│   │   ├── test_scenarios.rs     # Multi-entry sync scenarios (8.1 KB)
│   │   ├── docker-compose.yml    # Integration test stack (TW + Radicale)
│   │   ├── Dockerfile.taskwarrior # TW container image
│   │   ├── radicale.config       # CalDAV server config
│   │   ├── htpasswd              # Basic auth credentials for tests
│   │   └── tw-behavior-research.sh # TW CLI exploration script
│   └── robot/              # Robot Framework blackbox tests
│       ├── suites/         # RF test suites (26 passed, 5 skipped scenarios)
│       ├── resources/      # RF keywords and helpers
│       ├── docs/           # RF documentation
│       └── docker-compose.yml # RF test stack
├── docs/                   # Architecture Decision Records (ADR)
│   └── adr/                # ADR files documenting design decisions
├── specs/                  # Foundry spec tracking
│   ├── active/             # Active spec files
│   ├── pending/            # Pending specs
│   ├── completed/          # Completed specs (3 completed as of 2026-03-18)
│   └── archived/           # Archived specs
├── .planning/              # GSD planning documents (generated)
│   └── codebase/           # Codebase analysis documents (ARCHITECTURE.md, STRUCTURE.md, etc.)
└── target/                 # Cargo build artifacts (not committed)
```

## Directory Purposes

**src/**
- Purpose: All Rust source code (library + binary)
- Contains: Module definitions, implementations, tests
- Key files: `lib.rs` (crate root), `main.rs` (binary entry point)

**src/sync/**
- Purpose: Sync pipeline orchestration and execution
- Contains: Three-stage sync logic (IR construction, dependency resolution, write-back)
- Key files: `mod.rs` (orchestrator), `lww.rs` (conflict resolution), `writeback.rs` (execution)

**src/mapper/**
- Purpose: Bidirectional field mapping between TaskWarrior and CalDAV schemas
- Contains: Field transformation logic, status mapping
- Key files: `fields.rs` (field mappers), `status.rs` (status enum and mappings)

**tests/integration/**
- Purpose: Rust integration tests with mocked adapters
- Contains: Test scenarios, fixtures, helper functions
- Key files: `mod.rs` (common utilities), `test_*.rs` (individual test suites)

**tests/robot/**
- Purpose: Robot Framework blackbox tests (end-to-end with real services)
- Contains: RF test suites, keywords, resources
- Key files: `suites/` (RF test files), `docker-compose.yml` (test infrastructure)

**docs/adr/**
- Purpose: Architecture Decision Records documenting major design choices
- Contains: ADR files (one per decision)

**specs/**
- Purpose: Foundry spec tracking for features and fixes
- Contains: Active, pending, completed, and archived specs
- Key files: Autonomy context and index for GSD orchestrator

**.planning/codebase/**
- Purpose: GSD codebase analysis documents (generated by mappers)
- Contains: ARCHITECTURE.md, STRUCTURE.md, CONVENTIONS.md, TESTING.md, CONCERNS.md, STACK.md, INTEGRATIONS.md

## Key File Locations

**Entry Points:**
- `src/main.rs`: CLI entry point; handles argument parsing, config loading, adapter initialization, orchestration invocation, result output
- `tests/integration/mod.rs`: Integration test entry point for Rust tests
- `tests/robot/docker-compose.yml`: Blackbox test infrastructure

**Configuration:**
- `Cargo.toml`: Rust dependencies and binary definition
- `src/config.rs`: TOML configuration loading and validation
- `tests/robot/radicale.config`: CalDAV server configuration for testing
- `tests/robot/docker-compose.yml`: Test infrastructure (TW + Radicale containers)

**Core Logic:**
- `src/sync/mod.rs`: Three-stage pipeline orchestrator (IR → dependencies → writeback)
- `src/sync/lww.rs`: Last-Write-Wins conflict resolution (timestamp comparison, content-identical check)
- `src/sync/writeback.rs`: Operation planning and execution (TW/CalDAV writes, ETag retry)
- `src/ir.rs`: Intermediate representation construction (TW/CalDAV pairing, classification)
- `src/mapper/fields.rs`: Bidirectional field transformation (TW ↔ CalDAV)

**Adapters:**
- `src/tw_adapter.rs`: TaskWarrior CLI abstraction (trait + real + mock implementations)
- `src/caldav_adapter.rs`: CalDAV HTTP abstraction (trait + real + mock implementations)
- `src/ical.rs`: iCalendar RFC 5545 parser and builder

**Output and Formatting:**
- `src/output.rs`: Result formatting (dry-run operation listing, summary text, warnings)
- `src/error.rs`: Error type definitions with Display implementations

**Types:**
- `src/types.rs`: Core domain types (TWTask, VTODO, IREntry, PlannedOp, SyncResult, etc.)
  - Custom serde modules: `tw_date`, `tw_date_opt`, `tw_depends` for TW format handling

## Naming Conventions

**Files:**
- Rust modules: snake_case (e.g., `tw_adapter.rs`, `caldav_adapter.rs`)
- Test files: `test_*.rs` prefix (e.g., `test_first_sync.rs`)
- Subdirectories: lowercase, plural for collections (e.g., `src/mapper/`, `src/sync/`)

**Directories:**
- Logical grouping by concern (sync, mapper, tests)
- No "utils" or "helpers" directories (utilities colocated with their users)

**Functions:**
- snake_case throughout (e.g., `build_ir()`, `resolve_dependencies()`, `apply_writeback()`)
- Module-level functions are public (`pub fn`); private helpers use `fn`
- Test functions: `#[test]` attribute with descriptive names (e.g., `full_sync_tw_only_pushes_to_caldav`)

**Types:**
- PascalCase for structs and enums (e.g., `TWTask`, `VTODO`, `IREntry`, `PlannedOp`)
- Enums use PascalCase variants (e.g., `PlannedOp::PushToCalDav`, `Side::Tw`)
- Custom serde modules: snake_case (e.g., `tw_date`, `tw_depends`)

**Constants:**
- SCREAMING_SNAKE_CASE (e.g., `MAX_ETAG_RETRIES`, `WAIT_PROP`, `DEFAULT_PROJECT`)

**Traits:**
- PascalCase (e.g., `TaskRunner`, `CalDavClient`)
- Implementations: `Real<Trait>` for real impl, `Mock<Trait>` for test mock (e.g., `RealTaskRunner`, `MockTaskRunner`)

## Where to Add New Code

**New Feature (e.g., support for recurring tasks):**
- Primary code: `src/sync/lww.rs` (LWW decision logic) or `src/mapper/fields.rs` (field mapping)
- Tests: `tests/integration/test_scenarios.rs` (unit tests) + Robot Framework suite (blackbox tests)
- Docs: `docs/adr/` (ADR if major decision), `README.md` (user-facing)

**New Adapter (e.g., support for different CalDAV server):**
- Implementation: Create new type implementing `CalDavClient` trait in `src/caldav_adapter.rs` or new module
- Tests: Add mock implementation and integration tests in `tests/integration/`
- Config: Extend `src/config.rs` if new configuration needed

**New Field Mapper (e.g., support for custom TW UDA):**
- Implementation: `src/mapper/fields.rs` — add new field to `TwCalDavFields` or `CalDavTwFields`, implement `tw_to_caldav_fields()` or `caldav_to_tw_fields()` logic
- Tests: `tests/integration/test_scenarios.rs` or new `test_mapper.rs`

**New CLI Subcommand (e.g., `caldawarrior verify`):**
- CLI parsing: `src/main.rs` — add variant to `Commands` enum
- Implementation: New module `src/cmd_verify.rs` (or similar)
- Tests: `tests/integration/mod.rs`

**Utilities:**
- Shared helpers: Colocate in the module that uses them (e.g., if multiple mapper functions need a helper, define it in `mapper/fields.rs`)
- Cross-cutting: `src/types.rs` (for types) or new module if substantial (e.g., `src/timestamp_utils.rs`)

## Special Directories

**src/sync/**
- Purpose: Sync pipeline implementation (separate from other concerns)
- Generated: No
- Committed: Yes
- Structure: One file per major stage (`deps.rs`, `lww.rs`, `writeback.rs`) + orchestrator (`mod.rs`)

**tests/integration/**
- Purpose: Rust integration tests with controlled mocking
- Generated: No (but may contain generated test data)
- Committed: Yes
- Docker stack: `docker-compose.yml` defines TW + Radicale for integration testing

**tests/robot/**
- Purpose: Robot Framework blackbox tests (real TW and CalDAV services)
- Generated: Yes (`results/` contains RF output artifacts)
- Committed: Test code yes, results/ no
- Docker stack: `docker-compose.yml` for RF test execution

**docs/adr/**
- Purpose: Architecture Decision Records (immutable history)
- Generated: No
- Committed: Yes
- Format: ADR-NNN.md files documenting decisions and their rationale

**specs/**
- Purpose: Foundry spec tracking (feature specifications and task management)
- Generated: Yes (Autonomy metadata in `.autonomy/` subdirectories)
- Committed: Spec files yes, generated metadata conditional
- Subdirs: `active/`, `pending/`, `completed/`, `archived/` for workflow state

**.planning/codebase/**
- Purpose: GSD codebase analysis documents (consumed by /gsd:plan-phase and /gsd:execute-phase)
- Generated: Yes (written by codebase mapper)
- Committed: Yes
- Files: ARCHITECTURE.md, STRUCTURE.md, CONVENTIONS.md, TESTING.md, CONCERNS.md, STACK.md, INTEGRATIONS.md

**target/**
- Purpose: Cargo build artifacts
- Generated: Yes (by `cargo build`, `cargo test`)
- Committed: No (in .gitignore)

---

*Structure analysis: 2026-03-18*
