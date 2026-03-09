# Spec Review: blackbox-integration-tests

**Spec ID:** blackbox-integration-tests-2026-03-01-001
**Review Type:** standalone spec review
**Verdict:** unknown
**Template:** PLAN_REVIEW_FULL_V1
**Date:** 2026-03-01T10:37:03.214833
**Provider:** claude

## Review Output

# Review Summary

## Critical Blockers
Issues that must be fixed before implementation can begin.

- **[Architecture]** Radicale and Rust base images pinned to `latest`
  - **Description:** `task-3-1` uses `FROM rust:latest` and `task-3-2` uses `image: tomsquest/docker-radicale:latest`. Both tags are moving targets that can silently change between runs.
  - **Impact:** Tests can pass on one day and fail the next due to upstream image changes with no code change in this repo. This is an especially serious failure mode for a CI system where reproducibility is the entire point.
  - **Fix:** Pin both images to specific digest or version tags (e.g., `rust:1.76-bookworm`, `tomsquest/docker-radicale:3.3.0.0`). Record pinned versions in a comment citing the date they were selected. Add a Makefile target `update-docker-pins` as a deliberate upgrade path.

- **[Completeness]** `skip-unimplemented` tag semantics never defined
  - **Description:** The tag `skip-unimplemented` is referenced in Phase 1 gap analysis, Phase 2 catalog, and at least six test tasks (S-33, S-41, S-42, S-55), but nowhere in the spec is the tag's behavior defined: the exact string, whether Robot Framework skips or just tags the test, and how CI should interpret tagged-but-skipped tests.
  - **Impact:** The CI pipeline cannot be built until this is resolved. If tagged tests are counted as failures, the suite never goes green. If they are silently excluded, regressions can hide behind the tag. Without a definition, the Phase 6 fidelity review has no acceptance criterion for these cases.
  - **Fix:** Add a dedicated `*** Settings ***` block in `common.robot` that registers the tag and its intent. Define explicitly: (a) the canonical tag string (`skip-unimplemented`), (b) that tests use Robot's `[Tags] skip-unimplemented` combined with `Skip` keyword so they appear in the report as SKIP not FAIL, (c) that CI counts SKIP separately and does not fail the build on SKIP, and (d) add a `[Comment]` in each skipped test referencing the missing feature by name.

- **[Interface Design]** caldawarrior config TOML format is assumed, not verified
  - **Description:** `task-4-3` (common.robot) writes a hard-coded TOML structure (`server_url`, `username`, `password`, `[[calendar]]` with `project` and `url`) without any Phase 1 task explicitly reading `src/` to confirm this is the actual schema. The gap analysis task (`task-1-1`) focuses entirely on output formats, not config parsing.
  - **Impact:** If the actual config key names differ (e.g., `calendar_url` instead of `url`, or a nested `[server]` block), every single test in the suite will fail with a config parsing error rather than a test logic error, making debugging extremely confusing.
  - **Fix:** Add an explicit acceptance criterion to `task-1-1`: "Config file schema documented: all required keys, optional keys, and `[[calendar]]` array field names verified against `src/config.rs` or equivalent." Alternatively, add a brief `task-1-0` that reads the config source file before any other work begins.

- **[Architecture]** No `.dockerignore` file — Docker build context includes full repository
  - **Description:** The Dockerfile build context is set to the repo root (to access `Cargo.toml`, `Cargo.lock`, and `src/`), but no `.dockerignore` is defined. Without it, the entire repo including `target/`, `.git/`, test results, and documentation is sent to the Docker daemon on every build.
  - **Impact:** On a typical Rust project, `target/` can be several gigabytes. Build times increase dramatically, and the Rust build stage may pick up stale artifacts from the host rather than the clean source. This can cause non-reproducible builds.
  - **Fix:** Add a task in Phase 3 to create `.dockerignore` at the repo root that excludes `.git/`, `target/`, `tests/robot/results/`, and `*.md`. Include only what the Dockerfile `COPY` statements actually need.

