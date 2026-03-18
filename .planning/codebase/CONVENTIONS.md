# Coding Conventions

**Analysis Date:** 2026-03-18

## Naming Patterns

**Files:**
- Rust module files use `snake_case`: `src/tw_adapter.rs`, `src/caldav_adapter.rs`, `src/sync/lww.rs`
- Module subdir names use `snake_case`: `src/mapper/`, `src/sync/`
- Test files follow naming: `tests/integration/test_first_sync.rs`, `tests/robot/suites/01_first_sync.robot`

**Functions:**
- Rust functions use `snake_case`: `fn list_all()`, `fn resolve_calendar_url()`, `fn build_ir()`
- Public functions in traits are concise and direct: `fn run()`, `fn import()`, `fn export()`
- Helper functions (private) use descriptive `snake_case`: `fn resolve_path()`, `fn check_permissions()`, `fn normalize_status()`

**Variables:**
- Immutable bindings use `snake_case`: `let tw_tasks`, `let caldav_map`, `let config`
- Constants use `SCREAMING_SNAKE_CASE`: `const FMT: &str`, `const WAIT_PROP: &str`, `const DEFAULT_PROJECT: &str`
- Loop counters use short lowercase: `let mut caldav_creates = 0usize`

**Types:**
- Structs use `PascalCase`: `struct TWTask`, `struct VTODO`, `struct IREntry`, `struct CalendarEntry`
- Enums use `PascalCase`: `enum Commands`, `enum CaldaWarriorError`, `enum PlannedOp`
- Trait names use `PascalCase`: `trait CalDavClient`, `trait TaskRunner`
- Type modules use `snake_case`: `pub mod tw_date`, `pub mod tw_date_opt`, `pub mod tw_depends`

## Code Style

**Formatting:**
- No explicit formatter configured (no rustfmt.toml detected)
- Project uses Rust edition 2024 (see `Cargo.toml`): `edition = "2024"`
- Standard Rust conventions apply: 4-space indentation, no tabs

**Linting:**
- No explicit clippy config detected
- Code follows Rust idioms: uses `Result<T>` for fallible operations, `?` operator for error propagation
- `thiserror` used for custom error types with derived Display
- Error types are comprehensive: `CaldaWarriorError` enum covers Config, TaskWarrior, CalDAV, Auth, IcalParse, SyncConflict, EtagConflict

## Import Organization

**Order:**
1. Standard library imports: `use std::...`
2. External crate imports: `use chrono::...`, `use serde::...`, `use uuid::...`
3. Internal module imports: `use crate::...`
4. Type-specific imports for traits: `use crate::caldav_adapter::CalDavClient`

**Path Aliases:**
- No path aliases configured (no workspace root alias)
- Absolute imports use `crate::` prefix: `use crate::config`, `use crate::types::TWTask`
- No shorthand imports like `use super::*` (avoid glob imports)

**Example patterns from `src/main.rs`:**
```rust
use anyhow::{Context, Result};
use caldawarrior::caldav_adapter::{CalDavClient, RealCalDavClient};
use caldawarrior::tw_adapter::{RealTaskRunner, TwAdapter};
use caldawarrior::{config, output, sync};
use chrono::Utc;
use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process;
```

## Error Handling

**Patterns:**
- Primary error type is `anyhow::Result<T>` for top-level operations (main, sync entry point)
- Custom error type `CaldaWarriorError` used for domain-specific errors (see `src/error.rs`)
- Both error types support `.context()` and `.with_context()` for error chain annotation
- Errors propagate with `?` operator; explicit `Result<T, E>` return types only in specialized contexts

**Error context example from `src/main.rs`:**
```rust
let config = config::load(cli.config.as_deref())
    .context("Failed to load configuration")?;
```

**Process exit handling:**
```rust
fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {:#}", e);
        process::exit(1);
    }
}
```

**Caller responsibility:** Callers choose whether to treat an error as fatal. In `tw_adapter.rs`, deletion errors exit code 1 ("not deletable") is documented as acceptable for already-deleted tasks; caller decides context.

## Logging

