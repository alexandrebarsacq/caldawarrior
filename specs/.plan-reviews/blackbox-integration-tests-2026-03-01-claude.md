# Review Summary

## Critical Blockers
Issues that MUST be fixed before this becomes a spec.

- **[Architecture]** iCal parsing library absent from Docker image specification
  - **Description:** `CalDAVLibrary.py` requires `Modify VTODO Summary` and `Modify VTODO Status` to implement a GET→parse→mutate→PUT cycle, but the Dockerfile only installs `robotframework requests` via pip. Parsing and reconstructing iCal content requires a dedicated library (`icalendar` or `vobject`). String manipulation on raw iCal text is brittle and will fail on multi-line folded fields, Unicode, or generated timestamps. This gap affects at minimum S-10, S-11, S-13, S-30, S-31, S-32, and S-40.
  - **Impact:** Multiple keyword implementations will be impossible to write correctly without this, or will be written using fragile regex hacks that silently corrupt test VTODOs, producing false negatives.
  - **Fix:** Add `icalendar` (the maintained PyPI package) to the pip install line in the Dockerfile task description: `pip install robotframework requests icalendar`. Update the CalDAVLibrary description to note it uses `icalendar` for parsing. Also add `icalendar` to the Dependencies section.

- **[Architecture]** Radicale startup race condition in docker-compose
  - **Description:** The `docker-compose.yml` task specifies `depends_on: radicale`, but Docker Compose's `depends_on` only waits for the container to *start*, not for the Radicale HTTP server to be *accepting connections*. The `robot` container will frequently attempt its Suite Setup before Radicale's TCP port is open, causing the first test suite's `Create Collection` to fail with a connection error.
  - **Impact:** Intermittently flaky test results from the very first run, especially in CI environments where container startup is slower. This undermines the reliability of the suite.
  - **Fix:** Add a `healthcheck` to the `radicale` service and use `depends_on: radicale: condition: service_healthy` in the robot service. Example healthcheck: `test: ["CMD", "curl", "-f", "-u", "testuser:testpassword", "http://localhost:5232/"]`, `interval: 2s`, `retries: 10`. Add this to the docker-compose task's acceptance criteria.

- **[Clarity]** Docker build context and Dockerfile path are contradictory
  - **Description:** The docker-compose task states: build from `./Dockerfile` with build context `../..` (repo root). In Docker Compose, when `build.dockerfile` is specified, it is resolved *relative to the build context*. If the context is the repo root (`../..` from `tests/robot/`), then `dockerfile: ./Dockerfile` resolves to `<repo-root>/Dockerfile` — which does not exist. The actual file is `tests/robot/Dockerfile`.
  - **Impact:** `docker compose build` will fail with "Dockerfile not found" before a single test can run.
  - **Fix:** Change the dockerfile field to `dockerfile: tests/robot/Dockerfile` (relative to the repo-root context). The acceptance criteria should include a `docker compose build` step that explicitly confirms the build succeeds from a clean state.

---

## Major Suggestions
Significant improvements to strengthen the plan.

- **[Architecture]** `TASKRC` environment variable not set alongside `TASKDATA`
  - **Description:** The `Set TW Data Dir` keyword sets `TASKDATA` for isolation, and the task description mentions writing a `.taskrc` into that directory. However, TaskWarrior also reads `TASKRC` from the environment (defaulting to `~/.taskrc`). If the runner container image has a system-level `.taskrc` (possible after `apt install taskwarrior`), it will be loaded in addition to the per-test one, potentially overriding UDA configuration or adding conflicting hooks.
  - **Impact:** `caldavuid` UDA may be silently ignored if the system `.taskrc` takes precedence, causing `TW Task Should Have Caldavuid` to fail spuriously.
  - **Fix:** Update the `Set TW Data Dir` keyword description to also set `TASKRC=<path>/.taskrc` in the subprocess environment. Alternatively, add `ENV TASKRC=/dev/null` to the Dockerfile runner stage and construct the full taskrc path dynamically in the keyword.