---

## Major Suggestions
Significant improvements that enhance quality, maintainability, or design.

- **[Verification]** `verify-4-1` (dry-run syntax check) is insufficient as the sole library verification
  - **Description:** `robot --dryrun` verifies Robot Framework syntax but does not execute Python keyword implementations. `CalDAVLibrary.py` and `TaskWarriorLibrary.py` can have Python import errors, incorrect `requests` calls, or malformed iCal construction that `--dryrun` will never catch.
  - **Impact:** A library with a Python import error will produce cryptic "No keyword with name" errors in Phase 5, and all 30 scenarios will fail at once with no useful diagnostics. Debugging this mid-phase is expensive.
  - **Fix:** Add a `task-4-4` that runs a minimal smoke test inside the Docker container: `Create Collection` → `Put VTODO` → `Count VTODOs` → `Delete Collection` using a one-off robot file. Similarly, `Add TW Task` → `Get TW Task` → `TW Task Should Have Field`. This verifies the Python round-trip against a live Radicale before any test suite is written.

- **[Architecture]** Per-test CalDAV isolation is insufficient for certain failure modes
  - **Description:** `common.robot` shares one CalDAV collection per suite and only resets TaskWarrior data between tests. If test S-20 (CalDAV deletion) fails mid-execution and leaves an unexpected VTODO in the collection, subsequent tests in the same suite see a pre-polluted CalDAV state.
  - **Impact:** Failures become order-dependent and non-deterministic. A flaky network during S-20 could cause S-21 through S-22 to fail with misleading error messages about VTODO count rather than the actual issue.
  - **Fix:** Enhance the `Test Teardown` in `common.robot` to also call `Delete Collection` and `Create Collection` (recreating fresh state), or alternatively track all UIDs created in a test and delete them individually. Document the chosen approach explicitly as a design decision.

- **[Completeness]** No CI/CD workflow file specified
  - **Description:** The spec defines Docker infrastructure and a Makefile but provides no GitHub Actions (or equivalent) workflow file that actually runs `make test-robot` in CI.
  - **Impact:** The test suite exists but is never automatically executed on pull requests. The entire value of black-box integration tests depends on them being run automatically.
  - **Fix:** Add a `task-6-5` to create `.github/workflows/integration-tests.yml` (or equivalent). Minimum viable workflow: trigger on push/PR to main, run `make build-robot && make test-robot`, upload `tests/robot/results/` as an artifact, and fail the CI check if any test is FAIL (not SKIP).

- **[Architecture]** Phase 4 blocked by Phase 3 despite the description acknowledging parallelism
  - **Description:** The spec's description for Phase 4 says "Can be authored in parallel with Phase 3 but execution testing requires Phase 3 complete," yet the dependency graph has `phase-4` strictly `blocked_by phase-3`. This prevents starting library authoring until Docker infrastructure is complete.
  - **Impact:** Delivery timeline is artificially extended. Library authoring (Python and Robot syntax) has no dependency on Docker being built — only execution verification does.
  - **Fix:** Split Phase 4 into two phases: Phase 4a (author libraries, no Docker required, parallel with Phase 3) and Phase 4b (execution verification, blocked by Phase 3 and Phase 4a). Alternatively, keep Phase 4 but mark only `verify-4-1` as blocked by Phase 3.

- **[Data Model]** VTODO UID strategy for test-injected items not specified
  - **Description:** `task-4-1` defines a `Put VTODO (PUT uid.ics)` keyword but never specifies how UIDs are generated for test-side VTODOs. If tests use static UIDs (e.g., `test-task-1`), parallel test runs or suite reruns can produce UID collisions in Radicale's filesystem storage.
  - **Impact:** Radicale will either reject the PUT (409 Conflict) or silently overwrite a previous test's data, causing both false passes and false failures.
  - **Fix:** Generate UUIDs for all test-side VTODOs using Python's `uuid.uuid4()` within the keyword. Document in the keyword's docstring that it returns the generated UID so callers can reference it. The Suite Setup UUID slug (already in common.robot) can serve as a namespace prefix.

