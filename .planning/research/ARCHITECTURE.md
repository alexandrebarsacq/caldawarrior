# Architecture Patterns

**Domain:** Testing/Audit Architecture for CalDAV Bidirectional Sync Tool
**Researched:** 2026-03-18

## Recommended Architecture

The testing architecture for caldawarrior's hardening milestone follows a **three-tier pyramid with an audit overlay**. The pyramid is inverted compared to typical web apps because sync correctness depends heavily on integration and E2E tests (you cannot mock away CalDAV protocol behavior or TaskWarrior CLI quirks). The audit overlay adds static compliance checks that validate iCalendar output against RFC 5545 without running a full sync cycle.

```
                     +-----------------------+
                     |   E2E (Robot Framework)|  <-- Real TW + Real Radicale in Docker
                     |   30+ scenarios         |      Binary subprocess, full pipeline
                     +-----------+-----------+
                                 |
                     +-----------v-----------+
                     |  Integration (Rust)    |  <-- Real Radicale + Dockerized TW
                     |  Library-level sync    |      run_sync() called directly
                     |  18+ tests             |      Mocked adapters for unit-ish tests
                     +-----------+-----------+
                                 |
                     +-----------v-----------+
                     |  Unit (Rust #[test])   |  <-- Pure functions, no I/O
                     |  148+ tests            |      Mappers, LWW, iCal parser, deps
                     +-----------+-----------+
                                 |
                     +-----------v-----------+
                     |  Compliance Audit      |  <-- RFC 5545 validation of generated iCal
                     |  (Static assertions)   |      Property-level checks on VTODO output
                     +-----------------------+
```

### Why This Shape

Sync tools are fundamentally integration problems. A unit test proving the mapper converts priority "H" to integer 1 is necessary but insufficient -- the real question is whether Radicale accepts that VTODO and whether tasks.org renders it correctly. The existing codebase already has 148 unit tests (good coverage of mappers, LWW, iCal parsing). The hardening gap is in the integration and E2E tiers, particularly:

1. **Dependency/relation testing** -- RELATED-TO with RELTYPE=DEPENDS-ON is untested in production client scenarios
2. **Multi-cycle stability** -- ensuring N consecutive syncs reach a stable point
3. **Edge case coverage** -- empty fields, Unicode, long strings, concurrent modifications
4. **Compliance validation** -- generated VTODO output checked against RFC 5545 requirements

### Component Boundaries

| Component | Responsibility | Communicates With |
|-----------|---------------|-------------------|
| **Unit Tests** (`src/**/mod.rs#[cfg(test)]`) | Validate pure business logic: field mapping, LWW decisions, dependency resolution, iCal parsing/generation, config validation | Internal types only; no external systems |
| **Rust Integration Tests** (`tests/integration/`) | Exercise the full sync pipeline (`run_sync()`) against real Radicale and Dockerized TW; verify library-level correctness | Radicale (HTTP), TW container (Docker exec), caldawarrior library API |
| **Robot Framework E2E** (`tests/robot/`) | Black-box binary subprocess testing; validates CLI output format, exit codes, and cross-system state consistency | caldawarrior binary (Process), Radicale (HTTP via CalDAVLibrary.py), TW CLI (subprocess via TaskWarriorLibrary.py) |
| **Compliance Audit** (proposed) | Static validation of VTODO output: required properties present, date formats correct, RELATED-TO structure valid, no RFC violations | iCal output strings only; no live systems |
| **Multi-Client Compatibility** (proposed) | Validate that generated VTODOs are interpretable by target clients (tasks.org, Thunderbird, DAVx5) | Radicale (HTTP), client-specific iCal parsers or manual verification protocols |

### Data Flow