- **[Sequencing]** Phase 2 catalog output patterns are provisional, but the plan treats them as authoritative
  - **Description:** Phase 1 is explicitly static analysis only ("Read `src/output.rs` fully") — it cannot execute the binary. Phase 2 then commits catalog entries to "verified" output patterns sourced from Phase 1. But static analysis of format strings in Rust code can miss runtime interpolation, macro expansion quirks, or conditional branches. Phase 5 test execution will be the first time actual runtime output is observed.
  - **Impact:** If even 3–4 stdout patterns in the catalog are wrong, Phase 5 produces a cascade of failures across multiple suites. Phase 6's "fix failures" task will balloon into an undocumented Phase 2 revision cycle, with no tracking.
  - **Fix:** Add an explicit note in Phase 2's task description: *"Patterns are provisional pending Phase 5 execution; if Phase 5 reveals a mismatch, update CATALOG.md before marking the test `skip-unimplemented`."* Also add a task in Phase 6: *"Reconcile CATALOG.md output patterns against actual Phase 5 execution evidence."* This makes the provisional nature explicit rather than implicit.

- **[Architecture]** Temporary config.toml has no cleanup strategy
  - **Description:** `Run Caldawarrior Sync` in `common.robot` generates a temp `config.toml` for each sync invocation. The description doesn't specify where this file is written, what it's named, or who deletes it. Over a 30-scenario run, this produces 30+ orphaned temp files in an unspecified location.
  - **Impact:** Successive runs accumulate stale configs; if the temp path collides between tests (e.g., using a fixed `/tmp/caldawarrior-test.toml`), tests can inherit a prior test's configuration silently.
  - **Fix:** Specify that temp configs are written to a per-test path (e.g., `/tmp/cw-config-${TEST NAME}.toml`) and that `Test Teardown` deletes them. Add this to the `common.robot` acceptance criteria: *"No temp config files remain after teardown."*

- **[Risk]** `S-55` permission test is structurally ineffective in Docker without a non-root user
  - **Description:** The plan's mitigation is "skip if running as root," but the Dockerfile runner stage has no `USER` directive, meaning the container runs as root by default. `chmod 0644 config.toml` does not restrict access for root. The test will always be skipped unless the Dockerfile is changed, making it dead weight across all environments.
  - **Impact:** S-55 effectively never runs. If the permission-warning behavior regresses, this test provides no coverage.
  - **Fix:** Add a `RUN useradd -m testrunner` and `USER testrunner` to the Dockerfile runner stage. Update the Dockerfile acceptance criteria to include *"container runs as non-root user."* This also improves general container security hygiene.

- **[Sequencing]** Phase 3 and Phase 4 can be parallelized but aren't identified as such
  - **Description:** Phase 4 (keyword libraries) does not depend on Docker infrastructure being runnable — the Python and Robot Framework code can be written and reviewed without executing it. Phase 3's Docker build and Phase 4's library implementation are independent work streams. The plan's linear sequencing implies they must be serial.
  - **Impact:** Missed opportunity to reduce calendar time if more than one person is working on this, or to provide earlier feedback on the library design before the Docker environment is ready.
  - **Fix:** Add a note to Phase 4's description: *"Can be developed in parallel with Phase 3 — execution testing requires Phase 3 complete, but authoring does not."* This is informational only; it doesn't change the phase ordering.

---

## Minor Suggestions
Smaller refinements.

- **[Clarity]** `htpasswd` "plain text" format is ambiguous for Radicale configuration
  - **Description:** The task says "htpasswd (testuser:testpassword, plain text)." Radicale supports multiple htpasswd formats (`plain`, `md5`, `bcrypt`). "Plain text" is technically valid in Radicale's `htpasswd_encryption = plain` mode, but someone reading this might generate a bcrypt hash (the htpasswd tool's default), which would then fail authentication silently.
  - **Fix:** Change to: *"htpasswd using `plain` encryption (`testuser:testpassword` literally, with `htpasswd_encryption = plain` in radicale.config)."* Include the exact two-line content of the htpasswd file in the task description.

