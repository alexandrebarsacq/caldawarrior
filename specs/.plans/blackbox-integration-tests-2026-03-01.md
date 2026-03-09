# blackbox-integration-tests

## Mission

Build a Robot Framework black-box test suite for caldawarrior that runs entirely in Docker, backed by a living scenario catalog that provides full traceability from user stories to test descriptions to test functions.

## Objectives

- Treat `caldawarrior` binary as a black box: spawn it, assert on stdout/stderr/exit codes and real CalDAV+TW state
- Run entirely in Docker (multi-stage build + docker-compose): no host-side Rust, Python, or TaskWarrior required
- Produce a human-readable scenario catalog (`tests/robot/docs/CATALOG.md`) with user stories tracing to Robot Framework test cases
- Cover all 7 scenario categories: first sync, LWW conflict, orphan/deletion, status mapping, dependencies, CLI behavior, field mapping
- Add as a second test layer on top of existing Rust integration tests (do not replace them)

## Success Criteria

- [ ] `tests/robot/docs/CATALOG.md` exists with ~30 scenarios across 7 categories, each with user story + test case name
- [ ] `docker compose -f tests/robot/docker-compose.yml run --rm robot` runs the full suite with no host dependencies
- [ ] Robot HTML report generated in `tests/robot/results/` with pass/fail per scenario
- [ ] All 7 `.robot` suite files implemented with keyword-driven tests
- [ ] Python keyword libraries cover: CalDAV HTTP operations, TaskWarrior CLI, sync binary invocation
- [ ] No existing `tests/integration/` tests broken

## Assumptions

- `caldawarrior` binary is built inside Docker (multi-stage: Rust builder → RF runner image); no host Rust required
- `task` (TaskWarrior) is installed in the robot runner container alongside Robot Framework
- Radicale is the CalDAV server (already proven in existing tests); same `tomsquest/docker-radicale` image used
- Test isolation: each Robot suite creates a UUID-based Radicale collection; TW data dir wiped in Test Teardown
- Robot Framework keyword libraries written in Python using `requests` for CalDAV HTTP and subprocess for TW/sync CLI
- The scenario catalog is the primary living doc — `.robot` files reference catalog scenario IDs in `[Documentation]` tags

## Constraints

- Must not modify `tests/integration/` (existing Rust tests stay intact)
- New docker-compose is entirely separate from the existing `tests/integration/docker-compose.yml`
- No host dependencies: `docker compose run` is the only command needed (requires Docker Compose v2 plugin; `docker compose` not `docker-compose`)
- Robot Framework results (HTML report, log, output XML) must be written to a mounted host volume so they survive container exit
- Scenario IDs follow `S-NN` format; gaps between category ranges are reserved for future scenarios
- Container runner must execute as a non-root user (required for S-55 permission test and correct result file ownership on Linux hosts)

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Multi-stage Docker build is slow (Rust compile) | high | low | Expected; use layer caching; note in README |
| TaskWarrior version in container differs from host | low | medium | Pin TaskWarrior version in Dockerfile |
| Robot Framework test isolation fails (shared TW state) | medium | high | Explicit `Clear TW Data` in Test Teardown; each suite gets isolated Radicale collection |
| CLI output format assumptions wrong | medium | high | Phase 1 includes output format verification task before writing assertions |
| Some CLI behaviors (warnings, error exit codes) not yet implemented | medium | medium | Flag in Phase 1; mark those test cases `SKIP` with tracking comment if needed |

## Open Questions

- Should there be a `make test-robot` target (Makefile) for convenience? Proposed: yes, add to Phase 6.
- Should the Radicale config in `tests/robot/` be a copy of the existing one or a shared fixture? Proposed: copy (full isolation between test suites).
- Should results be committed to git or gitignored? Proposed: gitignore `tests/robot/results/`.

## Dependencies

- Docker + docker-compose (same prerequisite as existing integration tests)
- Existing Radicale config patterns from `tests/integration/` (reference for configuration)
- `src/output.rs` (625 lines) for exact stdout/stderr format confirmation

## Phases

### Phase 1: Assessment & Output Format Verification

