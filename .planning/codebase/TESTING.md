# Testing Patterns

**Analysis Date:** 2026-03-18

## Test Framework

**Runner:**
- `cargo test` (built-in Rust test runner)
- Robot Framework for blackbox integration tests (Python-based test automation)

**Assertion Library:**
- Rust: standard `assert!()`, `assert_eq!()`, `assert_ne!()` macros
- Robot Framework: standard keywords like `Should Be Equal As Integers`, `Should Contain`

**Run Commands:**
```bash
# Run all unit and integration tests (Rust)
cargo test

# Run unit tests only
cargo test --lib

# Run integration tests only
cargo test --test integration

# Run with output display
cargo test -- --nocapture

# Robot Framework blackbox tests
CURRENT_UID=$(id -u) CURRENT_GID=$(id -g) docker compose -f tests/robot/docker-compose.yml run --rm robot

# View RF results
cat tests/robot/results/report.html  # Opens in browser
```

## Test File Organization

**Location:**
- Unit tests: co-located with source code using `#[cfg(test)]` modules
- Integration tests: `tests/integration/` directory with separate modules
- Robot Framework blackbox tests: `tests/robot/suites/` (7 test suites)

**Naming:**
- Unit test modules: `#[cfg(test)] mod tests { ... }` at end of file
- Integration test files: `test_*.rs` pattern: `test_first_sync.rs`, `test_lww.rs`, `test_scenarios.rs`
- Robot test suites: numbered by feature: `01_first_sync.robot`, `02_lww_conflict.robot`, `03_orphan.robot`, etc.

**Structure:**
```
tests/
├── integration/
│   ├── mod.rs              # Integration test harness with Docker lifecycle
│   ├── test_first_sync.rs  # First-sync scenario tests
│   ├── test_lww.rs         # Last-writer-wins conflict tests
│   └── test_scenarios.rs   # Comprehensive scenario tests
└── robot/
    ├── docker-compose.yml  # Radicale + TaskWarrior container setup
    ├── Dockerfile          # TaskWarrior build image
    ├── resources/
    │   └── common.robot    # Shared keywords and setup/teardown
    └── suites/
        ├── 01_first_sync.robot
        ├── 02_lww_conflict.robot
        ├── 03_orphan.robot
        ├── 04_status_mapping.robot
        ├── 05_dependencies.robot
        ├── 06_cli_behavior.robot
        ├── 07_field_mapping.robot
        └── syntax_check.robot
```

## Test Structure

**Suite Organization (Rust):**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::caldav_adapter::{CalDavCall, MockCalDavClient};
    use crate::config::CalendarEntry;
    use crate::tw_adapter::{MockTaskRunner, TwAdapter};

    #[test]
    fn test_name() {
        // Arrange
        let config = make_config();
        let mock_tw = MockTaskRunner::new();

        // Act
        let result = some_function(&config, &mock_tw);

        // Assert
        assert!(result.is_ok());
    }
}
```

**Patterns:**
- Helper functions create test fixtures: `fn make_config() -> Config { ... }`
- Mocks pushed to queue before operations: `mock_tw.push_run_response(Ok(json_string))`
- Results checked post-operation: `assert!(result.errors.is_empty())`
- Assertions include context messages: `assert!(!caldavuid.is_empty(), "expected caldavuid UDA set on TW task after sync")`

**Suite Organization (Robot):**
```robot
*** Settings ***
Resource    ../resources/common.robot
Suite Setup    Suite Setup
Suite Teardown    Suite Teardown
Test Teardown    Test Teardown

*** Test Cases ***
Test Name
    [Documentation]    S-01: Clear requirement statement.
    [Tags]    first-sync
    ${uuid1} =    TW.Add TW Task    Buy apples
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Stdout Should Contain    Expected message
```

**Patterns:**
- Suite setup: creates unique CalDAV collection and TaskWarrior data dir (per test run)
- Test setup/teardown: Clear state between tests
- Documentation tag references scenario ID (e.g., `S-01`)
- Test tags for grouping: `first-sync`, `conflict`, `bulk`, `skip-unimplemented`
- Keywords capture state in suite variables: `${COLLECTION_URL}`, `${LAST_STDOUT}`, `${LAST_EXIT_CODE}`

## Mocking

**Framework:**
- Rust: Custom trait-based mocks (no external mocking library)
- Robot: No mocking; uses real CalDAV (Radicale Docker container) and TaskWarrior (Docker image)

**Patterns (Rust):**
```rust
// Define trait for abstraction
pub trait CalDavClient: Send + Sync {
    fn list_vtodos(&self, calendar_url: &str) -> Result<Vec<FetchedVTODO>, CaldaWarriorError>;
    fn put_vtodo(&self, href: &str, ical_content: &str, etag: Option<&str>) -> Result<Option<String>, CaldaWarriorError>;
}