- **[Verification]** S-12 and S-22 "zero writes" assertions use unverified string patterns
  - **Description:** Tasks for S-12 ("0 created 0 updated in both directions") and S-22 ("second sync shows 0 writes") assert against specific stdout substrings, but Phase 1's gap analysis task focuses on documenting formats from `src/output.rs` without an explicit requirement to confirm the zero-write case format.
  - **Impact:** If the zero-write summary line format is "Nothing to sync" or "↑ 0 ↓ 0" rather than "0 created 0 updated," both S-12 and S-22 will fail despite correct behavior. These are among the most valuable tests (idempotency verification).
  - **Fix:** Add "zero-write summary line (both directions)" to the explicit list of formats to document in `task-1-1`'s acceptance criteria. Create a named constant in `common.robot` for this pattern (e.g., `${ZERO_WRITES_PATTERN}`) so it's defined in one place.

- **[Security]** Plain-text htpasswd committed to repository with no guidance
  - **Description:** `task-3-3` creates `tests/robot/htpasswd` with literal `testuser:testpassword` in plain text and uses `htpasswd_encryption = plain` in Radicale config. While appropriate for a test environment, these files are committed to version control without documentation warning against credential reuse.
  - **Impact:** Developers may cargo-cult the pattern into staging or production environments. Password scanning tools in CI (e.g., `truffleHog`, GitHub secret scanning) may flag the file, causing false-positive security alerts.
  - **Fix:** Add a `# TEST CREDENTIALS ONLY — DO NOT REUSE IN ANY REAL ENVIRONMENT` comment at the top of both `htpasswd` and `radicale.config`. Add both files to a `.gitleaks.toml` or equivalent allowlist with justification. Note in README that these are deliberately weak test credentials.

---

## Minor Suggestions
Smaller improvements and optimizations.

- **[Completeness]** No test scenarios for Unicode or special-character task names
  - **Description:** All example tasks use ASCII descriptions. CalDAV stores iCal in UTF-8 and TaskWarrior has its own encoding handling. Edge cases like emoji, accented characters, or quotes in task names are not tested.
  - **Fix:** Add one scenario (S-64 or extend S-60) for a task description containing non-ASCII characters (e.g., "Tâche avec des accents et émojis 🎯") to verify round-trip encoding integrity.

- **[Architecture]** `FROM debian:bookworm-slim` Python pip install without pinning minor versions
  - **Description:** The Dockerfile pins major versions (`robotframework==7.*`) but wildcard minor versions mean silent upgrades to `7.5` from `7.0` can change behavior.
  - **Fix:** Pin exact versions in the Dockerfile (e.g., `robotframework==7.0.1 icalendar==5.0.12 requests==2.31.0`) and record a `pip freeze` output as a comment or in a `requirements.txt` file for auditability.

- **[Verification]** `verify-3-1` only checks RF version, not Radicale connectivity
  - **Description:** The Phase 3 verification runs `robot --version` inside the container. This confirms the image builds but does not verify that the robot container can actually reach Radicale at `http://radicale:5232`.
  - **Fix:** Change `verify-3-1` to run a minimal RF test that calls `Create Collection` on the live Radicale and then `Delete Collection`. This is a true end-to-end smoke test of the network path.

- **[Completeness]** S-22 large dataset (20 tasks) has no performance expectation
  - **Description:** S-22 tests stability with 20 tasks but defines no time budget. A correct but 10-minute sync would "pass" the test while indicating a serious performance problem.
  - **Fix:** Add a Robot `[Timeout]` directive to S-22 (e.g., `[Timeout] 2 minutes`) so the test fails if stability sync of 20 items takes too long.