**Goal:** Confirm the exact stdout/stderr output format of the caldawarrior binary and identify which CLI behaviors exist today, producing the ground truth for all Robot Framework assertions.

**Description:** Robot Framework tests will assert on specific stdout/stderr strings. Those patterns must be verified against the real binary before being encoded in `.robot` files. This phase also produces a gap analysis documenting what the existing 12 Rust integration tests cover vs. what the Robot suite will cover.

#### Tasks

- **Verify CLI output format and behavior coverage** `investigation` `small`
  - Description: (1) Read `src/output.rs` fully — extract exact stdout format strings for: success summary, dry-run prefix, dry-run summary, skip reasons, warning line format, error line format. (2) Check which behaviors are implemented: bad-auth exit code, config permission warning, recurring task warning, cyclic dependency warning. (3) Catalog all 12 existing Rust integration tests (file, function, what they assert) and note which are library-level vs. CLI-level. Document findings in `tests/robot/docs/GAP_ANALYSIS.md`.
  - File: `tests/robot/docs/GAP_ANALYSIS.md`
  - Acceptance criteria:
    - Exact stdout pattern for success summary documented (e.g. `Synced: N created, N updated in CalDAV; N created, N updated in TW`)
    - Exact dry-run line prefix documented
    - Warning and error line formats documented
    - List of behaviors not yet implemented in binary flagged (will be marked `SKIP` in robot tests)
    - All 12 existing tests catalogued

#### Verification

- **Manual checks:** Is every output format string in GAP_ANALYSIS.md traceable to a line in `src/output.rs`?

### Phase 2: Scenario Catalog

**Goal:** Create `tests/robot/docs/CATALOG.md` — the authoritative living document with user stories, test scenario descriptions, and test case name references.

**Description:** Each entry has: scenario ID, category, a user story written in plain language (non-technical persona), the setup state, the expected outcome (stdout pattern, exit code, CalDAV/TW state), and the Robot Framework test case name. This document is the spec that the `.robot` files implement. Gaps between category ranges (S-06–S-09, etc.) are reserved for future scenarios.

#### Tasks

- **Create scenario catalog** `implementation` `medium`
  - Description: Create `tests/robot/docs/CATALOG.md`. Include a header explaining the traceability chain (user story → catalog entry → `.robot` test case) and the scenario ID convention. Then write all ~30 scenarios across 7 categories using this format for each entry: ID, Category, User Story (2–4 sentence narrative), Setup, Expected Outcome (stdout pattern, stderr pattern, exit code, CalDAV state, TW state), Robot Test Case Name. Categories and ranges: First Sync (S-01–S-05), LWW Conflict (S-10–S-14), Orphan & Deletion (S-20–S-22), Status Mapping (S-30–S-33), Dependencies (S-40–S-42), CLI Behavior (S-50–S-55), Field Mapping (S-60–S-63). Output format strings must come from Phase 1 GAP_ANALYSIS.md findings. Example entry format:
    ```
    ## S-01: First Sync — TaskWarrior Tasks Pushed to CalDAV
    **Category:** First Sync
    **Robot test:** `suites/01_first_sync.robot :: S-01: First Sync — Tasks Pushed to CalDAV`

    ### User Story
    Alice has been using TaskWarrior for months and decides to start syncing to her
    Nextcloud CalDAV. On the first run she expects all her existing tasks to appear
    as VTODOs in her calendar — no configuration beyond the config file required.

    ### Setup
    - 2 pending TW tasks: "Buy milk", "Call doctor"
    - Empty CalDAV collection

    ### Expected Outcome
    - CalDAV contains 2 VTODOs with matching SUMMARY fields
    - stdout: matches `Synced: 2 created, 0 updated in CalDAV`
    - exit code: 0
    - Both TW tasks have non-empty `caldavuid` UDA
    ```
  - File: `tests/robot/docs/CATALOG.md`
  - Acceptance criteria:
    - ≥28 scenarios documented with all fields filled
    - Output patterns come from Phase 1 verified format (not guessed)
    - Each scenario has a Robot test case name that matches exactly what will be in the `.robot` file
    - Readable by a non-developer (user stories use plain language)

#### Verification