// Implement mock
pub struct MockCalDavClient {
    pub calls: Mutex<Vec<CalDavCall>>,
    pub list_responses: Mutex<Vec<...>>,
}

impl CalDavClient for MockCalDavClient {
    fn list_vtodos(&self, calendar_url: &str) -> Result<Vec<FetchedVTODO>, ...> {
        // Record call
        self.calls.lock().unwrap().push(CalDavCall::ListVTodos(calendar_url.to_string()));
        // Return queued response
        let mut responses = self.list_responses.lock().unwrap();
        if responses.is_empty() { Ok(vec![]) } else { responses.remove(0) }
    }
}
```

**Mock usage example from `src/sync/mod.rs`:**
```rust
let mock_tw = MockTaskRunner::new();
let caldav = MockCalDavClient::new();
// Push expected responses
mock_tw.push_run_response(Ok(json_string));
caldav.push_list_response(Ok(vec![...]));
// Run test
let result = run_sync(&tw_tasks, &vtodos_by_calendar, &config, &tw, &caldav, false, now);
// Verify calls
let calls = mock_tw.get_calls();
assert_eq!(calls.len(), expected_count);
```

**What to Mock:**
- External system interactions: `TaskRunner` (calls to `task` binary), `CalDavClient` (HTTP calls)
- Deterministic timestamps: Pass `now: DateTime<Utc>` to functions (injected for testing)

**What NOT to Mock:**
- Configuration loading/parsing
- Data structures and mappers (test with real types)
- Error types (use real error construction)
- In integration tests: mock only process-level dependencies (use real Radicale/TaskWarrior containers)

## Fixtures and Factories

**Test Data (Rust):**
```rust
fn t(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, mo, d, h, mi, s).unwrap()
}

fn make_config() -> Config {
    Config {
        server_url: "https://dav.example.com".to_string(),
        username: "alice".to_string(),
        password: "secret".to_string(),
        completed_cutoff_days: 90,
        allow_insecure_tls: false,
        caldav_timeout_seconds: 30,
        calendars: vec![CalendarEntry {
            project: "default".to_string(),
            url: "https://dav.example.com/user/calendar/".to_string(),
        }],
    }
}
```

**Test Data (Robot):**
```robot
*** Keywords ***
Suite Setup
    ${slug} =    Evaluate    str(uuid.uuid4())[:8]    modules=uuid
    ${COLLECTION_URL} =    CalDAV.Create Collection    ${slug}
    Set Suite Variable    ${COLLECTION_URL}
    TW.Set TW Data Dir    /tmp/tw-${slug}
    Set Suite Variable    ${TW_DATA_DIR}    /tmp/tw-${slug}
```

**Location:**
- Rust: Inline as helper functions in test modules
- Robot: Shared setup in `tests/robot/resources/common.robot` (Suite Setup/Teardown)

## Coverage

**Requirements:** No explicit coverage threshold enforced

**View Coverage (if desired):**
```bash
# Generate coverage (requires tarpaulin or llvm-cov)
cargo tarpaulin --out Html
# or
cargo llvm-cov --html
```

**Current coverage (from memory):**
- 170 tests total: 148 unit + 4 main + 18 integration (Rust)
- 26 passed Robot Framework tests (31 total with 5 skipped)
- Critical path coverage: sync logic, error handling, field mapping

## Test Types

**Unit Tests:**
- **Scope:** Single function or module in isolation
- **Approach:** Mock external dependencies (CalDAVClient, TaskRunner), test with fixtures
- **Examples:** `src/error.rs` (error display tests), `src/types.rs` (serialization tests), `src/sync/mod.rs` (sync logic)
- **Count:** ~148 unit tests

**Integration Tests:**
- **Scope:** Full sync cycle with mocked TaskWarrior and CalDAV
- **Approach:** Use `TestHarness` to manage Docker containers (Radicale + TaskWarrior image)
- **Setup:** Containers started once per test process (idempotent via `OnceLock`)
- **Examples:** `test_first_sync.rs` (initial push), `test_lww.rs` (conflict resolution), `test_scenarios.rs` (complex scenarios)
- **Skip behavior:** `SKIP_INTEGRATION_TESTS=1` env var skips gracefully; missing Docker fails loudly
- **Count:** ~18 integration tests

**E2E/Blackbox Tests:**
- **Framework:** Robot Framework
- **Scope:** Full caldawarrior binary with real CalDAV (Radicale) and TaskWarrior
- **Approach:** Docker Compose containers, subprocess execution, output assertions
- **Test Harness:** `tests/robot/resources/common.robot` provides shared keywords
- **Examples:** 7 suites covering first-sync, conflicts, orphans, status mapping, dependencies, CLI behavior, field mapping
- **Count:** 31 scenarios (26 passed, 5 skipped)

## Common Patterns

**Async Testing:**
- Not applicable (no async Rust code in caldawarrior)
- Robot Framework runs synchronously within test container

**Error Testing (Rust):**
```rust
#[test]
fn auth_error_directs_to_credentials() {
    let e = CaldaWarriorError::Auth { server_url: "https://dav.example.com".to_string() };
    let msg = e.to_string();
    assert!(msg.contains("dav.example.com"));
    assert!(msg.to_lowercase().contains("credential"));
}