- **[Architecture]** Temp config file cleanup happens at Test Teardown, not immediately after sync
  - **Description:** `common.robot`'s `Run Caldawarrior Sync` keyword "Deletes temp config after capture" but the surrounding prose implies this happens in Teardown. If a test fails mid-execution, the config file (containing credentials) remains on the filesystem until the next teardown cycle.
  - **Fix:** Use a `Run Keyword And Return` with a `[Teardown]` inside `Run Caldawarrior Sync` itself, or use Python's `tempfile` module with a context manager to guarantee cleanup regardless of test outcome.

- **[Data Model]** `.taskrc` UDA configuration not validated against caldawarrior's expectations
  - **Description:** `task-4-2` writes `uda.caldavuid.type=string` and `uda.caldavuid.label=CaldavUID` into `.taskrc`. The label string `CaldavUID` is not sourced from Phase 1 investigation — it's guessed. If caldawarrior expects a different label or a different UDA key name, `TW Task Should Have Caldavuid` will always fail.
  - **Fix:** Add UDA names and labels to the `task-1-1` acceptance criteria explicitly: "caldavuid UDA key name and type verified against caldawarrior source."

- **[Interface Design]** `LAST_STDOUT`/`LAST_STDERR` variables have no initialization guard
  - **Description:** `common.robot` sets `LAST_STDOUT`, `LAST_STDERR`, `LAST_EXIT_CODE` after each sync call, but if a test accidentally reads these before any sync is run, it will get an uninitialized variable reference error (or worse, a value from the previous test).
  - **Fix:** Initialize all three variables to `${EMPTY}` / `${-1}` in Suite Setup, and optionally reset them in Test Teardown to prevent cross-test contamination.

- **[Completeness]** No mention of a `tests/robot/docs/` directory creation task
  - **Description:** Both `GAP_ANALYSIS.md` and `CATALOG.md` reference the path `tests/robot/docs/` but no task explicitly creates this directory in the repository structure. It will be created implicitly when the files are written, but this is an implicit dependency.
  - **Fix:** Add creating the `tests/robot/docs/` directory (with a `.gitkeep`) to the Phase 1 task or add an explicit "Create directory structure" task at the start of Phase 2.

---

## Questions
Clarifications needed or ambiguities to resolve.