- **Manual checks:** Read CATALOG.md top to bottom — is every scenario understandable without reading the code?

### Phase 3: Docker Infrastructure

**Goal:** Create the Docker environment that runs the full Robot Framework test suite with no host dependencies.

**Description:** A multi-stage `Dockerfile` builds `caldawarrior` from source in a Rust container, then copies the binary into a Debian-based runner image that also has Robot Framework, Python `requests`, and `task` (TaskWarrior) installed. A separate `docker-compose.yml` brings up Radicale and the robot runner.

#### Tasks

- **Create multi-stage Dockerfile** `implementation` `medium`
  - Description: `tests/robot/Dockerfile`. Stage 1 (`builder`): `FROM rust:latest`, `WORKDIR /build`, copy `Cargo.toml`, `Cargo.lock`, `src/`. Use BuildKit cache mounts for speed: `RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/build/target cargo build --release`. Output binary at `/build/target/release/caldawarrior`. Stage 2 (`runner`): `FROM debian:bookworm-slim`, install: `python3 python3-pip curl gnupg taskwarrior`, install via pip: `robotframework requests icalendar` (the `icalendar` package is required for CalDAVLibrary's GET→parse→mutate→PUT operations). Copy caldawarrior binary from builder stage. Create a non-root user: `RUN useradd -m testrunner` and switch with `USER testrunner` before `WORKDIR /tests`. The runner stage must NOT include the Rust toolchain. Pin pip package versions (e.g. `robotframework==7.*`, `icalendar==5.*`, `requests==2.*`).
  - File: `tests/robot/Dockerfile`
  - Acceptance criteria:
    - `docker build -f tests/robot/Dockerfile .` succeeds from repo root (with `DOCKER_BUILDKIT=1`)
    - Built image has `caldawarrior`, `task`, and `robot` on PATH
    - `whoami` inside the container returns `testrunner` (not `root`)
    - Image does not include Rust toolchain
  - Depends on: none

- **Create docker-compose.yml** `implementation` `small`
  - Description: `tests/robot/docker-compose.yml`. Two services: (1) `radicale`: `image: tomsquest/docker-radicale:latest`, mounts `./radicale.config:/config/config:ro` and `./htpasswd:/config/htpasswd:ro`, `environment: TZ=UTC`, add a healthcheck so the robot service waits for Radicale to be ready: `healthcheck: test: ["CMD", "curl", "-f", "-u", "testuser:testpassword", "http://localhost:5232/"] interval: 2s retries: 15`. (2) `robot`: `build: context: ../.. dockerfile: tests/robot/Dockerfile` (context is repo root; dockerfile path is relative to repo root), `depends_on: radicale: condition: service_healthy`, `environment: RADICALE_URL=http://radicale:5232 RADICALE_USER=testuser RADICALE_PASSWORD=testpassword TZ=UTC`, mounts `./:/tests:ro` (test files read-only) and `./results:/results` (output, writable), command: `robot --outputdir /results /tests/suites/`. The `./results/` directory is created by Makefile target before compose run.
  - File: `tests/robot/docker-compose.yml`
  - Acceptance criteria:
    - `docker compose -f tests/robot/docker-compose.yml run --rm robot robot --version` exits 0
    - Robot container waits for Radicale healthcheck before starting (no connection-refused errors on first suite)
    - Results written to `tests/robot/results/` with correct ownership (non-root testrunner user)
    - `TZ=UTC` set on both containers
  - Depends on: Create multi-stage Dockerfile

- **Create Radicale config and credentials** `implementation` `small`
  - Description: Create `tests/robot/radicale.config` (copy and adapt from `tests/integration/radicale.config`: basic auth, owner_only rights, filesystem storage). Create `tests/robot/htpasswd` with content `testuser:testpassword` literally (plain text); set `htpasswd_encryption = plain` in radicale.config to match. Add `tests/robot/results/` to `.gitignore`. Create `tests/robot/results/.gitkeep` so the directory exists in the repo.
  - File: `tests/robot/radicale.config`
  - Acceptance criteria:
    - Radicale starts and the healthcheck passes (HTTP 200 with testuser:testpassword)
    - `tests/robot/results/` is gitignored (only `.gitkeep` is tracked)
  - Depends on: none

#### Verification

- **Manual checks:** `docker compose -f tests/robot/docker-compose.yml run --rm robot robot --version` exits 0 and prints RF version

### Phase 4: Robot Framework Keyword Libraries

**Goal:** Implement the three Python keyword libraries and the shared `common.robot` resource that all test suites will use.

**Description:** `CalDAVLibrary.py` handles all HTTP operations against Radicale. `TaskWarriorLibrary.py` wraps the `task` CLI. `common.robot` provides suite-level setup/teardown, test-level teardown, and the core `Run Caldawarrior Sync` keyword that captures stdout/stderr/exit code.

#### Tasks

- **Implement CalDAVLibrary.py** `implementation` `medium`
  - Description: `tests/robot/resources/CalDAVLibrary.py`. Robot keyword library (class-based). Constructor reads `RADICALE_URL`, `RADICALE_USER`, `RADICALE_PASSWORD` from environment. Uses `requests` for HTTP and `icalendar` for iCal parsing. Keywords: `Create Collection` (MKCOL, returns collection URL), `Delete Collection` (DELETE), `Count VTODOs` → int (PROPFIND depth:1, counts `.ics` hrefs), `Put VTODO` (args: uid, ical_text; PUT to `<collection>/<uid>.ics`), `Get VTODO Raw` (args: uid → ical text string), `Delete VTODO` (args: uid), `Modify VTODO Summary` (args: uid, new_summary; GET→parse with `icalendar`→mutate→serialize→PUT), `Modify VTODO Status` (args: uid, status; sets STATUS and COMPLETED timestamp if status=COMPLETED), `Get VTODO Property` (args: uid, property_name → string value, uses `icalendar` parser), `VTODO Should Exist` (args: uid), `VTODO Should Have Property` (args: uid, property, value). All HTTP errors raise Robot `AssertionError` with descriptive messages.
  - File: `tests/robot/resources/CalDAVLibrary.py`
  - Acceptance criteria:
    - `Create Collection` + `Put VTODO` + `Count VTODOs` round-trip works against live Radicale
    - `Modify VTODO Summary` correctly round-trips via `icalendar` (not regex)
    - All keywords have Robot-compatible docstrings
  - Depends on: Create docker-compose.yml (infrastructure must exist to test against)

- **Implement TaskWarriorLibrary.py** `implementation` `medium`
  - Description: `tests/robot/resources/TaskWarriorLibrary.py`. Robot keyword library (class-based). All subprocess calls set both `TASKDATA=<path>` and `TASKRC=<path>/.taskrc` in the environment to fully override any system-level config. Keywords: `Set TW Data Dir` (args: path; creates directory, sets instance `_tw_env`, writes `.taskrc` with `data.location=<path>`, `uda.caldavuid.type=string`, `uda.caldavuid.label=CaldavUID`, `confirmation=no`, `json.array=on`). `Add TW Task` (args: description, **kwargs → parses "Created task N", exports to get UUID, returns uuid string). `Get TW Task` (args: uuid → dict, uses `task export rc.json.array=on <uuid>`). `Complete TW Task` (args: uuid). `Delete TW Task` (args: uuid). `TW Task Count` → int. `TW Task Should Have Caldavuid` (args: uuid). `TW Task Should Have Status` (args: uuid, status). `TW Task Should Have Field` (args: uuid, field, expected). `Clear TW Data` (removes all task data files, preserves directory).
  - File: `tests/robot/resources/TaskWarriorLibrary.py`
  - Acceptance criteria:
    - `Add TW Task` + `Get TW Task` round-trip returns correct description
    - `Set TW Data Dir` writes a valid `.taskrc` with caldavuid UDA configured
    - Two instances with different data dirs do not share task state
  - Depends on: Create docker-compose.yml

- **Implement common.robot** `implementation` `medium`
  - Description: `tests/robot/resources/common.robot`. Provides: (1) `Suite Setup` keyword: generates UUID slug (e.g. `test-<uuid4>`), calls `Create Collection` → sets `${COLLECTION_URL}`, calls `Set TW Data Dir` with `/tmp/tw-<uuid4>`. (2) `Suite Teardown`: calls `Delete Collection`, `Clear TW Data`, deletes temp config dir. (3) `Test Teardown`: calls `Clear TW Data` (TW reset per test; CalDAV collection shared across suite). (4) `Run Caldawarrior Sync` keyword: writes a temp config file to `/tmp/cw-config-<test-slug>.toml` with exact content: `server_url = "<RADICALE_URL>" username = "<RADICALE_USER>" password = "<RADICALE_PASSWORD>" [[calendar]] project = "default" url = "<COLLECTION_URL>"`. Runs `caldawarrior --config /tmp/cw-config-<slug>.toml sync` with `TASKDATA` and `TASKRC` env vars set. Captures stdout/stderr/returncode. Deletes temp config file after capture. Sets test variables `${LAST_STDOUT}`, `${LAST_STDERR}`, `${LAST_EXIT_CODE}`. (5) `Run Caldawarrior Sync Dry Run`: same with `--dry-run`. (6) `Stdout Should Contain` (pattern), `Stderr Should Contain` (pattern), `Exit Code Should Be` (code). Note: config.toml uses `${COLLECTION_URL}` (interpolated from Suite Setup) to ensure per-suite CalDAV isolation.
  - File: `tests/robot/resources/common.robot`
  - Acceptance criteria:
    - `Run Caldawarrior Sync` on empty TW + CalDAV exits 0
    - `${LAST_STDOUT}` populated after each call
    - No temp config files remain after Test Teardown
    - Two sequential suites do not share TW state or CalDAV collection
  - Depends on: Implement CalDAVLibrary.py, Implement TaskWarriorLibrary.py

#### Verification

- **Run tests:** `docker compose -f tests/robot/docker-compose.yml run --rm robot robot --dryrun /tests/suites/` (syntax check only, no real execution)
- **Fidelity review:** Compare keyword API against what Phase 5 test cases need

### Phase 5: Robot Framework Test Suites

**Goal:** Implement all 7 `.robot` suite files covering ~30 scenarios from the catalog.

**Description:** Each suite file has a `*** Settings ***` block (imports, Suite Setup/Teardown), a `*** Test Cases ***` block where each case name matches exactly the name in CATALOG.md, and `[Documentation]` tags with the user story. Test cases use keywords from Phase 4 libraries.

#### Tasks

- **Implement 01_first_sync.robot** `implementation` `medium`
  - Description: S-01 to S-05. `S-01: First Sync — Tasks Pushed to CalDAV`: add 2 TW tasks, sync, assert CalDAV has 2 VTODOs, stdout matches success pattern, exit 0. `S-02: caldavuid UDA Set After First Sync`: sync 1 task, assert TW task has caldavuid. `S-03: Dry-Run Does Not Write to CalDAV`: dry-run with 1 TW task, assert CalDAV still 0, stdout contains `[DRY-RUN]`. `S-04: Project Routing to Default Calendar`: task with no project routes to configured default calendar (1 VTODO created). `S-05: CalDAV-Only VTODO Pulled to TW`: put VTODO directly in CalDAV, sync, assert TW has task with matching description.
  - File: `tests/robot/suites/01_first_sync.robot`
  - Acceptance criteria:
    - All 5 test cases pass
    - Each has `[Documentation]` tag with user story text matching CATALOG.md
    - `S-03` asserts CalDAV count = 0 after dry-run
  - Depends on: Implement common.robot

- **Implement 02_lww_conflict.robot** `implementation` `medium`
  - Description: S-10 to S-14. `S-10: TW Wins LWW Conflict`: sync task, modify TW task, sync again, assert CalDAV VTODO updated (TW description wins). `S-11: CalDAV Wins LWW Conflict`: sync task, reach stable sync (sync twice), modify CalDAV VTODO externally, sync again, assert TW task updated. `S-12: Stable Sync Produces Zero Writes`: sync twice, assert second sync stdout matches `0 created, 0 updated` in both directions. `S-13: ETag Conflict Recovered Gracefully`: modify CalDAV VTODO and TW task concurrently, sync, assert exit code 0 (no crash). `S-14: TW Task With No Modified Timestamp Syncs Correctly`: create minimal TW task (entry only, no modified), sync, assert VTODO created in CalDAV, exit 0.
  - File: `tests/robot/suites/02_lww_conflict.robot`
  - Acceptance criteria:
    - All 5 test cases pass
    - `S-12` explicitly asserts "0 created, 0 updated" in stdout
  - Depends on: Implement common.robot

- **Implement 03_orphan.robot** `implementation` `small`
  - Description: S-20 to S-22. `S-20: CalDAV Deletion Removes TW Task`: sync TW task, delete VTODO from CalDAV externally, sync, assert TW task deleted and CalDAV still 0. `S-21: TW Deletion Removes CalDAV VTODO`: sync TW task, delete TW task, sync, assert CalDAV 0 VTODOs. `S-22: Large Dataset Stability`: add 20 TW tasks, sync (all 20 to CalDAV), sync again, assert second sync stdout shows 0 writes.
  - File: `tests/robot/suites/03_orphan.robot`
  - Acceptance criteria:
    - All 3 test cases pass
    - `S-20` confirms VTODO not re-created (CalDAV count = 0 after sync)
  - Depends on: Implement common.robot

- **Implement 04_status_mapping.robot** `implementation` `small`
  - Description: S-30 to S-33. `S-30: CalDAV COMPLETED Status Propagates to TW`: push TW task, mark VTODO STATUS:COMPLETED in CalDAV, sync, assert TW task status = completed. `S-31: TW Completed Task Propagates to CalDAV`: sync TW task, complete TW task, sync, assert VTODO STATUS:COMPLETED. `S-32: CalDAV CANCELLED Status Maps to TW Deleted`: put VTODO with STATUS:CANCELLED, sync, assert TW task created with status deleted. `S-33: Recurring VTODO Emits Warning and Is Skipped`: put VTODO with RRULE property, sync, assert stderr contains warning about recurring (skip with `[Tags] skip-unimplemented` if behavior not confirmed in Phase 1).
  - File: `tests/robot/suites/04_status_mapping.robot`
  - Acceptance criteria:
    - S-30, S-31, S-32 pass
    - S-33 either passes or is tagged `skip-unimplemented` with explanatory comment
  - Depends on: Implement common.robot

- **Implement 05_dependencies.robot** `implementation` `small`
  - Description: S-40 to S-42. `S-40: TW Dependency Synced to CalDAV RELATED-TO`: create tasks A and B where B depends on A, sync, assert B's VTODO has `RELATED-TO;RELTYPE=DEPENDS-ON` pointing to A's UID. `S-41: CalDAV RELATED-TO Synced to TW Depends`: put two VTODOs with RELATED-TO, sync, assert TW task has depends field (flag as `skip-unimplemented` if Phase 1 confirms reverse mapping not implemented). `S-42: Cyclic Dependency Emits Warning`: create cyclic TW dependency (A→B→A), sync, assert stderr contains warning about cyclic/cycle, exit code 0 (graceful).
  - File: `tests/robot/suites/05_dependencies.robot`
  - Acceptance criteria:
    - All 3 test cases pass or are tagged `skip-unimplemented` per Phase 1 findings
    - `S-40` reads raw VTODO iCal text and checks for RELATED-TO property string
  - Depends on: Implement common.robot

- **Implement 06_cli_behavior.robot** `implementation` `medium`
  - Description: S-50 to S-55. `S-50: Success Output Format`: sync 1 TW task, assert stdout matches exact success summary pattern from CATALOG.md (verified in Phase 1). `S-51: Dry-Run Output Format`: dry-run with 1 task, assert stdout has `[DRY-RUN]` prefix lines and summary. `S-52: Invalid Credentials Exit Code`: run sync with wrong password in config, assert exit code 1 and stderr has error message. `S-53: Missing Config File Exit Code`: run with `--config /nonexistent`, assert exit code 1 and stderr mentions config. `S-54: Unmapped Project Warning`: task with project not in config calendars, sync, assert stderr contains `[WARN]` and `UnmappedProject`. `S-55: Config File Permission Warning`: chmod config 0644, sync, assert stderr contains `[WARN]` and permission; container runs as non-root `testrunner` user so chmod is effective.
  - File: `tests/robot/suites/06_cli_behavior.robot`
  - Acceptance criteria:
    - S-50 through S-55 pass (non-root container user makes S-55 reliable)
    - Error tests assert exit code = 1
  - Depends on: Implement common.robot

- **Implement 07_field_mapping.robot** `implementation` `medium`
  - Description: S-60 to S-63. `S-60: Due Date Roundtrip`: TW task with due date → sync → VTODO has DUE → sync back → TW has same due (±1s). `S-61: Tags Roundtrip`: TW tags ["home","urgent"] → CATEGORIES → back to TW tags. `S-62: Priority Roundtrip`: TW priority H → VTODO PRIORITY:1 → back to TW H. `S-63: Wait Date Roundtrip`: future wait date → X-TASKWARRIOR-WAIT → back to TW wait field.
  - File: `tests/robot/suites/07_field_mapping.robot`
  - Acceptance criteria:
    - All 4 test cases pass
    - Datetime comparisons allow ±1 second tolerance
    - Each test performs a full bidirectional roundtrip (TW→CalDAV→TW)
  - Depends on: Implement common.robot

#### Verification

- **Run tests:** `docker compose -f tests/robot/docker-compose.yml run --rm robot`
- **Fidelity review:** Compare all test case names against CATALOG.md — do they match exactly?

### Phase 6: Verification & Documentation

**Goal:** Full suite passes, results are accessible, and the project is documented for future contributors.

#### Tasks

- **Run full suite and fix failures** `investigation` `small`
  - Description: Run `docker compose -f tests/robot/docker-compose.yml run --rm robot` and review the HTML report in `tests/robot/results/`. Fix any test failures. If a test relies on a behavior not implemented in the binary, tag it `skip-unimplemented` and add a comment referencing the missing feature (do not delete the test).
  - File: N/A
  - Acceptance criteria:
    - All tests pass or are tagged `skip-unimplemented` with explanatory comment
    - HTML report (`tests/robot/results/report.html`) opens and shows results

- **Update CATALOG.md with implementation status** `implementation` `small`
  - Description: For each scenario in CATALOG.md, add ✅ (passing), ⚠️ (skipped/unimplemented), or ❌ (failing with issue link) status marker. Update any test case names that changed during implementation.
  - File: `tests/robot/docs/CATALOG.md`
  - Acceptance criteria:
    - Every scenario has a status marker
    - All test case names match actual `.robot` file case names

- **Create Makefile with test targets** `implementation` `small`
  - Description: Create or update `Makefile` at repo root with targets: `test-robot` (creates `tests/robot/results/` if absent, then runs `docker compose -f tests/robot/docker-compose.yml run --rm robot`), `test-integration` (runs `cargo test --test integration`), `test-all` (runs both). Include a `build-robot` target for `docker compose -f tests/robot/docker-compose.yml build`.
  - File: `Makefile`
  - Acceptance criteria:
    - `make test-robot` runs the full Robot Framework suite end-to-end
    - `make build-robot` builds the Docker images
    - Targets are documented with `## comment` for `make help` compatibility
  - Depends on: Create docker-compose.yml

- **Update README.md Testing section** `implementation` `small`
  - Description: Add or update the "Testing" section to explain the two test layers: (1) White-box Rust integration tests (`make test-integration`, requires Docker Compose v2); (2) Black-box Robot Framework behavioral tests (`make test-robot`, fully self-contained, no host Rust/Python/TW required). Note Docker Compose v2 requirement (`docker compose`, not `docker-compose`). Link to `tests/robot/docs/CATALOG.md`.
  - File: `README.md`
  - Acceptance criteria:
    - README explains both test layers
    - `make test-robot` is the documented invocation
    - Docker Compose v2 requirement stated
    - Link to CATALOG.md present
  - Depends on: Create Makefile with test targets

#### Verification

- **Run tests:** `make test-robot` (all RF tests)
- **Fidelity review:** Full implementation vs. spec comparison
- **Manual checks:** Open `tests/robot/results/report.html` — is every scenario visible with documentation?