```
Test Input Setup
     |
     v
[TW State]  +  [CalDAV State]     <-- Seeded by test harness
     |               |
     v               v
   caldawarrior sync (binary or library)
     |
     v
[TW State']  +  [CalDAV State']   <-- Verified by assertions
     |               |
     v               v
  Assertions:                       <-- Three categories:
    1. State assertions (TW task fields, VTODO properties)
    2. Output assertions (stdout/stderr format, exit code)
    3. Compliance assertions (RFC 5545 validity of generated iCal)
```

## Component Detail

### 1. Unit Test Layer (existing, adequate)

**Location:** `src/` (15 modules with `#[cfg(test)]` blocks)

**What it covers well:**
- Field mapping bidirectional transformations (TW <-> CalDAV)
- Status mapping (pending/completed/deleted <-> NEEDS-ACTION/COMPLETED/CANCELLED)
- LWW conflict resolution logic (timestamp comparison, content-identical detection)
- iCalendar parsing (line unfolding, property extraction, VTODO builder)
- Dependency resolution (UUID mapping, cycle detection via three-colour DFS)
- Config validation

**Pattern:** Pure function in, assertion out. No I/O, no Docker, no HTTP. Uses `MockTaskRunner` and `MockCalDavClient` for adapter-dependent tests.

**Confidence:** HIGH -- this layer is solid at 148 tests. No architectural changes needed.

### 2. Rust Integration Test Layer (existing, needs expansion)

**Location:** `tests/integration/`

**Current architecture:**
- `TestHarness` struct owns an isolated CalDAV calendar (UUID-based path) and TW data directory
- Radicale runs via Docker Compose (`docker-compose.yml` in `tests/integration/`)
- TW runs in a Docker container (`Dockerfile.taskwarrior` -- Arch Linux with TW 3.x)
- `DockerizedTaskRunner` implements `TaskRunner` trait, executing `task` commands via `docker run --rm`
- `run_sync()` called directly as a library function (not via CLI binary)
- Cleanup: `Drop` on `TestHarness` deletes the CalDAV collection

**Key design decisions:**
- Uses library API directly (not binary subprocess) -- this is intentional; it tests the sync engine, not CLI formatting
- Docker-per-command for TW (not a persistent container) -- ensures clean state but adds latency
- `OnceLock` for container startup (Radicale started once per test process, not per test)
- `TempDir` for TW data isolation (auto-cleaned on drop)

**Expansion targets:**
- More dependency/relation test scenarios
- Multi-calendar testing (currently only single calendar)
- ETag conflict stress testing
- Concurrent modification simulation

**Confidence:** HIGH -- the existing harness is well-designed and extensible.

### 3. Robot Framework E2E Layer (existing, needs expansion)

**Location:** `tests/robot/`

**Current architecture:**
```
docker-compose.yml
  +-- radicale (tomsquest/docker-radicale:3.3.0.0)
  |     - healthcheck on port 5232
  |     - htpasswd auth
  +-- robot (multi-stage Dockerfile)
        - Stage 1: Rust builder (rust:1.85-bookworm) -> caldawarrior binary
        - Stage 2: Arch Linux runner (TW 3.x + Python + RF)
        - Mounts tests/ as /tests:ro
        - Outputs to results/
```

**Custom keyword libraries:**
- `CalDAVLibrary.py` -- CRUD operations on VTODOs via HTTP (PROPFIND, PUT, GET, DELETE), iCalendar manipulation via Python `icalendar` library
- `TaskWarriorLibrary.py` -- TW CLI wrapper (add, modify, complete, delete, export, assertions)
- `common.robot` -- Suite/test setup/teardown, caldawarrior invocation, output assertions

**Test isolation pattern:**
- Suite Setup: creates unique CalDAV collection (UUID slug), unique TW data dir, unique config path
- Test Teardown: wipes TW data and all VTODOs in collection (not the collection itself)
- Suite Teardown: deletes the CalDAV collection entirely