- **[Architecture]** How does caldawarrior handle multiple `[[calendar]]` entries, and does the test suite need to exercise project routing?
  - **Context:** S-04 tests "Project Routing to Default Calendar" but only tests the default case. The config in `common.robot` specifies a single `[[calendar]]` with `project=default`. If caldawarrior supports per-project calendars, a test with a task having a non-default project exercises a different code path entirely.
  - **Needed:** Clarification on whether multi-calendar routing should be tested (and thus a second Radicale collection is needed in common.robot's setup).

- **[Data Model]** What is the exact LWW (Last-Write-Wins) timestamp source used by caldawarrior?
  - **Context:** S-10 through S-13 rely on LWW conflict resolution, but the spec never defines what timestamp caldawarrior uses: the TaskWarrior `modified` field, the CalDAV ETag, the VTODO `LAST-MODIFIED` property, or wall-clock time at sync.
  - **Needed:** Knowing the timestamp source is essential to writing reliable LWW tests — particularly S-10 and S-11, which need to ensure the "winning" side has a definitively newer timestamp. If the resolution is ETag-based, modifying a VTODO and immediately syncing could race against server-side ETag generation.

- **[Architecture]** Is the `caldawarrior` binary expected to be idempotent within a single sync run?
  - **Context:** S-12 runs two sequential syncs and asserts the second produces zero writes. But if caldawarrior increments a sequence number or modifies `LAST-MODIFIED` as part of the sync process itself, the second sync will always see "changes" and never produce zero writes.
  - **Needed:** Confirmation from reading the source (Phase 1) that sync operations are truly idempotent, or a note that the zero-write test depends on a specific quiescence condition.

- **[Completeness]** How should the test suite behave when run on a host with an already-running Radicale or TaskWarrior installation?
  - **Context:** The spec says "No host Rust, Python, or TaskWarrior required" but doesn't address port conflicts. If port 5232 is already bound on the host, `docker compose up` will fail. The robot container uses `TASKDATA` env var to isolate TW, but the host port forwarding for Radicale could conflict.
  - **Needed:** Clarify whether Radicale port is exposed to host at all (it doesn't need to be — robot container accesses it via Docker network). If it is exposed, consider using a dynamic or non-standard port.

- **[Interface Design]** What is the expected behavior of `Run Caldawarrior Sync` when caldawarrior exits non-zero?
  - **Context:** S-52 and S-53 expect exit code 1. `common.robot`'s keyword captures stdout/stderr/returncode, but Robot Framework's `Run Process` raises an exception by default on non-zero exit. If the keyword uses `Run Process` without `return_rc=True`, it will fail before setting `LAST_EXIT_CODE`.
  - **Needed:** Explicit statement that the keyword captures non-zero exit codes without raising (using `return_rc=True` or equivalent) so error-path tests can assert on exit code.

- **[Data Model]** How does S-32 (CANCELLED → TW deleted) interact with TW's soft-delete mechanism?
  - **Context:** TaskWarrior marks tasks as "deleted" with `status:deleted` but keeps them in `pending.data`. The test asserts "TW task created with status deleted," but subsequent syncs might re-create the CalDAV VTODO if caldawarrior interprets a deleted TW task as "to be re-pushed."
  - **Needed:** Clarification on how caldawarrior handles TW deleted tasks during sync — are they excluded from push? This affects both S-32 and S-21 correctness.

---

## Praise
What the spec does well.

- **[Architecture]** Multi-stage Dockerfile with BuildKit cache mounts is well-designed
  - **Why:** Separating the Rust builder stage from the lean runner stage keeps the final image small while the `--mount=type=cache` for cargo registry and build target dramatically speeds up iterative development. This is the correct Rust-in-Docker pattern and explicitly avoiding the Rust toolchain in the runner is correctly called out in the acceptance criteria.

- **[Completeness]** Traceability chain from user story → catalog → test case name is excellent
  - **Why:** Requiring test case names in CATALOG.md to match `.robot` file names exactly, combined with the ID-range numbering scheme with reserved gaps, creates a genuinely maintainable living document. The explicit convention that ranges have gaps (for future expansion) shows forward-thinking design.

- **[Verification]** Phased dependency structure is logical and well-sequenced
  - **Why:** The strict ordering (Assessment → Catalog → Infrastructure → Libraries → Suites → Documentation) ensures that each phase produces verified artifacts that the next phase depends on. The gap analysis producing ground-truth output formats before any test assertions are written is especially sound — it prevents the common failure of tests that assert against guessed output strings.

- **[Architecture]** Non-root container user for testing file permission behavior is clever
  - **Why:** Running as `testrunner` (non-root) means S-55 (config file permission warning) is reliable and non-trivially checkable — a root user would bypass filesystem permission checks. This is a subtle but important design choice that ensures the permission test has real meaning.

- **[Interface Design]** CalDAVLibrary using `icalendar` library (not regex) for VTODO mutation is explicitly enforced
  - **Why:** The acceptance criterion "Modify VTODO Summary correctly round-trips via icalendar library not regex" prevents a common testing antipattern where tests use brittle string manipulation to construct iCal data, leading to invalid iCal that only the test environment accepts.

- **[Completeness]** Both white-box (Rust) and black-box (Robot Framework) test layers are clearly distinguished
  - **Why:** Documenting both test layers in the README and Makefile with separate `make` targets gives contributors a clear mental model of what each test layer covers. The decision to keep them separate (rather than trying to replace one with the other) reflects accurate understanding of their different purposes.

- **[Architecture]** Isolated TW state via `TASKDATA` and `TASKRC` environment variables on every subprocess call
  - **Why:** Setting both `TASKDATA` and `TASKRC` (not just one) in all subprocess calls is the correct way to fully isolate TaskWarrior from the host system. Many implementations miss `TASKRC` and inherit host configuration that can silently alter behavior. Making this an explicit acceptance criterion is exactly right.

---
*Generated by Foundry MCP Spec Review*