#[test]
fn etag_conflict_carries_vtodo() {
    let vtodo = VTODO { ... };
    let fetched = FetchedVTODO { ... };
    let e = CaldaWarriorError::EtagConflict { refetched_vtodo: fetched.clone() };
    if let CaldaWarriorError::EtagConflict { refetched_vtodo } = e {
        assert_eq!(refetched_vtodo.href, "/cal/test.ics");
    } else {
        panic!("wrong variant");
    }
}
```

**Error Testing (Robot):**
```robot
Test Auth Error On Bad Credentials
    [Tags]    auth
    Run Caldawarrior Sync
    Exit Code Should Be    1
    Stderr Should Contain    Authentication failed
    Stderr Should Contain    check your credentials
```

**Dry-Run Testing (Rust):**
```rust
#[test]
fn first_sync_dry_run_does_not_write_vtodos() {
    let h = TestHarness::new();
    h.add_tw_task("Should not appear in CalDAV during dry run");
    let result = h.run_sync(true);
    assert!(result.errors.is_empty());
    assert_eq!(h.count_caldav_vtodos(), 0, "dry run must not write VTODOs to CalDAV");
    assert!(!result.planned_ops.is_empty(), "expected at least one planned op in dry run");
}
```

**Dry-Run Testing (Robot):**
```robot
First Sync Dry Run Does Not Write VTODOs
    [Documentation]    S-03: Dave wants to preview what sync would do before committing.
    [Tags]    first-sync
    ${uuid} =    TW.Add TW Task    Plan meeting
    Run Caldawarrior Sync Dry Run
    Exit Code Should Be    0
    Stdout Should Contain    [DRY-RUN]
    ${count} =    CalDAV.Count VTODOs    ${COLLECTION_URL}
    Should Be Equal As Integers    ${count}    0
```

## Test Lifecycle

**Integration Test Harness (Rust):**
1. **Module Init (`mod.rs`):**
   - Radicale container started once (idempotent): `CONTAINER_STARTED.get_or_init()`
   - TaskWarrior image built once (idempotent): `TASKWARRIOR_IMAGE_READY.get_or_init()`
   - Wait loop ensures Radicale is reachable (30s timeout)

2. **Test Creation:**
   - `TestHarness::new()` creates isolated TW data dir and CalDAV collection
   - Mocks for TaskRunner and CalDavClient prepared

3. **Test Execution:**
   - Mock responses queued
   - `run_sync()` called with mocks
   - Results asserted

4. **Cleanup:**
   - Implicit (containers persist across test process; cleaned at suite teardown)

**Robot Framework Suite Lifecycle:**
1. **Suite Setup:**
   - Generate unique collection slug
   - Create CalDAV collection
   - Create TaskWarrior data dir
   - Set suite variables for tests

2. **Test Setup:**
   - None (implicit; variables already set)

3. **Test Execution:**
   - Run caldawarrior binary with subprocess
   - Capture exit code, stdout, stderr
   - Assert against captured output

4. **Test Teardown:**
   - Clear TaskWarrior data
   - Clear VTODOs from collection

5. **Suite Teardown:**
   - Delete collection
   - Clean TaskWarrior data dir
   - Remove config files

---

*Testing analysis: 2026-03-18*