**Current suite structure:**
| Suite | Scenarios | Status |
|-------|-----------|--------|
| 01_first_sync.robot | S-01 through S-05 | All passing |
| 02_lww_conflict.robot | S-10 through S-14 | All passing |
| 03_orphan.robot | S-20 through S-22 | All passing |
| 04_status_mapping.robot | S-30 through S-33 | All passing |
| 05_dependencies.robot | S-40 through S-42 | 2 passing, 1 skipped (S-42 cyclic) |
| 06_cli_behavior.robot | S-50 through S-55 | All passing |
| 07_field_mapping.robot | S-60 through S-63 | All passing |

**Gap analysis (from CATALOG.md):**
- S-70..S-79 (Bulk Operations) -- planned but no suite file yet
- S-80..S-89 (Multi-Sync Journeys) -- planned but no suite file yet
- S-42 (Cyclic dependency) -- skipped, needs CLI-level verification
- Multi-calendar scenarios -- not covered
- RELATED-TO round-trip with real client-generated VTODOs -- not covered

**Confidence:** HIGH -- the existing RF infrastructure is production-quality and easily extensible.

### 4. Compliance Audit Layer (proposed)

**Purpose:** Validate that caldawarrior-generated VTODO output conforms to RFC 5545 requirements. This is not about testing sync logic but about ensuring the iCalendar serialization is well-formed.

**Why a separate layer:** Compliance bugs are subtle. A VTODO might sync perfectly with Radicale (which is lenient) but break when tasks.org or Thunderbird tries to parse it. Catching this requires checking the raw iCal output against the spec, not just verifying the round-trip.

**Recommended implementation:**

```
Approach: Rust unit tests in src/ical.rs that validate generated output
```

Specific checks:
- Required VTODO properties present: UID, DTSTAMP (RFC 5545 section 3.6.2)
- DATE-TIME values use correct format (YYYYMMDDTHHMMSSZ for UTC)
- RELATED-TO property uses correct parameter syntax (`;RELTYPE=DEPENDS-ON:uid`)
- Line folding at 75 octets (RFC 5545 section 3.1)
- CRLF line endings in output
- No duplicate UID within a calendar object
- PRIORITY is integer 0-9 (RFC 5545 section 3.8.1.9)
- STATUS is one of NEEDS-ACTION, COMPLETED, IN-PROCESS, CANCELLED (RFC 5545 section 3.8.1.11)

**NOT recommended:** Using Apple's CalDAVTester (archived February 2024, Python, XML-based test scripts, designed for server-side testing not client-side output validation). Too heavy for this use case.

**NOT recommended:** External iCal validation services (adds network dependency to tests, not reproducible offline).

**Confidence:** MEDIUM -- the specific checks needed are well-defined by RFC 5545, but the implementation approach (embed in unit tests vs. separate validation pass) should be decided during phase planning.

### 5. Multi-Client Compatibility Layer (proposed, future)

**Purpose:** Verify that caldawarrior-generated VTODOs work correctly with real CalDAV client applications.

**Reality check on client RELATED-TO support:**

| Client | VTODO Support | RELATED-TO Support | DEPENDS-ON RELTYPE | Notes |
|--------|---------------|--------------------|--------------------|-------|
| tasks.org (via DAVx5) | Full | Subtasks (PARENT) | Unknown/unlikely | Uses RELATED-TO for parent-child hierarchy, not DEPENDS-ON |
| Thunderbird | Full | No subtasks | No | Bug 194863 open since 2003; no hierarchy support |
| Apple Reminders | Dropped CalDAV | N/A | N/A | iOS 13+ moved to private iCloud silo; no CalDAV VTODO |
| jtx Board (via DAVx5) | Full | Subtasks | Unknown | Supports VTODO with subtask features |
| Nextcloud Tasks | Full | Subtasks (PARENT) | Unknown | Uses RELATED-TO;RELTYPE=PARENT for hierarchy |