- **[Completeness]** Task-level dependencies are not explicit between phases
  - **Description:** Per the spec format guidelines, tasks should list dependencies. Phase 5 tasks depend on Phase 4 tasks, Phase 4 depends on Phase 3, etc. Currently this is only implied by phase ordering, not stated at the task level.
  - **Fix:** Add a `Dependencies:` field to Phase 5 tasks listing Phase 4's three tasks by name, and to Phase 4 tasks listing Phase 3. For Phase 1 tasks, note "no dependencies."

- **[Clarity]** Reserved ID ranges between categories are incompletely specified
  - **Description:** The plan mentions "gaps between category ranges are reserved for future scenarios" and lists S-06–S-09 as an example, but the actual gaps are S-06–S-09, S-15–S-19, S-23–S-29, S-34–S-39, S-43–S-49, S-56–S-59. Only the first gap is called out explicitly.
  - **Fix:** Add a table or bullet list in the Phase 2 task description or CATALOG.md header listing all reserved ranges for each category. This makes the convention self-documenting.

- **[Architecture]** `S-22: Large Dataset Stability` is a load test masquerading as a behavioral test
  - **Description:** Twenty tasks is arbitrary and the assertion ("second sync shows 0 writes") is already covered by S-12's stable-sync check on a smaller dataset. This test primarily exercises sync runtime performance, not a distinct behavioral scenario.
  - **Fix:** Reduce to 10 tasks (still clearly "more than trivial") and reframe the acceptance criteria to focus on the behavioral assertion: *"idempotency holds at larger scale, not just a 2-task baseline."* Or merge the large-dataset case into S-12 as a parameterized variant.

- **[Risk]** Python version is not pinned in the Dockerfile
  - **Description:** `apt install python3` installs whatever version Debian bookworm ships (currently 3.11). Future bookworm security updates or a base image change to trixie could shift the Python version and break `icalendar`/`requests` compatibility.
  - **Fix:** Pin the base image tag: `FROM debian:bookworm-slim` is already pinned to a release name (not `latest`), which is good. Add a comment noting the Python version expected: `# python3 = 3.11 on bookworm`. Optionally use `pip install robotframework==7.x icalendar==5.x requests==2.x` with pinned versions.

- **[Completeness]** `results/` volume mount conflict between `./` and `./results` mounts
  - **Description:** The robot service mounts `./` to `/tests` and `./results` to `/results`. Since `./results/` is a subdirectory of `./`, it is technically accessible inside the container as both `/tests/results/` and `/results/`. The robot command writes to `/results` (the dedicated mount), which is correct, but the overlap could confuse contributors.
  - **Fix:** Add a comment in the docker-compose.yml task description: *"The `./results` subdirectory is double-mounted intentionally; always reference `/results` as the output directory, never `/tests/results`."*

---

## Questions
Clarifications needed before proceeding.

- **[Architecture]** How is `config.toml` structured for test runs, and does caldawarrior support per-collection URLs?
  - **Context:** `Run Caldawarrior Sync` generates a "temp config.toml pointing at `${COLLECTION_URL}`." The caldawarrior config format (calendars, credentials, sync direction) is not shown anywhere in the plan. If the config maps projects to calendar URLs, each test suite's UUID-based collection URL must appear as the target for an "unprojectd" or "default" calendar. If the config doesn't support a per-run collection URL this way, the isolation model breaks.
  - **Needed:** A minimal example of the config.toml structure used in tests, showing how `${COLLECTION_URL}` is injected. This could be a code block in the `common.robot` task description.