**Framework:** No structured logging framework used. Direct `println!()` and `eprintln!()` macros only.

**Patterns:**
- `eprintln!()` for errors: `eprintln!("Error: {:#}", e)`
- `eprintln!()` for warnings: `eprintln!("{} {}", prefix, warn.message)`
- `println!()` for normal output and summaries: `println!("Synced: {} created...")`
- Errors and warnings are prefixed: `[ERROR]`, `[WARN]`, `[WARN] [<uuid>]` (in `src/output.rs`)
- Dry-run operations prefixed: `[DRY-RUN] [CREATE]`, `[DRY-RUN] [UPDATE]`, `[DRY-RUN] [DELETE]`, `[DRY-RUN] [SKIP]`

**Output formatting in `src/output.rs`:**
- Errors go to stderr
- Warnings go to stderr
- Dry-run operations and summaries go to stdout
- Live-mode summary: `"Synced: {} created, {} updated in CalDAV; {} created, {} updated in TW"`

## Comments

**When to Comment:**
- Document the WHY, not the WHAT (code should be self-documenting)
- Complex algorithms get narrative comments explaining intent
- Non-obvious design decisions (e.g., timestamp precision, status normalization rules)
- Examples from code:
  - `src/sync/lww.rs` line 5: `// -----------` section separators for logical blocks
  - `src/ir.rs`: Full classification rules documented before `build_ir()` function
  - `src/types.rs`: Serde format requirements documented inline

**JSDoc/TSDoc:**
- Rust uses `///` for public API documentation (not used extensively; prefer inline comments)
- Module documentation with `//!` appears in integration test harness (`tests/integration/mod.rs`)
- Function documentation uses `///` with Markdown formatting where present

**Example documentation:**
```rust
/// Resolves a project name to a calendar URL.
///
/// Lookup order:
///   1. First `calendars` entry whose `project` matches `project`.
///   2. First `calendars` entry whose `project` is `"default"` (fallback).
///   3. `None` — caller should emit an `UnmappedProject` warning.
fn resolve_calendar_url(project: Option<&str>, config: &Config) -> Option<String> {
```

## Function Design

**Size:**
- Typical range: 20–100 lines
- Largest functions: `apply_writeback()` (~400 lines in `src/sync/writeback.rs`), `list_vtodos()` (~150 lines in caldav_adapter)
- Short helper functions common: `to_secs_opt()` (3 lines), `normalize_status()` (7 lines)

**Parameters:**
- Prefer struct/config aggregation over many parameters: `fn run_sync(..., config: &Config, ...)`
- Use references for read-only inputs: `&[TWTask]`, `&HashMap<String, Vec<FetchedVTODO>>`
- Trait objects for dependency injection: `tw: &TwAdapter<R>`, `caldav: &dyn CalDavClient`

**Return Values:**
- Use `Result<T>` for fallible operations
- Return value chaining via method calls: `config.calendars.iter().find(|c| c.project == proj)`
- Boolean predicates for filtering: `filter(|t| { ... })`
- Option for fallback logic: `resolve_project_from_url()` returns `Option<String>`

## Module Design

**Exports:**
- All public functionality re-exported at crate root in `src/lib.rs`
- Example: `pub mod types;`, `pub mod error;`, `pub mod config;`
- Internal submodule organization in `src/sync/mod.rs`: `pub mod deps;`, `pub mod lww;`, `pub mod writeback;`
- Private test modules: `#[cfg(test)] mod tests { ... }`

**Barrel Files:**
- `src/lib.rs` is the barrel file (imports and re-exports all public modules)
- `src/mapper/mod.rs` declares submodules: `pub mod status;` and `pub mod fields;`
- `src/sync/mod.rs` declares submodules and imports key functions

**Module visibility:**
- Public modules for external crate consumers: `caldav_adapter`, `tw_adapter`, `config`, `types`, `error`
- Private modules for internal use: `sync`, `mapper`, `ir`, `ical`, `output`
- Sync submodules (`lww`, `writeback`, `deps`) private to `sync` module

---

*Convention analysis: 2026-03-18*