**Critical finding:** The RELATED-TO;RELTYPE=DEPENDS-ON property that caldawarrior uses for task dependencies is an RFC 5545 standard property, but **no major CalDAV client currently renders or uses DEPENDS-ON relationships**. Clients that support RELATED-TO typically use RELTYPE=PARENT for subtask hierarchies. This means:

1. The DEPENDS-ON properties will be preserved through sync (servers store them, clients ignore them)
2. No client will display dependency relationships -- they are invisible to users
3. The primary consumer of these relations is caldawarrior itself (TW round-trip) and tasks.org only sees them as opaque data

**Implication for testing:** Multi-client testing should focus on **property preservation** (does the RELATED-TO survive a round-trip through tasks.org?) rather than **feature verification** (does tasks.org show the dependency?).

**Recommended approach:** NOT automated Docker-based multi-client testing (tasks.org is an Android app; Thunderbird requires X11). Instead:

1. **Property preservation tests** -- Create VTODO with all caldawarrior-generated properties, round-trip through Radicale, verify nothing is lost
2. **Manual verification protocol** -- Document a step-by-step manual test procedure for tasks.org and Thunderbird
3. **Client-generated VTODO import tests** -- Capture real VTODOs from tasks.org and Thunderbird, use them as test fixtures in RF tests to verify caldawarrior handles them correctly

**Confidence:** MEDIUM -- property preservation is testable; actual client behavior requires manual verification.

## Patterns to Follow

### Pattern 1: Test Fixture Seed-Sync-Assert

**What:** Every test follows the same three-phase pattern: seed initial state in TW and/or CalDAV, run caldawarrior sync, assert final state in both systems.

**When:** Every integration and E2E test.

**Why:** Sync is stateful; the only way to verify correctness is to control initial state, trigger the sync, and check both sides.

**Example (Robot Framework):**
```robot
*** Test Cases ***
TW Task Syncs To CalDAV
    # SEED
    ${uuid} =    TW.Add TW Task    Buy groceries
    # SYNC
    Run Caldawarrior Sync
    Exit Code Should Be    0
    # ASSERT
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldavuid} =    Set Variable    ${task}[caldavuid]
    CalDAV.VTODO Should Exist    ${COLLECTION_URL}    ${caldavuid}
```

**Example (Rust integration):**
```rust
let h = TestHarness::new();
// SEED
let uuid = h.add_tw_task("Buy groceries");
// SYNC
let result = h.run_sync(false);
// ASSERT
assert!(result.errors.is_empty());
assert_eq!(h.count_caldav_vtodos(), 1);
```

### Pattern 2: Multi-Cycle Stability Assertion

**What:** Run sync N times and assert the system reaches a stable point (zero writes after the first sync that makes changes).

**When:** Every test that modifies state should verify stability by running sync a second time and checking for zero writes.

**Why:** The most dangerous sync bug is a loop -- where each sync triggers another change, causing infinite oscillation. The existing LWW loop-prevention logic must be verified in every scenario.

**Example:**
```robot
*** Test Cases ***
Stable After First Sync
    ${uuid} =    TW.Add TW Task    Buy groceries
    Run Caldawarrior Sync
    Exit Code Should Be    0
    # Second sync should be a no-op
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Sync Should Produce Zero Writes
```

### Pattern 3: Isolated State Per Test

**What:** Each test gets a fresh TW database and (optionally) cleared CalDAV collection.

**When:** Always. Shared state between tests is the number one cause of flaky sync tests.

**Why:** Sync correctness depends on the exact state of both systems. Leftover tasks from a previous test will cause pairing conflicts, phantom operations, and non-deterministic results.

**Current implementation:**
- RF: `Test Teardown` calls `TW.Clear TW Data` and `CalDAV.Clear VTODOs`
- Rust: `TestHarness::new()` creates fresh calendar and TempDir per test; `reset()` for intra-test cleanup

### Pattern 4: Raw iCal Inspection for Compliance

**What:** After sync, fetch the raw iCalendar text from CalDAV and inspect it for specific properties, rather than relying solely on high-level property accessors.