- **[Architecture]** Do S-41 (RELATED-TO → TW depends) and S-63 (X-TASKWARRIOR-WAIT) rely on features verified to exist in caldawarrior?
  - **Context:** S-41 asserts CalDAV RELATED-TO propagates to TW's `depends` field. S-63 asserts `X-TASKWARRIOR-WAIT` roundtrips. These are nontrivial bidirectional field mappings. If caldawarrior doesn't implement one or both directions, these tests will silently fail with a wrong error message (e.g., "field not found" rather than "feature not implemented"). Phase 1's gap analysis should explicitly cover these, but the Phase 1 task description doesn't call them out.
  - **Needed:** Add explicit checks to Phase 1's task description for: (a) RELATED-TO→depends field mapping in both directions, and (b) X-TASKWARRIOR-WAIT field existence. If either is unimplemented, the corresponding catalog entries should be pre-tagged `skip-unimplemented` in Phase 2 rather than discovered at Phase 5 failure time.

- **[Clarity]** Is `docker compose` (v2, plugin) or `docker-compose` (v1, standalone) the assumed runtime?
  - **Context:** The plan consistently uses `docker compose` (space, v2 syntax), which is the current standard. However, `make test-robot` in Phase 6 will invoke one or the other. Many CI environments still have v1 installed.
  - **Needed:** Explicitly state the minimum required docker compose version (e.g., "Docker Compose v2.x plugin required") in the Constraints section and in the README task. This prevents a common "command not found" failure in CI.

- **[Architecture]** What is the expected behavior when caldawarrior is run against an empty config (no calendars configured)?
  - **Context:** Several tests create a minimal config pointing at a UUID collection. If caldawarrior requires at least one calendar/project mapping in the config to run successfully, a "bare minimum" config may produce an error exit rather than the "0 synced" result the tests expect for scenarios with no tasks.
  - **Needed:** Either document the minimal valid config in the `common.robot` task, or add a note in Phase 1 confirming that caldawarrior handles a single default-calendar config without requiring project mappings.

---

## Praise
What the plan does well.

- **[Sequencing]** Phase 1 static analysis before any test authoring
  - **Why:** Requiring exact stdout/stderr patterns to be sourced from `src/output.rs` before encoding them in `.robot` files is disciplined. It prevents the common failure mode of test suites built on assumed output formats that slowly drift from reality. Separating this into a dedicated verification phase with its own acceptance criteria ("traceable to a line in output.rs") is particularly strong.

- **[Risk]** Risk table is realistic and pre-emptively mitigated
  - **Why:** The five risks are genuinely the right risks for this type of project (slow builds, TW version drift, isolation failures, format assumptions, unimplemented behaviors). The mitigations are concrete and build-in rather than deferred. The decision to use `SKIP` tagging with comments rather than deleting unimplemented tests is an excellent operational choice — it makes gaps visible in the HTML report rather than hiding them.

- **[Architecture]** Test isolation model is well-reasoned
  - **Why:** UUID-based CalDAV collections per suite + per-test TW data wipe is a clean hybrid that balances isolation cost against isolation completeness. Having Suite Teardown delete the collection (not just individual VTODOs) ensures no cross-suite leakage even if individual test teardowns fail midway.

- **[Completeness]** Living CATALOG.md as the primary spec document is an excellent architectural decision
  - **Why:** By making CATALOG.md the authoritative source (user story → catalog entry → `.robot` test case name, with status markers in Phase 6), the plan creates a self-maintaining traceability artifact. This is far better than the common pattern where test intent is only readable by parsing Robot Framework syntax.

- **[Completeness]** Open Questions are answered inline with proposed resolutions
  - **Why:** Rather than leaving questions genuinely open, each has a "Proposed: yes/copy/gitignore" answer. This pattern moves reviewers from "this is unclear" to "do you agree with this decision" — a much more efficient review conversation.

- **[Clarity]** The worked example entry for CATALOG.md (S-01) is concrete and complete
  - **Why:** Including an exact format example with user story, setup, expected outcome (stdout pattern, exit code, CalDAV state), and Robot test case name eliminates ambiguity about what "a complete scenario entry" means. The example format is detailed enough to be templated directly.

- **[Architecture]** Clear separation of two test layers is articulated from the start
  - **Why:** Explicitly framing this as "a second test layer on top of existing Rust integration tests (do not replace them)" prevents scope creep and protects existing coverage. The constraint "Must not modify `tests/integration/`" enforces this structurally.