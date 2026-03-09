# Review Summary

## Critical Blockers
Issues that MUST be fixed before this becomes a spec.

- **[Completeness] Missing Per-Task Dependencies**
  - **Description:** The plan outlines tasks for each phase but omits explicit dependency declarations (e.g., `Dependencies: [...]`) at the individual task level.
  - **Impact:** Automated implementation tools (like `foundry-implement`) rely on these explicit task-level dependencies to sequence work correctly. Without them, tasks might be executed out of order, causing implementation failures (e.g., attempting to write tests before the keyword libraries exist).
  - **Fix:** Add a `Dependencies:` field to every task definition. For example, Phase 5 suite implementation tasks should explicitly depend on Phase 4 keyword library tasks.

## Major Suggestions
Significant improvements to strengthen the plan.

- **[Architecture] Slow Docker Build / Missing Rust Caching**
  - **Description:** The Phase 3 multi-stage Dockerfile runs `cargo build --release` from scratch. Without caching, this will download the crate index, download all dependencies, and compile the entire binary from scratch on every run.
  - **Impact:** The feedback loop for running these tests will be painfully slow (minutes per run), severely degrading developer experience and discouraging frequent local testing.
  - **Fix:** Specify the use of Docker BuildKit cache mounts (`--mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/build/target`) in the `Dockerfile` task, or explicitly propose using `cargo-chef` to cache the dependency layer.

- **[Sequencing] Missing Makefile Task**
  - **Description:** Phase 6 tasks mention adding `make test-robot` to the `README.md` and verifying that it works, but there is no specific task allocated to actually create or modify the `Makefile`.
  - **Impact:** The Phase 6 acceptance criteria for the README task ("`make test-robot` works") will fail because the implementation step to define the target was skipped.
  - **Fix:** Add a distinct task in Phase 6 to update/create the `Makefile` and define the `test-robot` and `test-integration` targets.

- **[Risk] Docker Volume Permission Issues on Linux**
  - **Description:** The `robot` service in `docker-compose.yml` mounts `./results:/results`. Since the `debian:bookworm-slim` container runs as root by default, the generated HTML/XML result files will be owned by `root` on Linux host machines.
  - **Impact:** Causes friction for Linux users who will encounter "Permission denied" errors when trying to clean up, edit, or delete the test artifacts on their host machine.
  - **Fix:** Add a requirement in Phase 3 to configure the `robot` service in `docker-compose.yml` to use the host's user ID (e.g., `user: "${UID:-1000}:${GID:-1000}"`) or include a test teardown step that `chown`s the directory back to the host user.

## Minor Suggestions
Smaller refinements.

- **[Architecture] Robust TaskWarrior JSON Export**
  - **Description:** Phase 4 `TaskWarriorLibrary.py` mentions getting a "dict from task export". TaskWarrior's export format can be influenced by local configurations.
  - **Fix:** Explicitly state in the task that the subprocess call should use `task export rc.json.array=on <uuid>` to guarantee valid JSON array output regardless of any underlying configuration quirks.

- **[Clarity] Dynamic Config Generation**
  - **Description:** Phase 4 `Run Caldawarrior Sync` mentions generating a temp `config.toml`. 
  - **Fix:** Explicitly note that this dynamically generated config must interpolate the `${COLLECTION_URL}` created during Suite Setup to ensure the suite-level isolation functions correctly.

## Questions
Clarifications needed before proceeding.

- **[Architecture] Timezone Handling**
  - **Context:** Date and time operations between TaskWarrior and CalDAV can be highly sensitive to local timezones, leading to flaky tests across different environments.
  - **Needed:** Should the `docker-compose.yml` explicitly set the `TZ=UTC` environment variable for both `radicale` and `robot` containers to guarantee deterministic test execution regardless of the host machine's local timezone?

- **[Clarity] Unimplemented Behavior Fallbacks**
  - **Context:** Phase 4 mentions `S-33` checking for recurring warnings, and suggests skipping it if not implemented.
  - **Needed:** Should Phase 1's `GAP_ANALYSIS.md` explicitly dictate which tests get the `skip-unimplemented` tag before Phase 5 begins, or is the developer expected to figure that out during Phase 5?

## Praise
What the plan does well.

- **[Architecture] State Isolation Strategy**
  - **Why:** Creating a UUID-based Radicale collection per suite and wiping the TaskWarrior data directory per test is an exceptional strategy. It guarantees complete test isolation, prevents state leakage, and eliminates order-dependent flakiness.

- **[Clarity] Documentation-Driven Testing**
  - **Why:** Establishing a living Scenario Catalog (`CATALOG.md`) before writing any test code ensures the suite is strictly tied to user requirements. It also makes the test suite highly accessible to non-technical stakeholders and provides a clear checklist for implementation.