**When:** Tests that verify field mapping, RELATED-TO structure, or RFC compliance.

**Why:** High-level CalDAV libraries may normalize or reinterpret properties. Checking the raw text ensures caldawarrior is generating exactly what the spec requires.

**Example:**
```robot
${raw} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${caldavuid}
Should Contain    ${raw}    RELATED-TO;RELTYPE=DEPENDS-ON:${related_uid}
Should Match Regexp    ${raw}    PRIORITY:[0-9]
Should Contain    ${raw}    DTSTAMP:
```

### Pattern 5: Client-Generated Fixture Import

**What:** Capture real VTODO output from target clients (tasks.org, Thunderbird), save as `.ics` test fixtures, and use them as input for sync tests.

**When:** Multi-client compatibility verification.

**Why:** Each CalDAV client has quirks in how it generates iCalendar data (non-standard properties, different line folding, timezone handling). Testing against real client output is more valuable than testing against hand-crafted iCal strings.

**Proposed directory structure:**
```
tests/robot/fixtures/
  tasks-org/
    simple-task.ics
    task-with-subtask.ics
    completed-task.ics
  thunderbird/
    simple-task.ics
    task-with-priority.ics
```

## Anti-Patterns to Avoid

### Anti-Pattern 1: Mocking the CalDAV Server

**What:** Replacing Radicale with a mock HTTP server that returns canned responses.

**Why bad:** CalDAV is a complex protocol with server-specific behaviors (ETag generation, PROPFIND response format, MKCALENDAR handling). Mocking hides real interoperability bugs. The existing `MockCalDavClient` is appropriate for unit tests of the sync engine, but integration and E2E tests must use real Radicale.

**Instead:** Keep the existing Docker-based Radicale setup. It is fast (sub-second responses), deterministic, and catches real protocol issues.

### Anti-Pattern 2: Testing Through the UI of External Clients

**What:** Automating tasks.org or Thunderbird via Appium/Selenium to verify end-to-end behavior.

**Why bad:** Fragile (UI changes break tests), slow (app startup, animation waits), and adds massive infrastructure complexity (Android emulator, X11 for Thunderbird). The ROI is negative.

**Instead:** Use client-generated fixture import (Pattern 5) for automated tests, and a manual verification protocol for the rare occasions when actual client behavior needs checking.

### Anti-Pattern 3: Asserting on Timestamps in E2E Tests

**What:** Checking exact `LAST-MODIFIED` or `modified` timestamps in test assertions.

**Why bad:** Timestamps depend on wall-clock time. Tests that assert `LAST-MODIFIED == "20260318T143000Z"` will flake on different machines or when the test runner is slow.

**Instead:** Assert on relative properties: "CalDAV LAST-MODIFIED is newer than TW modified" or "LAST-MODIFIED exists and is a valid UTC datetime". The existing LWW tests use `sleep(1)` to ensure timestamp ordering, which is acceptable for Docker-based tests.

### Anti-Pattern 4: One Mega-Suite That Tests Everything

**What:** A single Robot Framework suite file with 50+ test cases covering all scenarios.

**Why bad:** Suite setup/teardown overhead is shared, but test isolation becomes harder to reason about. A failure in test 25 may corrupt state for test 26 if teardown is incomplete.

**Instead:** Keep the existing per-category suite structure (01_first_sync, 02_lww_conflict, etc.). Each suite has its own collection and TW data directory.

## Scalability Considerations

| Concern | At 30 tests (current) | At 100 tests (target) | At 300 tests (unlikely) |
|---------|----------------------|----------------------|------------------------|
| **RF test duration** | ~2 min total | ~5-7 min (parallelizable by suite) | Split into fast/slow suites; parallel Docker Compose stacks |
| **Docker build time** | ~30s (cached) | Same (binary build is the bottleneck) | Pre-built images in CI registry |
| **Radicale load** | Negligible | Negligible (thousands of VTODOs fine) | Single Radicale is sufficient |
| **Test isolation** | Per-suite collection | Per-suite collection (proven pattern) | Consider per-test collections if flakiness appears |
| **CI parallelism** | Sequential (Makefile) | Run `cargo test` and RF in parallel (different Docker stacks) | Split RF suites across parallel runners |

## Suggested Build Order (Dependencies)

The testing architecture components should be built in this order based on dependencies:

```
1. Compliance Audit (unit tests)
   |  No dependencies; can validate existing iCal output immediately
   |
2. E2E Test Expansion (RF suites)
   |  Depends on: existing RF infrastructure (ready)
   |  Covers: S-42 cyclic, S-70..S-79 bulk, S-80..S-89 multi-sync journeys
   |
3. Integration Test Expansion (Rust)
   |  Depends on: existing test harness (ready)
   |  Covers: multi-calendar, relation round-trips, ETag stress
   |
4. Client Fixture Collection
   |  Depends on: manual effort to capture VTODOs from real clients
   |  Independent of other test work
   |
5. Client Fixture Import Tests (RF)
   |  Depends on: (4) fixtures collected
   |  Covers: tasks.org compatibility, Thunderbird compatibility
   |
6. Manual Verification Protocol
   |  Depends on: (1-3) automated tests passing
   |  Documents: step-by-step manual verification for tasks.org, Thunderbird
```

**Rationale for ordering:**
- Start with compliance audit because it requires no infrastructure changes and catches low-hanging bugs in iCal generation
- E2E expansion next because the RF infrastructure is proven and the CATALOG.md already defines the scenarios
- Integration expansion in parallel with E2E since it uses a different harness
- Client fixtures later because they require manual effort and the automated tests should pass first
- Manual verification last because it is the most expensive and least repeatable

## Sources

- [RFC 5545 - iCalendar Specification](https://www.rfc-editor.org/rfc/rfc5545.html) -- Authoritative reference for VTODO component, RELATED-TO property, and all property constraints
- [RFC 4791 - CalDAV Access](https://icalendar.org/CalDAV-Access-RFC-4791/) -- CalDAV protocol specification
- [Apple CalDAVTester (archived)](https://github.com/apple/ccs-caldavtester) -- CalDAV testing framework; archived Feb 2024, reference only
- [CalConnect CalDAVTester](https://github.com/CalConnect/caldavtester) -- Community fork of Apple's tester
- [Forward Email CalDAV Testing Guide](https://github.com/forwardemail/forwardemail.net/blob/master/CALDAV-TESTING.md) -- Multi-client CalDAV testing strategy
- [DAVx5 FAQ: Advanced Task Features](https://www.davx5.com/faq/tasks/advanced-task-features) -- Tasks.org and jtx Board VTODO support
- [Tasks.org CalDAV Documentation](https://tasks.org/docs/caldav_intro.html) -- tasks.org CalDAV integration details
- [iCalendar.org Validator](https://icalendar.org/) -- Online iCalendar validation tool (reference, not for automated testing)
- [Radicale Docker Image](https://github.com/tomsquest/docker-radicale) -- Docker image used in existing test infrastructure
- [Apple Reminders drops CalDAV (Nextcloud #17190)](https://github.com/nextcloud/server/issues/17190) -- Confirmation that Apple Reminders no longer supports CalDAV
- [Thunderbird subtasks bug (Bugzilla #194863)](https://bugzilla.mozilla.org/show_bug.cgi?id=194863) -- Open since 2003; no VTODO hierarchy support
- [MuleSoft Bidirectional Sync Patterns](https://blogs.mulesoft.com/api-integration/patterns/data-integration-patterns-bi-directional-sync/) -- Integration pattern reference
- Existing codebase: `tests/robot/docs/CATALOG.md`, `tests/robot/docs/GAP_ANALYSIS.md`, `tests/integration/mod.rs`
