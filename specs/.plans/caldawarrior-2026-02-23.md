# caldawarrior

## Mission

Build a bidirectional CLI synchronization tool that keeps TaskWarrior tasks and CalDAV VTODO items consistent, using the CalDAV UID stored as a TaskWarrior UDA as the sole join key — requiring no intermediate sync database.

## Objectives

- Implement bidirectional sync where changes in either TaskWarrior or CalDAV propagate to the other system on each sync run
- Provide a no-sync-database architecture: the `caldavuid` UDA on each TW task is the only state needed to link it to its CalDAV counterpart
- Map each TaskWarrior project 1:1 to a CalDAV calendar; multi-server support is out of scope for v1
- Implement Last Write Wins conflict resolution at task-level granularity using modification timestamps, with CalDAV-side wins on ties
- Sync task dependencies (TW `depends` ↔ CalDAV `RELATED-TO;RELTYPE=DEPENDS-ON`) with DFS cycle detection
- Provide a `--dry-run` mode that previews all planned changes without committing any writes
- Deliver comprehensive unit tests and Docker-based integration tests against a real CalDAV server (Radicale)

## Success Criteria

- [ ] `caldawarrior sync` performs a full bidirectional sync; a task created in TW appears in CalDAV and vice versa
- [ ] `caldawarrior sync --dry-run` prints all planned operations and exits with no writes committed
- [ ] All status mappings (pending/waiting/completed/deleted/recurring ↔ NEEDS-ACTION/COMPLETED/CANCELLED) work in both directions per the spec
- [ ] All field mappings (scheduled/wait/due/end/depends) round-trip correctly without perpetual re-write loops
- [ ] X-TASKWARRIOR-WAIT custom property is written and read correctly; expired wait dates collapse to `pending`
- [ ] Dependency edges are synced bidirectionally; cycles are detected by DFS and excluded from write-back with a warning
- [ ] LWW conflict resolution picks the newer side; CalDAV wins on timestamp tie
- [ ] Orphaned `caldavuid` on a TW task (VTODO deleted from CalDAV) is handled correctly without re-creation loop
- [ ] Unit test suite passes (status mapper, field mapper, LWW resolver, IR builder, cycle detector)
- [ ] Docker integration tests pass against Radicale CalDAV server
- [ ] All v1 known limitations are documented and warned about at runtime

## Assumptions

- TaskWarrior is installed and accessible via CLI (`task export`, `task import`)
- Target CalDAV server supports RFC 4791 (CalDAV) and RFC 5545 (iCalendar VTODO)
- The `caldavuid` UDA is registered in TaskWarrior on first run by the tool automatically
- Tasks belong to at most one project at a time (no multi-project tasks)
- The tool is run by one user at a time; concurrent invocations are out of scope for v1
- All timestamps are stored and compared in UTC
- A single CalDAV server is used for all calendars (multi-server is v2 scope)
- The IR loads all configured calendars in one pass before Step 2 begins; `IREntry` carries its source `calendar_url`
- Partial sync failure (network error mid-write) is acceptable; idempotent retries on next run converge correctly

## Constraints

- No intermediate sync database: the `caldavuid` UDA is the only persistent link state
- LWW resolution is task-level, not field-level (v1 constraint; field-level merging is v2)
- `recurring` TW tasks are not synced in v1 (skipped with warning); CalDAV VTODOs with `RRULE` are also skipped with a `RecurringCalDavSkipped` warning
- **CalDAV-only CANCELLED VTODOs are never imported as new TW tasks** (prevents importing historical CANCELLED items; round-trip hazard). A both-exist task whose CalDAV counterpart transitions to CANCELLED IS treated as a deletion signal for the existing TW task.
- The tool must not hard-delete a CalDAV VTODO when a TW task is deleted: use CANCELLED status to signal deletion safely
- A TW-only entry with `caldavuid` set but no matching CalDAV VTODO (orphaned UID) must NOT recreate the VTODO; it must treat the absence as a CalDAV-deletion signal: run `task <uuid> delete` and clear `caldavuid` UDA
- "DELETE TW task" throughout means `task <uuid> delete` (moves to deleted state) followed by clearing the `caldavuid` UDA. Purge is never performed by the tool.
- "Content identical" for the no-spurious-write check means: SUMMARY, STATUS, DUE, DTSTART, COMPLETED, DESCRIPTION, RELATED-TO[DEPENDS-ON], and X-TASKWARRIOR-WAIT all match after normalization. Modification timestamps are excluded from this comparison. COMPLETED is compared at second precision as `DateTime<Utc>`.
- HTTP 412 ETag conflicts are handled at the orchestrator layer (not adapter layer): the adapter returns `CaldaWarriorError::EtagConflict { refetched_vtodo }` and the orchestrator retries LWW (max 3 attempts)
- A TW task whose project has no matching calendar config entry is assigned to the `default` calendar with a `UnmappedProject` warning. If no `default` calendar is configured, the task is skipped with this warning.
- VTODOs from the `default` calendar produce TW tasks with no project field set (not `project: "default"`)
- CalDAV VTODOs with `RRULE` are detected via a named `rrule: Option<String>` field on the VTODO struct (never `None` on writes; the tool never creates recurring items); presence of this field triggers `RecurringCalDavSkipped` warning and skips the VTODO
- All timestamps in iCalendar are normalized to UTC: TZID-aware timestamps are converted via `chrono-tz`; UTC (trailing `Z`) parsed directly; floating timestamps treated as UTC with a logged warning
- `TwAdapter.list_all()` uses **two separate `task export` invocations** to avoid excluding long-stable active tasks: (1) `task export '(status:pending or status:waiting or status:recurring)'` with NO date filter — ALL active tasks always included; (2) `task export '(status:completed or status:deleted) and status.not:purged and modified.after:<cutoff_date>'` — completed/deleted tasks limited to `completed_cutoff_days` window. Results are merged. This prevents the duplicate-creation loop that would occur if active tasks older than the cutoff window were absent from the IR.
- `completed_cutoff_days` (default: 90) is a v1 config key added to `src/config.rs`; it limits ONLY completed/deleted tasks; pending/waiting tasks are never filtered by date
- `TwAdapter.delete(uuid)` runs `task <uuid> delete` then `task <uuid> modify caldavuid:` as two sequential Command calls. These are NOT transactional — stale caldavuid on second-command failure is benign because a task in `deleted` state with an orphaned caldavuid is handled correctly (AlreadyDeleted SKIP branch) on the next sync
- Per-item errors within a CalDAV 207 multi-status body (e.g., per-`<D:response>` status 404) are logged as `Warning::CalDavItemError { uid, status }` and skipped; they do not abort `list_vtodos()`
- `SyncResult` includes an `errors: Vec<(String, CaldaWarriorError)>` field (keyed by caldav_uid) for entries that fail after all retries; a `SyncConflict` on one entry does not abort the sync
- All `task` CLI invocations use `std::process::Command` with argument lists (never shell string interpolation); this is a security requirement
- iCalendar library evaluation criteria for Phase 0: must support (1) VCALENDAR wrapper on serialize, (2) unknown-property round-trip preservation, (3) RELTYPE parameter on RELATED-TO, (4) line unfolding on parse, (5) RFC 5545 TEXT escaping. If the `icalendar` crate fails any criterion, use a custom line parser.
- RFC 5545 TEXT escaping (§3.3.11) is required: SUMMARY and DESCRIPTION must escape `\` as `\\`, `,` as `\,`, `;` as `\;`, and literal newlines as `\n` on serialize; reverse on parse
- RFC 5545 line folding (§3.1) is required: lines exceeding 75 octets are folded (CRLF + SPACE) on serialize; fold-continuation lines are recombined on parse
- DTSTAMP is a required VTODO field (RFC 5545 §3.6.2): **On write:** DTSTAMP is always set to the current UTC wall clock (`Utc::now()`); the server's DTSTAMP value is never copied into outgoing VTODO writes. **On read (LWW comparison only):** the server's DTSTAMP as received in `list_vtodos()` IS used as a fallback timestamp when LAST-MODIFIED is absent (this is reading server state for timestamp comparison, not for a write path). This distinction is documented at the LWW fallback site in `src/sync/lww.rs`.
- XML parsing library for CalDAV 207 responses must be evaluated in Phase 0 (candidates: `quick-xml`, `roxmltree`, `minidom`); the choice is documented in `docs/adr/xml-parser.md`
- Implementation language is Rust; use `thiserror` for library errors and `anyhow` only at the CLI boundary
- Credential storage in TOML config file must be documented with `0600` permissions warning; `CALDAWARRIOR_PASSWORD` env-var override must be supported

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `task import` mutates `modified` timestamp, causing LWW re-write loop | high | high | Store a `X-CALDAWARRIOR-LAST-SYNC` custom property on each VTODO with the sync epoch; only trigger LWW when `modified` is newer than last-sync epoch. Verify empirically in Phase 0. |
| First-sync duplicates if same task pre-exists in both systems | medium | medium | Document clearly; recommend treating one system as authoritative before first sync |
| LWW tie (identical modification timestamps) | low | low | Tiebreaker: CalDAV side wins; documented in spec |
| CalDAV server non-compliance with RFC 4791 / RFC 5545 | low | high | Test against Radicale (primary) and Baikal; document tested servers |
| Dependency cycle introduced by CalDAV client | medium | medium | DFS cycle detection in Step 2; cyclic tasks excluded from write-back with warning |
| Cross-calendar dependency references unresolvable | low | low | Drop with per-task warning; document as known limitation |
| Plaintext credentials in config file | medium | medium | Require `0600` file permissions; support `CALDAWARRIOR_PASSWORD` env-var override; add keyring crate to v2 roadmap |
| Concurrent CalDAV client write during sync (lost update) | medium | medium | Use `If-Match: <etag>` on PUT/DELETE; on HTTP 412, adapter returns `EtagConflict` error with refetched VTODO; orchestrator retries LWW (max 3 attempts) |
| Large TW database causes slow or OOM sync | medium | low | Add optional `completed_cutoff_days` config key (default: 90) to limit `task export` to recently modified tasks; document trade-off |
| `task restore` of a deleted TW task with stale `caldavuid` immediately re-deleted | low | medium | Document as v1 limitation: restoring a TW task that was deleted by the sync tool will re-trigger deletion on next sync. Mitigation: clear `caldavuid` before restoring. |
| `task import` creates duplicates on UUID collision instead of updating in place | high | critical | Empirically verified in Phase 0 research item #2; if true, replace update path with `task <uuid> modify` CLI for field updates and `task import` only for new-task creation. |

## Open Questions

- Does `task import` mutate the `modified` timestamp in practice? (Must verify empirically in Phase 0 before finalizing Phase 3 LWW approach)
- Should the `caldawarrior` config support multiple CalDAV servers? (Decision: out of scope for v1; add to constraints)
- Should `caldawarrior` register the `caldavuid` UDA automatically on first run? (Recommendation: yes, auto-register)
- What is the configuration format? (Decision: TOML at `~/.config/caldawarrior/config.toml` with env-var overrides)
- What action should be taken for a TW task in `deleted` state with an orphaned `caldavuid`? (Decision: it is already deleted; treat as no-op and do not purge from TW database unless `task purge` is explicitly requested by user)
- Should v1 support RFC 6578 sync tokens for incremental fetch when the server advertises support? (Recommendation: defer to v2; design `list_vtodos()` interface as replaceable)
- Should v1 support system keychain via the `keyring` crate for credential storage? (Recommendation: defer to v2; document `0600` config file permissions for v1)

## Dependencies

- TaskWarrior CLI (`task` binary)
- A CalDAV server reachable over HTTP/HTTPS (tested against Radicale and Baikal)
- Rust toolchain (cargo, rustc) for building
- Docker and docker-compose for integration tests
- Key Rust crates (to be confirmed in Phase 0): `reqwest` (blocking, TLS), `icalendar` or custom parser, `quick-xml`/`roxmltree`/`minidom` (CalDAV 207 XML), `clap` (derive), `serde`/`serde_json`/`toml`, `thiserror`, `tracing`/`tracing-subscriber`, `uuid` (v4), `chrono`/`chrono-tz` (timezone normalization), `percent-encoding` (URL encoding for PUT paths)

## v1 Known Limitations

1. `waiting` tasks are invisible to third-party CalDAV clients (stored in opaque `X-TASKWARRIOR-WAIT`)
2. `IN-PROCESS` CalDAV status is demoted to `NEEDS-ACTION` on round-trip (TW has no in-progress concept)
3. `CANCELLED`-only CalDAV items are never imported as new TW tasks
4. `recurring` TW tasks are not synced (skipped with warning)
5. CalDAV VTODOs with `RRULE` (server-native recurring items) are skipped on import with a warning
6. LWW is task-level, not field-level (a changed field on the losing side is silently overwritten)
7. Cross-calendar dependency links are dropped with a warning
8. Non-`DEPENDS-ON` RELATED-TO relationship types (PARENT, CHILD, SIBLING) are silently ignored
9. First-sync may create duplicates if the same task was independently created in both systems
10. Restoring a TW-deleted task with stale `caldavuid` will re-trigger deletion on next sync (clear `caldavuid` first)
11. If the process crashes after a CalDAV VTODO is created but before the TW `caldavuid` UDA is saved, a dangling VTODO remains in CalDAV and will be duplicated on the next sync — requires manual cleanup
12. Moving a TW task between projects does not migrate its VTODO between CalDAV calendars. The VTODO remains in its original calendar until deleted manually.
13. First sync will create CalDAV VTODOs for all completed TW tasks within the `completed_cutoff_days` window (default: 90 days). For large TW databases, run `caldawarrior sync --dry-run` first to preview the volume; consider setting `completed_cutoff_days = 7` for the initial sync.
14. If an entry fails after 3 ETag conflict retries, it is skipped for this sync run. Re-running sync will retry and typically converge once the concurrent write has settled. ETag-failed entries are logged to stderr with their caldav_uid before sync concludes.

## Phases

### Phase 0: Project Scaffolding & Empirical Research

**Goal:** Initialize the Rust project, select and pin dependencies, verify key behavioral assumptions (especially `task import` timestamp mutation), and set up CI.

**Description:** Bootstrap the project with `cargo new`, select and add all required Rust crates, set up GitHub Actions CI, and run an empirical test to confirm whether `task import` mutates the `modified` timestamp. This finding is critical for the Phase 3 LWW design.

#### Tasks

- **Initialize Rust project and select dependencies** `implementation` `small`
  - Description: Run `cargo new caldawarrior`. Add to `Cargo.toml`: `reqwest` (blocking, TLS), `clap` (derive feature), `serde`/`serde_json` (TW JSON), `toml`/`serde`, `thiserror`, `tracing`/`tracing-subscriber`, `uuid` (v4 feature), `chrono`/`chrono-tz` (timezone normalization), `percent-encoding`, and libraries to evaluate: iCalendar (`icalendar` crate or custom), XML parser (`quick-xml`, `roxmltree`, or `minidom`). Set up GitHub Actions workflow: `cargo test` on push to main. Evaluation criteria for iCalendar library: (1) VCALENDAR wrapper on serialize, (2) unknown-property round-trip, (3) RELTYPE parameter on RELATED-TO, (4) line unfolding on parse, (5) RFC 5545 TEXT escaping. Document in `docs/adr/ical-library.md`. Evaluation criteria for XML parser: namespace-aware parsing of `D:` and `C:` prefixes, `<D:getetag>`, `<C:calendar-data>` extraction. Document in `docs/adr/xml-parser.md`. Add `[[bin]] name = "caldawarrior" path = "src/main.rs"` to Cargo.toml.
  - File: `Cargo.toml`
  - Acceptance criteria:
    - `cargo build` succeeds with all dependencies resolved
    - CI workflow runs `cargo test` on push
    - iCalendar library decision documented in `docs/adr/ical-library.md` with evaluation against all 5 criteria
    - XML parser decision documented in `docs/adr/xml-parser.md`
    - `[[bin]]` entry sets binary name to `caldawarrior`

- **Set up Docker/Radicale for empirical research** `implementation` `small`
  - Description: Create `tests/integration/docker-compose.yml` with a Radicale service (lightweight Python CalDAV server). This infrastructure is reused by Phase 5 integration tests (not duplicated). Confirm Radicale is accessible at `http://localhost:5232` and can accept basic-auth CalDAV requests. Create a basic calendar and verify VTODO PUT/REPORT round-trip manually.
  - File: `tests/integration/docker-compose.yml`
  - Acceptance criteria:
    - `docker-compose up` starts Radicale at `http://localhost:5232`
    - A VTODO can be PUT and retrieved via REPORT successfully (verified manually with `curl`)
    - The service is used in Phase 0 empirical research and reused in Phase 5

- **Empirical research: `task import` timestamp mutation and UUID-restore behavior** `research` `small`
  - Description: Write shell scripts verified against the local TW + Radicale (Phase 0 Docker setup): (1) Create a TW task, record `modified`. Export and re-import. Does `modified` change? (2) Create a TW task, delete it, then `task import` its JSON again with the same UUID — does TW restore, error, or silently ignore? (3) `task import` with an existing pending UUID — update in place or duplicate? (4) `task import` with a fresh UUID4 — creates new task with UDA fields intact? (5) Verify `status.not:purged` filter syntax is valid for the target TW version; if invalid, remove it entirely (purged tasks don't appear in `task export` output regardless). (6) Verify TW JSON export for a task whose wait date has expired: is status still `waiting` or `pending`? Is `wait` field present or absent? (7) Verify `task <uuid> modify caldavuid:` (trailing colon) clears the UDA field entirely — not sets it to an empty string — by checking with `task <uuid> export` after. (8) Verify `task <uuid> delete` on a task already in `deleted` state: no-op, non-zero exit code, or idempotent? (9) Verify `task <uuid> modify <field>:` syntax for clearing all sync-relevant standard fields: `due:`, `scheduled:`, `wait:`, `end:`, `description:`, `depends:`. Confirm each clears correctly. Document all findings in ADRs (separate file for field-clearing: `docs/adr/tw-field-clearing.md`). ADR format: Title / Status / Context / Decision / Consequences; minimum content: evaluation table, chosen approach, rejected alternatives.
  - File: `docs/adr/tw-import-timestamp.md`
  - Acceptance criteria:
    - All 9 findings documented in ADRs (items 1–9):
      - (1) Does `task import` mutate `modified`? → determines Phase 3 LWW approach and Layer 1 loop-prevention validity
      - (2) `task import` with an existing deleted UUID: restore, error, or ignore?
      - (3) `task import` with an existing pending UUID: update in-place or duplicate? → determines `TwUpdateStrategy` default
      - (4) `task import` with a fresh UUID4: creates new task with UDA fields intact?
      - (5) `status.not:purged` filter syntax valid for target TW version?
      - (6) Expired wait date in TW export: status `waiting` or `pending`? `wait` field present or absent?
      - (7) `task <uuid> modify caldavuid:` (trailing colon): clears UDA entirely (verified via export) or sets literal empty string?
      - (8) `task <uuid> delete` on already-deleted task: no-op, error code, or idempotent?
      - (9) Verify `task <uuid> modify <field>:` syntax for clearing all sync-relevant standard fields: `due`, `scheduled`, `wait`, `end`, `description`, `depends`. Verify each clears correctly. Also verify status transitions: (a) `task <uuid> modify status:completed end:<date>` vs `task <uuid> done` — hook behavior and `end` field handling; (b) `task <uuid> modify status:waiting wait:<date>` — moves to waiting correctly; (c) `task <uuid> modify status:pending` — restores a waiting task. If status transitions via `modify` are unreliable, document a dedicated `TwAdapter.complete(uuid)` method using `task <uuid> done` instead. Document in `docs/adr/tw-field-clearing.md`.
    - If item (1) confirms `task import` mutates `modified`, verify content-identical check (all 7 fields) produces stable results across two consecutive syncs on an unmodified task (Layer 2 loop-prevention gate)
    - If item (3) confirms `task import` duplicates on UUID collision, Phase 3 uses `TaskModify` strategy; otherwise `TaskImport` may be activated
    - `status.not:purged` filter validated; export invocations updated if invalid
    - `task import` timestamp mutation finding determines Phase 3 LWW approach
    - Wait-date expiry behavior documented; field mapper acceptance criteria updated accordingly

- **Phase 0 fidelity review** `verification` `fidelity`
  - Description: Confirm Phase 0 deliverables are complete
  - File: N/A
  - Acceptance criteria:
    - Project builds; CI green
    - `task import` timestamp behavior is documented and Phase 3 LWW approach is finalized

#### Verification

- **Run tests:** `cargo build && cargo test`
- **Fidelity review:** Compare Phase 0 to spec
- **Manual checks:** CI pipeline is green

---

### Phase 1: Foundation — Adapters, Config & Error Types

**Goal:** Establish the two system adapters (TW and CalDAV), configuration infrastructure, error type hierarchy, warning enum, and iCalendar serializer so the rest of the implementation has a stable I/O layer.

**Description:** Implement the Warning enum (needed by Phases 2+), error types, the TaskWarrior adapter (with trait-based mocking), the CalDAV adapter (VTODO CRUD with ETag support), iCalendar VTODO serialization/deserialization including the VCALENDAR wrapper, and the configuration module.

#### Tasks

- **Define shared domain types** `implementation` `medium`
  - Description: Create `src/types.rs` with all shared domain types. Define with `#[derive(Serialize, Deserialize, Debug, Clone)]` and explicit `#[serde(rename)]` to match TW JSON export keys:
    ```rust
    enum TWStatus { Pending, Waiting, Completed, Deleted, Recurring }
    struct TWTask {
        uuid: Uuid,
        description: String,       // serde rename "description"
        project: Option<String>,   // serde rename "project"
        status: TWStatus,          // serde rename "status"
        modified: DateTime<Utc>,   // serde rename "modified"
        due: Option<DateTime<Utc>>,
        scheduled: Option<DateTime<Utc>>,
        wait: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
        depends: Vec<Uuid>,
        caldavuid: Option<String>, // UDA; absent from non-UDA-registered TW instances
    }
    struct IcalProp { pub name: String, pub value: String }
    enum VTodoStatus { NeedsAction, InProcess, Completed, Cancelled }
    enum RelType { DependsOn, Other(String) }
    struct RelatedTo { uid: String, reltype: RelType }
    struct VTODO {
        uid: String, summary: String, description: Option<String>,
        status: VTodoStatus, dtstamp: DateTime<Utc>,  // required; set to current UTC on write
        dtstart: Option<DateTime<Utc>>,
        due: Option<DateTime<Utc>>, completed: Option<DateTime<Utc>>,
        last_modified: Option<DateTime<Utc>>,
        related_to: Vec<RelatedTo>,
        x_taskwarrior_wait: Option<DateTime<Utc>>,
        x_caldawarrior_last_sync: Option<DateTime<Utc>>,
        rrule: Option<String>,  // always None on writes; presence triggers RecurringCalDavSkipped
        extra_props: Vec<IcalProp>,  // unknown properties preserved
    }
    struct FetchedVTODO { vtodo: VTODO, etag: Option<String>, calendar_url: String }
    enum PlannedOp {
        Create { side: Side, uid: String, summary: String },
        Update { side: Side, uid: String, summary: String, reason: UpdateReason },
        Delete { side: Side, uid: String, summary: String },
        Skip   { uid: String, summary: String, reason: SkipReason },
    }
    enum Side { Tw, CalDav }
    enum UpdateReason {
        LwwTwWins,               // field diff; TW modified timestamp wins
        LwwCalDavWins,           // field diff; CalDAV LAST-MODIFIED wins
        TwDeletedMarkCancelled,  // TW deleted → CalDAV updated to CANCELLED (PlannedOp::Update)
        TwCompletedMarkCompleted, // TW completed → CalDAV updated to COMPLETED (PlannedOp::Update)
        CalDavCompletedUpdateTw, // CalDAV COMPLETED → TW task marked completed (PlannedOp::Update)
        // NOTE: CalDAV CANCELLED → TW task deleted emits PlannedOp::Delete { side: Tw }, NOT PlannedOp::Update
    }
    enum SkipReason {
        Cancelled,              // CalDAV-only CANCELLED item; not imported as new TW task
        Completed,              // CalDAV-only COMPLETED item; not imported as new TW task
        Recurring,              // TW recurring or CalDAV RRULE item; skipped
        Cyclic,                 // dependency cycle detected; excluded from write-back
        Identical,              // both-exist, all 8 fields match after normalization (SUMMARY, STATUS, DUE, DTSTART, COMPLETED, DESCRIPTION, RELATED-TO[DEPENDS-ON], X-TASKWARRIOR-WAIT)
        DeletedBeforeSync,      // TW task in deleted state with no caldavuid → was never synced; no-op
        AlreadyDeleted,         // TW-only entry in `deleted` state with caldavuid set → CalDAV deletion already won; no-op
        CalDavDeletedTwTerminal, // TW task completed (terminal); its caldavuid has no matching VTODO; no action needed
    }
    struct SyncResult {
        planned_ops: Vec<PlannedOp>,
        warnings: Vec<Warning>,
        errors: Vec<(String, CaldaWarriorError)>,  // (caldav_uid, error); one per failed entry after retries
        written_tw: usize,    // always 0 in dry-run
        written_caldav: usize,  // always 0 in dry-run
        skipped: usize,       // number of entries skipped (identical, cancelled, recurring, cyclic, etc.)
    }
    ```
  - File: `src/types.rs`
  - Acceptance criteria:
    - All types defined with correct field names matching TW JSON export format
    - `FetchedVTODO` wraps `VTODO` + ETag (iCalendar layer stays pure RFC 5545)
    - `PlannedOp`, `SyncResult` fully defined with all variants documented
    - `SyncResult.written_*` documented as always 0 in dry-run; `SyncResult.skipped` counts all skipped entries
    - `SkipReason::AlreadyDeleted` covers the "TW deleted + caldavuid set + no CalDAV match → no-op" case
    - `UpdateReason` has 6 variants covering all distinct writeback update scenarios (not just LWW)
    - `IREntry.caldav_data` uses `Option<FetchedVTODO>` (not `Option<VTODO>`)
    - `VTODO` includes `dtstamp`, `rrule`, `IcalProp`-typed `extra_props`, `RelType` enum for `reltype`
    - `SyncResult.errors` field is defined and documented (never aborts sync on single-entry failure)
    - `CyclicEntry.caldav_uid` (not `uid`) matches IREntry naming convention
    - All `Option<T>` fields that may be absent from TW JSON export carry `#[serde(default)]`: `project`, `due`, `scheduled`, `wait`, `end`, `caldavuid`; `depends: Vec<Uuid>` also carries `#[serde(default)]` (returns empty Vec when key absent)
    - All `Option<T>` fields carry `#[serde(skip_serializing_if = "Option::is_none")]` to prevent writing null values into TW import JSON; `depends` carries `#[serde(skip_serializing_if = "Vec::is_empty")]`
    - Unit tests confirm serde round-trip for TWTask from sample TW JSON export including tasks with all optional fields absent

- **Define error type hierarchy** `implementation` `small`
  - Description: Create `src/error.rs`. Define `CaldaWarriorError` enum using `thiserror`: `Config(String)`, `Tw { code: i32, stderr: String }`, `CalDav { status: u16, body: String }`, `Auth { server_url: String }` (for HTTP 401 responses — includes a human-readable message directing users to check username/password and config file), `IcalParse(String)`, `SyncConflict(String)`, `EtagConflict { refetched_vtodo: Box<FetchedVTODO> }`. All library functions return `Result<T, CaldaWarriorError>`. `main.rs` uses `anyhow::Result` only at the CLI boundary.
  - File: `src/error.rs`
  - Acceptance criteria:
    - All error variants have human-readable `Display` implementations via `thiserror`
    - `Auth` variant message says "Authentication failed for <server_url>: check username/password in config or CALDAWARRIOR_PASSWORD env var"
    - `EtagConflict` carries the freshly fetched `FetchedVTODO` for orchestrator retry
    - Unit test confirms error messages are correct

- **Define Warning enum** `implementation` `small`
  - Description: Create `src/warnings.rs`. Define `Warning` enum:
    ```rust
    enum Warning {
        RecurringSkipped { uuid: String, title: String },
        RecurringCalDavSkipped { uid: String, summary: String },
        CancelledSkipped { uid: String },
        UnresolvableDependency { task_title: String, dep_ref: String },
        CyclicTasksExcluded { tasks: Vec<CyclicEntry> },
        UnsupportedReltype { count: usize, types_seen: Vec<String> },
        UnmappedProject { project: String, task_uuid: String },
        CalDavItemError { uid: String, status: String },  // per-item error in 207 body; non-blocking
    }
    struct CyclicEntry { caldav_uid: String, summary: String }
    ```
    Do NOT add `print_warnings()` here — that belongs in Phase 4. Note: `DuplicateSummaryDetected` is removed (high false-positive rate; SUMMARY alone is insufficient for deduplication; deferred to v2).
  - File: `src/warnings.rs`
  - Acceptance criteria:
    - All eight warning variants defined with named fields (no unnamed tuples)
    - `CyclicEntry.caldav_uid` (not `uid`) matches IREntry naming convention
    - `CalDavItemError` is non-blocking (collected as warning, not error)
    - Unit tests confirm variant construction

- **Define configuration module** `implementation` `medium`
  - Description: Create `src/config.rs`. Parse a TOML config file (`~/.config/caldawarrior/config.toml`) with fields: `server_url`, `username`, `password` (overridable via `CALDAWARRIOR_PASSWORD` env var), `completed_cutoff_days` (optional u32, default 90), `allow_insecure_tls` (optional bool, default false; required for Radicale in Docker test environment), `caldav_timeout_seconds` (optional u32, default 30; used by CalDAV adapter `ClientBuilder`), and a `[[calendar]]` array mapping TW project names to CalDAV calendar URLs (including a `default` entry for projectless tasks). Config file path resolution order: (1) `--config <path>` CLI flag, (2) `CALDAWARRIOR_CONFIG` env var, (3) `~/.config/caldawarrior/config.toml`. v1 supports only HTTP Basic authentication (documented as a v1 constraint; DIGEST and Bearer are v2). v1 supports a single server only. At startup: (a) emit a `[WARN]` to stderr if the config file permissions are more permissive than `0600`; (b) validate that all `[[calendar]]` entries (excluding `default`) have distinct `url` values.
  - File: `src/config.rs`
  - Acceptance criteria:
    - Config loads from file; password overridden by `CALDAWARRIOR_PASSWORD` env var when present
    - Config file path resolution order: `--config` flag → `CALDAWARRIOR_CONFIG` env → default path
    - `completed_cutoff_days` defaults to 90 when absent; `allow_insecure_tls` defaults to false; `caldav_timeout_seconds` defaults to 30
    - Missing required fields produce a `CaldaWarriorError::Config` error with a clear message
    - Duplicate `[[calendar]]` URLs (excluding `default`) produce `CaldaWarriorError::Config("Duplicate calendar URL: {url}")`
    - Config file with permissions > `0600` emits a `[WARN]` to stderr (non-fatal)
    - Unit tests cover happy path, missing-field error, defaults, duplicate URL error, permission warning

- **Implement TaskWarrior adapter** `implementation` `medium`
  - Description: Create `src/tw_adapter.rs`. Define a `TaskRunner` trait with `export(filter: &[&str]) -> Result<Vec<TWTask>>`, `import(tasks: &[TWTask]) -> Result<()>`, and `modify(uuid: &Uuid, fields: &[(&str, &str)]) -> Result<()>`. `RealTaskRunner` uses `std::process::Command` with argument lists (NEVER shell string interpolation — security requirement). `TwAdapter` exposes: `list_all(cutoff_days: u32)` which issues **two** separate `task export` invocations and merges results: (1) `task export '(status:pending or status:waiting or status:recurring)'` (no date filter); (2) `task export '(status:completed or status:deleted) and status.not:purged and modified.after:<cutoff_date>'` (windowed). `create(task: TWTask)` always uses `task import` with a fresh UUID4 — used only by the CalDAV-only CREATE branch. `update(task: TWTask)` always uses a single `task <uuid> modify field1:val1 field2:val2 ...` invocation — used by all other writeback branches that modify an existing TW task. The writeback layer determines which to call from `IREntry.tw_data` being `None` (create) vs. `Some` (update). NOTE: Phase 0 research item #3 validates whether `task import` is also safe for in-place updates; if confirmed safe, this design remains unchanged (create=import, update=modify); no strategy enum is needed because `create()` is the only path that uses `task import` for new tasks. `delete(uuid)` runs `task <uuid> delete` then `task <uuid> modify caldavuid:` (two sequential Command calls; NOT transactional — benign on failure). UDA registration runs during `TwAdapter::new()` before any export/import call.
  - File: `src/tw_adapter.rs`
  - Acceptance criteria:
    - `TaskRunner` trait enables `MockTaskRunner` injection in unit tests (no PATH manipulation)
    - All `task` CLI invocations use `std::process::Command` with argument lists
    - `list_all()` issues two separate export commands: one for pending/waiting/recurring (no date filter), one for completed/deleted (cutoff-filtered); results merged; deduplication by UUID: when the same UUID appears in both invocations (e.g., task transitioned state mid-run), retain the entry with the higher `modified` timestamp
    - Active tasks not modified in 90+ days are always included (not filtered by cutoff)
    - `TwAdapter` exposes `create(task: TWTask)` (always uses `task import` with fresh UUID4 — only for CalDAV-only CREATE branch) and `update(task: TWTask)` (always uses single `task <uuid> modify field1:val1 field2:val2 ...` for all changed fields — for all other writeback branches)
    - Writeback layer calls `tw.create()` when `IREntry.tw_data` is `None`; calls `tw.update()` when `IREntry.tw_data` is `Some`
    - `update()`: `depends` field constructed as comma-separated UUIDs; field-clear uses trailing-colon syntax verified in Phase 0 item #9; all field changes issued in a single `task modify` invocation
    - `delete(uuid)` runs two sequential Commands; stale caldavuid on second-command failure is documented as benign (task in deleted state with orphaned caldavuid is handled by AlreadyDeleted SKIP branch on next sync)
    - UDA registration runs in `TwAdapter::new()` before any export/import call; registration configures `uda.caldavuid.type=string` and `uda.caldavuid.label=CalDAV UID`
    - UDA registration emits `[INFO] Registering caldavuid UDA in TaskWarrior configuration.` to stderr (only if newly registered)
    - Unit test confirms registration runs before `list_all()` (mock runner verifies call ordering)
    - Unit tests use `MockTaskRunner`; command argument construction verified for both strategies

- **Implement iCalendar VTODO serializer/deserializer** `implementation` `medium`
  - Description: Create `src/ical.rs`. Parse iCalendar text into a `VTODO` struct and serialize back. Support all fields: UID, SUMMARY, DESCRIPTION, STATUS, DTSTAMP, DTSTART, DUE, COMPLETED, LAST-MODIFIED, RRULE (parsed into `rrule: Option<String>`), RELATED-TO (with RELTYPE param parsed into `RelType` enum), X-TASKWARRIOR-WAIT, X-CALDAWARRIOR-LAST-SYNC. Preserve unknown properties as `Vec<IcalProp>` on round-trip. Apply RFC 5545 TEXT escaping (§3.3.11) to SUMMARY and DESCRIPTION on serialize; reverse on parse. Apply RFC 5545 line folding (§3.1) at 75 octets on serialize; unfold on parse. TZID-aware timestamps are converted to UTC via `chrono-tz`; UTC timestamps (trailing `Z`) parsed directly; floating timestamps treated as UTC with a logged warning. The serializer wraps VTODO in a full VCALENDAR envelope. The parser extracts VTODO from the VCALENDAR wrapper.
  - File: `src/ical.rs`
  - Acceptance criteria:
    - `to_icalendar_string(vtodo)` produces a valid VCALENDAR document with DTSTAMP set to current UTC
    - `from_icalendar_string(s)` extracts the VTODO from its VCALENDAR wrapper
    - DTSTAMP is always written; never read from server state on write path
    - `rrule: Option<String>` is populated if RRULE present; never written by the tool
    - TEXT escaping: SUMMARY/DESCRIPTION escape `\`, `,`, `;`, newlines on serialize; reverse on parse
    - Line folding: lines > 75 octets folded (CRLF + SPACE); fold-continuation recombined on parse
    - TZID timestamps normalized to UTC via `chrono-tz`
    - RELTYPE parameter parsed into `RelType::DependsOn` or `RelType::Other(s)`
    - Unknown properties preserved as `Vec<IcalProp>` on round-trip
    - Unit tests cover: all target fields, TEXT escaping (each of `\`, `,`, `;`, newline), line folding (>75 chars), TZID normalization, unknown-property round-trip, VCALENDAR wrapper, RRULE detection

- **Implement CalDAV adapter** `implementation` `medium`
  - Description: Create `src/caldav_adapter.rs`. Define a `CalDavClient` trait with methods: `list_vtodos(calendar_url: &str) -> Result<Vec<FetchedVTODO>>`, `put_vtodo(calendar_url: &str, vtodo: &VTODO, etag: Option<&str>) -> Result<()>`, `delete_vtodo(calendar_url: &str, uid: &str, etag: Option<&str>) -> Result<()>`. `RealCalDavClient` implements the trait using `reqwest` blocking HTTP. `apply_writeback()` and `run_sync()` accept `&dyn CalDavClient` (or `impl CalDavClient`) for mock injection in unit tests. `list_vtodos()` uses REPORT with `comp-filter`; parses the WebDAV multi-status 207 response into a flat `Vec<FetchedVTODO>` (interface replaceable with RFC 6578 sync tokens in v2). Store ETag from each `<D:getetag>` element. `put_vtodo()`: PUT to `{calendar_url}/{uid}.ics` with `Content-Type: text/calendar`; include `If-Match: <etag>` when etag present; `If-None-Match: *` when etag absent (new resource creation, per RFC 4791); return `Err(CaldaWarriorError::EtagConflict { refetched_vtodo })` on HTTP 412. `delete_vtodo()`: DELETE with `If-Match`. HTTP 401: return `Err(CaldaWarriorError::Auth { server_url })`. TLS errors: wrap in `CaldaWarriorError::CalDav` with guidance to set `allow_insecure_tls = true`. The adapter never retries or calls LWW — that is the orchestrator's responsibility. `reqwest::blocking::Client` constructed with `timeout(Duration::from_secs(config.caldav_timeout_seconds))`.
  - File: `src/caldav_adapter.rs`
  - Acceptance criteria:
    - `CalDavClient` trait defined; `RealCalDavClient` implements it; `MockCalDavClient` available in test modules
    - `apply_writeback()` and `run_sync()` accept `dyn CalDavClient`; Phase 3 unit tests use `MockCalDavClient` (no HTTP server required)
    - `list_vtodos()` returns a flat `Vec<FetchedVTODO>` parsing all `<D:response>` elements from the 207 body
    - Creates/updates VTODOs via PUT to `{calendar_url}/{uid}.ics` with correct Content-Type
    - Includes `If-Match: <etag>` on PUT/DELETE when etag available; `If-None-Match: *` on PUT when etag absent
    - HTTP 412: returns `Err(CaldaWarriorError::EtagConflict { refetched_vtodo })` with a fresh fetch — does NOT retry internally
    - HTTP 401: returns `Err(CaldaWarriorError::Auth { server_url })`
    - TLS errors: wrapped with `allow_insecure_tls` guidance message
    - Per-item errors within the 207 body collected as `Warning::CalDavItemError` (non-blocking)
    - PUT URL constructed as `{calendar_url}/{percent_encoded_uid}.ics` using `percent-encoding` crate
    - Returns `CaldaWarriorError::CalDav` for other HTTP 4xx/5xx responses
    - `reqwest::blocking::Client` configured with timeout from config
    - Unit tests use `MockCalDavClient` (not a mock HTTP server)

- **Phase 1 fidelity review** `verification` `fidelity`
  - Description: Compare Phase 1 implementation against spec — adapters, config, iCalendar serializer, error types, warning enum
  - File: N/A
  - Acceptance criteria:
    - All Phase 1 tasks implemented as specified
    - No deviations from interface contracts (especially VCALENDAR wrapper and ETag handling)

#### Verification

- **Run tests:** `cargo test`
- **Fidelity review:** Compare implementation to spec Phase 1
- **Manual checks:** `cargo run -- --help` smoke test; confirm `VCALENDAR` wrapper appears in PUT body

---

### Phase 2: Core Mapping Engine

**Goal:** Implement the bidirectional status mapper, field mapper, X-TASKWARRIOR-WAIT encoding logic, and the IR builder.

**Description:** All data transformation logic lives here. The status and field mappers translate between TW and CalDAV representations using the canonical `StatusDecision` enum to distinguish skip-create vs. delete-existing semantics. The IR builder loads both systems, matches by `caldavuid`, and assigns new UIDs where needed.

#### Tasks

- **Implement status mapper (TW → CalDAV)** `implementation` `medium`
  - Description: In `src/mapper/status.rs`, implement `tw_to_caldav_status(task: &TWTask) -> TwToCalDavStatus` where `TwToCalDavStatus` is an enum: `NeedsAction`, `NeedsActionWithWait(DateTime)`, `Completed(DateTime)`, `TwDeleted`, `Skip(Warning)`. Map: pending→NeedsAction, waiting→NeedsActionWithWait, completed→Completed, deleted→TwDeleted, recurring→Skip(RecurringSkipped). NOTE: `TwDeleted` does NOT mean hard-delete the CalDAV VTODO — the writeback layer dispatches this to CANCELLED (if both-exist) or SKIP (if TW-only with caldavuid). The name carries no implication of what action the CalDAV side takes.
  - File: `src/mapper/status.rs`
  - Acceptance criteria:
    - All five TW statuses produce the correct enum variant
    - `deleted` returns `TwDeleted` (not a CalDAV hard-delete signal; writeback handles dispatch)
    - `recurring` returns `Skip(RecurringSkipped {...})` (no panic)
    - Unit tests cover all five branches

- **Implement status mapper (CalDAV → TW) with StatusDecision** `implementation` `medium`
  - Description: In `src/mapper/status.rs`, implement `caldav_to_tw_status(vtodo: &VTODO) -> StatusDecision` where `StatusDecision` is: `Map(TWStatus)`, `SkipCreate(Warning)`, `DeleteExisting`. Map: NEEDS-ACTION (no X-TASKWARRIOR-WAIT)→Map(pending), NEEDS-ACTION+wait(future)→Map(waiting), NEEDS-ACTION+wait(past)→Map(pending), COMPLETED→Map(completed), IN-PROCESS→Map(pending) (preserve DTSTART as `scheduled`), CANCELLED→DeleteExisting (if both-exist) or SkipCreate (if CalDAV-only).
  - File: `src/mapper/status.rs`
  - Acceptance criteria:
    - `StatusDecision` has three variants: `Map`, `SkipCreate`, `DeleteExisting`
    - CANCELLED context is determined by the caller (both-exist vs. CalDAV-only); the mapper returns the discriminating variant per context
    - Expired X-TASKWARRIOR-WAIT collapses to `Map(pending)`
    - IN-PROCESS preserves DTSTART as `scheduled`
    - Unit tests cover all six input cases including past/future wait date

- **Implement field mapper** `implementation` `medium`
  - Description: Create `src/mapper/fields.rs`. Implement `tw_to_caldav_fields(task: &TWTask, vtodo: &mut VTODO)` and `caldav_to_tw_fields(vtodo: &VTODO, task: &mut TWTask)`. Fields: scheduled↔DTSTART, wait↔X-TASKWARRIOR-WAIT, due↔DUE, end/completion↔COMPLETED timestamp, description↔DESCRIPTION, title↔SUMMARY, sync epoch↔X-CALDAWARRIOR-LAST-SYNC (written by sync orchestrator, not field mapper).
  - File: `src/mapper/fields.rs`
  - Acceptance criteria:
    - All field mappings applied in both directions
    - Missing optional fields on either side do not produce errors
    - Unit tests cover round-trip mapping for all fields

- **Implement IR builder** `implementation` `medium`
  - Description: Create `src/ir.rs`. Define the `IREntry` struct:
    ```rust
    struct IREntry {
        caldav_uid: String,                   // always present; primary join key
        calendar_url: String,                 // source CalDAV calendar URL
        tw_uuid: Option<Uuid>,                // None if CalDAV-only
        tw_data: Option<TWTask>,              // None if CalDAV-only
        caldav_data: Option<FetchedVTODO>,    // None if TW-only; includes ETag
        resolved_depends: Vec<String>,        // caldav_uid strings (populated in Step 2)
        dirty_tw: bool,                       // set only by LWW resolver or decision tree
        dirty_caldav: bool,                   // set only by LWW resolver or decision tree
        cyclic: bool,                         // set by cycle detector in Step 2
    }
    ```
    Implement `build_ir(tw_tasks: Vec<TWTask>, vtodos_by_calendar: Vec<(String, Vec<FetchedVTODO>)>, config: &Config) -> (Vec<IREntry>, Vec<Warning>)`. The `config` parameter is required for project→calendar URL resolution and reverse-mapping for TW-only entries. Three-way classification of each TW task:
    - `caldavuid == None` → new TW-only entry; assign a fresh UUID4 as `caldavuid`
    - `caldavuid IS SET` and matches a loaded VTODO UID → paired entry (both `tw_data` and `caldav_data` set)
    - `caldavuid IS SET` but no matching VTODO found → **orphaned UID**: TW-only entry with the existing `caldavuid` UNCHANGED (do NOT assign a new UID; this signals CalDAV deletion to the writeback layer)
    CalDAV-only (unmatched VTODOs): skip with `RecurringCalDavSkipped` if RRULE present; otherwise create CalDAV-only entry. VTODOs from `default` calendar produce TW tasks with `project: None`. Emit `UnmappedProject` warning for TW tasks whose project has no config entry (assign to `default`). `dirty_*` and `cyclic` always `false` at construction.
  - File: `src/ir.rs`
  - Acceptance criteria:
    - `IREntry` struct matches specification exactly (all fields, types, semantics)
    - `caldav_data` is `Option<FetchedVTODO>` (not `Option<VTODO>`)
    - `resolved_depends` stores `caldav_uid` strings (not vector indices)
    - TW tasks with `caldavuid == None` get a fresh UUID4 assigned as `caldavuid`
    - TW tasks with an existing non-None `caldavuid` matching a loaded VTODO → paired entry
    - TW tasks with an existing non-None `caldavuid` that does NOT match any loaded VTODO → TW-only entry with their existing `caldavuid` UNCHANGED (orphaned UID state; distinguished from new tasks by the presence of `caldavuid`)
    - Unmatched CalDAV VTODOs with RRULE → skipped with `RecurringCalDavSkipped` warning
    - Unmatched CalDAV VTODOs without RRULE → CalDAV-only entries
    - VTODOs from `default` calendar produce TW tasks with `project: None`
    - TW-only entry `calendar_url` is resolved from config by project name at IR construction time (not at writeback time); `UnmappedProject` warning is emitted here if no config entry matches (assigned to `default` calendar URL)
    - CalDAV-only entry `calendar_url` is taken directly from `FetchedVTODO.calendar_url`
    - CalDAV-only NEEDS-ACTION entries have a fresh UUID4 pre-assigned to `IREntry.tw_uuid` during `build_ir()` (same pattern as `caldavuid` for TW-only entries); this ensures Step 3 dependency resolution never encounters `None` tw_uuid for co-created tasks
    - CalDAV-only CANCELLED/COMPLETED entries have `IREntry.tw_uuid = None` (never imported)
    - `dirty_*` and `cyclic` always false after construction
    - Unit tests cover: new TW task (caldavuid None), paired match, orphaned caldavuid (no VTODO match), RRULE skip, unmapped project, CalDAV-only NEEDS-ACTION pre-assigned tw_uuid

- **Phase 2 fidelity review** `verification` `fidelity`
  - Description: Compare Phase 2 implementation against spec — status mapper, field mapper, IR builder, StatusDecision enum
  - File: N/A
  - Acceptance criteria:
    - StatusDecision enum correctly distinguishes SkipCreate vs DeleteExisting
    - IREntry struct matches spec exactly

#### Verification

- **Run tests:** `cargo test mapper:: ir::`
- **Fidelity review:** Compare implementation to spec Phase 2
- **Manual checks:** None

---

### Phase 3: Sync Algorithm

**Goal:** Implement the three-step sync engine: IR construction (Step 1), dependency resolution with cycle detection (Step 2), and the write-back decision tree with LWW conflict resolution (Step 3).

**Description:** Core sync logic. Step 2 resolves TW UUID↔CalDAV UID dependency references and runs DFS cycle detection. Step 3 implements the write-back decision tree including the orphaned-caldavuid branch and LWW. LWW uses the `X-CALDAWARRIOR-LAST-SYNC` sync-epoch approach if `task import` mutates `modified` timestamps (confirmed in Phase 0).

#### Tasks

- **Implement dependency resolver (Step 2)** `implementation` `complex`
  - Description: Create `src/sync/deps.rs`. Implement `resolve_dependencies(ir: &mut Vec<IREntry>) -> Vec<Warning>`. For each entry: translate TW UUID-based `depends` to CalDAV UID references and vice versa using the IR as a lookup table. Drop unresolvable references with `UnresolvableDependency` warning. Non-DEPENDS-ON RELATED-TO types: collect and emit exactly one `UnsupportedReltype` summary warning per run. Run DFS cycle detection; set `cyclic = true` on all flagged entries and emit one `CyclicTasksExcluded` warning.
  - File: `src/sync/deps.rs`
  - Acceptance criteria:
    - TW UUID `depends` translated to CalDAV UIDs for write-back
    - CalDAV `RELATED-TO;RELTYPE=DEPENDS-ON` translated to TW UUID references
    - Unresolvable references dropped with `UnresolvableDependency` warning
    - Non-DEPENDS-ON RELATED-TO types produce exactly one `UnsupportedReltype` summary warning per run
    - DFS detects cycles; all tasks in a cycle have `cyclic = true`
    - Unit tests: normal deps, unresolvable drop, 3-node cycle, non-DEPENDS-ON skip

- **Implement LWW conflict resolver** `implementation` `medium`
  - Description: Create `src/sync/lww.rs`. Implement `resolve_lww(entry: &mut IREntry)`. Compare TW `modified` against CalDAV `LAST-MODIFIED`. If `X-CALDAWARRIOR-LAST-SYNC` is present on the VTODO (see Phase 0 finding), use it as the sync-epoch baseline: only trigger LWW write if the modified timestamp is strictly newer than LAST-SYNC. `X-CALDAWARRIOR-LAST-SYNC` is ALWAYS set to the **TW task's `modified` value at the time of the CalDAV write** (copied from `TWTask.modified`, NOT from the wall clock). This ensures that on the next sync, TW.modified == LAST-SYNC → no LWW evaluation for the TW-wins case. **Two-layer loop prevention:** Layer 1 (TW-wins path): `X-CALDAWARRIOR-LAST-SYNC = TWTask.modified` prevents re-trigger because on next sync `TW.modified == LAST-SYNC → no LWW evaluation`. Layer 2 (CalDAV-wins path): LAST-SYNC is NOT updated (no CalDAV PUT occurs); re-trigger is prevented by the content-identical check in the writeback decision tree, which short-circuits before LWW is called. The content-identical check correctness is therefore a hard dependency for loop prevention on the CalDAV-wins path. Tiebreaker: CalDAV side wins. Set `dirty_caldav = true` when TW wins; `dirty_tw = true` when CalDAV wins. `LAST-MODIFIED` fallback: if `last_modified` is `None`, use `dtstamp` as the CalDAV timestamp; if `dtstamp` is also unavailable (should not occur for well-formed VTODOs), CalDAV wins by default with a logged warning.
  - File: `src/sync/lww.rs`
  - Acceptance criteria:
    - Newer TW side: `dirty_caldav = true`
    - Newer CalDAV side: `dirty_tw = true`
    - Timestamp tie: CalDAV wins (`dirty_tw = true`)
    - If sync-epoch approach is active: no write when TW `modified` ≤ LAST-SYNC epoch (TW-wins path only)
    - If X-CALDAWARRIOR-LAST-SYNC is absent (first sync for this entry): skip the sync-epoch gate and compare TW.modified vs CalDAV.LAST-MODIFIED directly
    - If `last_modified` is None: fall back to `dtstamp`; if both absent: CalDAV wins with a logged warning
    - Unit tests cover TW-wins, CalDAV-wins, tie, sync-epoch no-op, LAST-MODIFIED-None fallback, and first-sync (no LAST-SYNC present) cases

- **Implement write-back decision tree (Step 3)** `implementation` `complex`
  - Description: Create `src/sync/writeback.rs`. Implement `apply_writeback(ir: &mut Vec<IREntry>, tw: &TwAdapter, caldav: &dyn CalDavClient, dry_run: bool) -> (Vec<PlannedOp>, Vec<Warning>)`. The `ir` parameter is mutable to allow updating `IREntry.tw_uuid` for newly created TW tasks and `IREntry.caldav_data` during ETag conflict retries (re-run `resolve_lww()` with refetched VTODO then retry write, max 3 attempts per entry). Decision tree:
    ```
    Cyclic entry              → SKIP (never write)

    TW only:
      recurring (any caldavuid state) → SKIP, emit RecurringSkipped warning
      deleted, no caldavuid   → SKIP (DeletedBeforeSync; was never synced)
      deleted, has caldavuid  → SKIP (TW task already deleted; CalDAV deletion wins; no-op)
      pending/waiting, no caldavuid → CREATE in CalDAV
      pending/waiting, has caldavuid but no CalDAV match → DELETE/complete TW task (orphaned UID = CalDAV deletion signal)
      completed, no caldavuid → CREATE in CalDAV as COMPLETED
      completed, has caldavuid but no CalDAV match → SKIP (CalDavDeletedTwTerminal; task completed normally; CalDAV item was subsequently deleted; caldavuid left in place; task ages out of cutoff window)

    CalDAV only:
      CANCELLED / COMPLETED   → SKIP
      NEEDS-ACTION            → CREATE in TW: UUID4 pre-assigned in build_ir() as IREntry.tw_uuid,
                                construct TWTask with uuid=IREntry.tw_uuid, caldavuid=vtodo.uid,
                                all fields via caldav_to_tw_fields() + caldav_to_tw_status();
                                call tw.create(new_task) — NEVER tw.update()

    Both exist:
      TW recurring                  → SKIP, emit RecurringSkipped warning
      TW deleted, CalDAV active    → UPDATE CalDAV → CANCELLED
      TW completed, CalDAV active  → UPDATE CalDAV → COMPLETED
      CalDAV CANCELLED, TW active (pending or waiting) → DELETE TW task (DeleteExisting branch)
      CalDAV COMPLETED, TW active (pending or waiting) → UPDATE TW → completed
      both deleted/cancelled/completed → SKIP
      both active, identical content (SUMMARY+STATUS+DUE+DTSTART+DESCRIPTION+RELATED-TO[DEPENDS-ON]+X-TASKWARRIOR-WAIT match; timestamps excluded) → SKIP
      both active, content differs   → resolve_lww()
    ```
    Dry-run: collect `PlannedOp` entries without executing writes. Update `X-CALDAWARRIOR-LAST-SYNC` on all CalDAV writes. The simplest implementation path: call `tw_to_caldav_status()` at the top of the writeback loop and return early on `TwToCalDavStatus::Skip` before evaluating any other branch.
  - File: `src/sync/writeback.rs`
  - Acceptance criteria:
    - CalDAV-only NEEDS-ACTION → new TW task created with fresh UUID4, caldavuid set to VTODO UID, all fields mapped via caldav_to_tw_fields(); new TW task `project` field is reverse-mapped from `IREntry.calendar_url` using the config's `[[calendar]]` reverse mapping (`default` calendar URL → `project: None`); `tw.create(new_task)` used for creation (not `tw.update()`)
    - For new TW-only CalDAV creates: PUT VTODO first; only on success call tw.update() to persist caldavuid. If process crashes between the two writes, a dangling VTODO exists in CalDAV — documented in Known Limitations as requiring manual cleanup
    - Orphaned caldavuid (TW pending, caldavuid set but no CalDAV match) → TW task deleted (not CalDAV re-create)
    - TW-only recurring (any caldavuid state) → SKIP with RecurringSkipped warning (early return before any other branch)
    - Both-exist TW recurring → SKIP with RecurringSkipped warning
    - "CalDAV CANCELLED/COMPLETED, TW active" branches explicitly handle both pending and waiting states
    - All decision tree branches implemented as specified
    - Both-exist identical content check covers all 8 fields (SUMMARY, STATUS, DUE, DTSTART, COMPLETED, DESCRIPTION, RELATED-TO[DEPENDS-ON], X-TASKWARRIOR-WAIT); modification timestamps excluded. Normalization contract: (a) Timestamps (DUE, DTSTART, X-TASKWARRIOR-WAIT): truncate to second precision and compare as `DateTime<Utc>`; (b) DESCRIPTION: apply RFC 5545 TEXT unescaping, then trim trailing whitespace and normalize `\r\n` → `\n`; (c) SUMMARY: apply RFC 5545 TEXT unescaping before comparison; (d) RELATED-TO: extract only `RELTYPE=DEPENDS-ON` entries; sort caldav_uid strings alphabetically before comparison; (e) STATUS: normalize through canonical `TWStatus`/`VTodoStatus` enum (not raw strings); (f) X-TASKWARRIOR-WAIT: normalize to second precision
    - A DTSTART-only change triggers `dirty_caldav` or `dirty_tw` (not skipped)
    - Cyclic entries never written
    - `X-CALDAWARRIOR-LAST-SYNC` updated on every CalDAV write
    - When writing TW `depends` field: iterate `IREntry.resolved_depends` (CalDAV UIDs); look up each UID in the IR index (`HashMap<caldav_uid, &IREntry>`) to retrieve `tw_uuid`; drop unresolvable entries (already warned by Step 2 `UnresolvableDependency`); write result as `Vec<Uuid>`
    - When writing CalDAV `RELATED-TO`: use `IREntry.resolved_depends` (CalDAV UIDs) directly
    - Dry-run returns PlannedOp list without executing writes
    - `SyncResult.errors` accumulates per-entry `SyncConflict` after 3 retries; sync continues for remaining entries
    - On `EtagConflict` during a CalDAV DELETE (CANCELLED write): re-run the full writeback decision tree for that entry using the refetched VTODO, then retry (max 3 attempts) — this may resolve to a CANCELLED PUT instead of DELETE if the refetched VTODO changes the decision
    - Unit tests cover every branch including recurring (TW-only and both-exist), orphaned caldavuid, DTSTART-only change, EtagConflict retry exhaustion, and dependency reverse-mapping
    - Regression test: sync once (CalDAV wins, TW updated), sync again with no changes → assert `written_caldav == 0 && written_tw == 0` (loop-prevention stable-point test)

- **Implement full sync orchestrator** `implementation` `medium`
  - Description: Create `src/sync/mod.rs`. Implement `run_sync(config: &Config, tw: &TwAdapter, caldav: &CalDavAdapter, dry_run: bool) -> SyncResult`. Step 1: fetch VTODOs from all calendars (`list_vtodos` per calendar), build IR. Step 2: resolve dependencies. Step 3: write back. On `CaldaWarriorError::EtagConflict { refetched_vtodo }` from a CalDAV write: update the IR entry's `caldav_data` with the refetched VTODO, re-run `resolve_lww()`, and retry the write — max 3 attempts before returning a `SyncConflict` error for that entry. Collect all warnings from all steps and return in `SyncResult`.
  - File: `src/sync/mod.rs`
  - Acceptance criteria:
    - All three steps run in order: build_ir() → deps resolver → apply_writeback()
    - ETag retry logic is owned exclusively by `apply_writeback()` (not duplicated here); `run_sync()` calls `apply_writeback()` and propagates its `SyncResult` without re-implementing retry
    - All warnings collected from all three steps and returned in `SyncResult`
    - Dry-run passes through correctly
    - Integration test (mock adapters) covers full three-step flow

- **Phase 3 fidelity review** `verification` `fidelity`
  - Description: Compare Phase 3 implementation against spec — sync algorithm, LWW, orphaned UID handling, dependency resolution
  - File: N/A
  - Acceptance criteria:
    - Decision tree matches spec exactly (all branches including orphaned caldavuid)
    - LWW tiebreaker is CalDAV-wins
    - Sync-epoch approach consistent with Phase 0 findings

#### Verification

- **Run tests:** `cargo test sync::`
- **Fidelity review:** Compare implementation to spec Phase 3
- **Manual checks:** None

---

### Phase 4: CLI & Output

**Goal:** Expose the sync engine as a usable CLI with a `sync` subcommand and `--dry-run` flag, plus structured warning printing and logging.

**Description:** Wire everything into an executable. The CLI parses arguments, loads config, calls `run_sync`. Dry-run output is structured and human-readable. `print_warnings()` from `warnings.rs` is added here. `tracing` is configured for debug output via `RUST_LOG`.

#### Tasks

- **Implement CLI entry point** `implementation` `medium`
  - Description: Create `src/main.rs` and `src/cli.rs`. Use `clap` (derive). Subcommands: `sync` (flags: `--dry-run`, `--config <path>`). On `sync`: load config, call `run_sync`, call `print_warnings`, print dry-run output if applicable, exit 0 on success or non-zero on fatal error. Initialize `tracing-subscriber` supporting `RUST_LOG=caldawarrior=debug`.
  - File: `src/main.rs`
  - Acceptance criteria:
    - `caldawarrior sync` runs a full sync
    - `caldawarrior sync --dry-run` prints planned operations and exits without writing
    - `caldawarrior --help` and `caldawarrior sync --help` print usage
    - Non-zero exit code on config error or fatal sync error
    - Warnings and logs go to stderr; dry-run output goes to stdout
    - `RUST_LOG=caldawarrior=debug` produces verbose trace output

- **Implement dry-run output formatter** `implementation` `small`
  - Description: In `src/output.rs`, implement `format_dry_run(ops: &[PlannedOp]) -> String`. Format: `[CREATE CalDAV] "Buy milk" (uuid-a)`, `[UPDATE TW] "Cook dinner" (uuid-b) — CalDAV wins LWW`, `[DELETE TW] "Go shopping" (uuid-c)`, `[SKIP] "Cancelled task" — CANCELLED`. Output is deterministic (sorted by caldav_uid).
  - File: `src/output.rs`
  - Acceptance criteria:
    - All operation types have distinct prefixes: CREATE, UPDATE, DELETE, SKIP
    - LWW winner noted on UPDATE lines
    - Output is sorted deterministically
    - Unit tests cover all operation types

- **Implement `print_warnings` in warnings.rs** `implementation` `small`
  - Description: In `src/warnings.rs` (existing), add `print_warnings(warnings: &[Warning])`. Print each warning to stderr with `[WARN]` prefix. `CyclicTasksExcluded` lists all task names and UUIDs. `UnsupportedReltype` is always one line (count + types). `RecurringSkipped` / `CancelledSkipped` / `UnresolvableDependency` each print per-item.
  - File: `src/warnings.rs`
  - Acceptance criteria:
    - All warning types print with distinct messages
    - `UnsupportedReltype` always produces exactly one line
    - Cycle warning lists all cyclic task names and UUIDs
    - Unit tests verify message format for each warning type

- **Phase 4 fidelity review** `verification` `fidelity`
  - Description: Compare Phase 4 implementation against spec — CLI, dry-run output, warning printing, logging
  - File: N/A
  - Acceptance criteria:
    - CLI interface matches spec (subcommands, flags, exit codes, stderr/stdout split)

#### Verification

- **Run tests:** `cargo test`
- **Fidelity review:** Compare implementation to spec Phase 4
- **Manual checks:** Run `caldawarrior sync --dry-run` against a test CalDAV server; verify output format and stderr/stdout split

---

### Phase 5: Testing

**Goal:** Comprehensive unit tests for all core logic modules and Docker-based integration tests against Radicale.

**Description:** Unit tests were developed in earlier phases. This phase adds the Docker integration test harness and six end-to-end scenarios. All scenarios run against a real Radicale CalDAV server.

#### Tasks

- **Implement Docker integration test harness** `implementation` `complex`
  - Description: Use the existing `tests/integration/docker-compose.yml` established in Phase 0 (not duplicated here). Create `tests/integration/mod.rs` with Rust test infrastructure: programmatic start/stop of the Radicale container, `caldawarrior` configuration setup, and full state reset between tests (TW database + CalDAV calendars wiped). The docker-compose infrastructure was verified in Phase 0; this task adds the Rust test scaffolding only.
  - File: `tests/integration/mod.rs`
  - Acceptance criteria:
    - Rust test harness can start/stop Radicale via the Phase 0 docker-compose file
    - Test harness can create/delete CalDAV calendars and TW tasks programmatically
    - State fully reset between test cases
    - Note: `docker-compose up` Radicale accessibility is verified in Phase 0; this task owns the Rust harness only

- **Integration test: first sync (TW → CalDAV)** `implementation` `medium`
  - Description: Create 3 TW tasks, run sync, verify 3 VTODOs appear in CalDAV with correct fields and `caldavuid` UDA set on each TW task.
  - File: `tests/integration/test_first_sync.rs`
  - Acceptance criteria:
    - All 3 tasks appear in CalDAV after sync with correct SUMMARY, DUE
    - `caldavuid` UDA is set on each TW task after sync

- **Integration test: first sync (CalDAV → TW)** `implementation` `medium`
  - Description: Create 2 VTODOs in CalDAV, run sync, verify 2 TW tasks with correct fields and `caldavuid`.
  - File: `tests/integration/test_first_sync.rs`
  - Acceptance criteria:
    - 2 TW tasks created with correct `caldavuid`, status, due date, description

- **Integration test: bidirectional update + LWW** `implementation` `medium`
  - Description: Sync a task, modify it in both TW (older timestamp) and CalDAV (newer timestamp). Run sync. Verify CalDAV side wins and its content appears in both systems.
  - File: `tests/integration/test_conflict.rs`
  - Acceptance criteria:
    - Newer CalDAV content appears in both systems after sync
    - No data corruption or spurious extra tasks

- **Integration test: dependency sync** `implementation` `medium`
  - Description: Create two TW tasks where A depends on B. Sync. Verify B's CalDAV UID appears in A's RELATED-TO. Add a dependency from CalDAV; verify it appears in TW on next sync.
  - File: `tests/integration/test_deps.rs`
  - Acceptance criteria:
    - `RELATED-TO;RELTYPE=DEPENDS-ON` correctly references B's UID
    - CalDAV-originated dependency appears as TW `depends` after sync

- **Integration test: deletion and CANCELLED** `implementation` `medium`
  - Description: Sync a task, delete in TW, sync again — verify CalDAV item is CANCELLED. Hard-delete a different VTODO from CalDAV, sync — verify TW task is deleted (not re-created).
  - File: `tests/integration/test_deletion.rs`
  - Acceptance criteria:
    - TW deletion → CalDAV CANCELLED (not hard-delete)
    - CalDAV deletion → TW task deleted; no re-creation loop on subsequent sync

- **Integration test: cycle detection** `implementation` `medium`
  - Description: Create a 3-task dependency cycle via CalDAV RELATED-TO. Run sync. Verify all 3 cyclic tasks are excluded with cycle warning; non-cyclic tasks in the same run sync normally.
  - File: `tests/integration/test_cycle.rs`
  - Acceptance criteria:
    - All 3 cyclic tasks excluded from write-back
    - Cycle warning lists all 3 tasks
    - Other tasks in the same sync run are unaffected

- **Phase 5 fidelity review** `verification` `fidelity`
  - Description: Compare Phase 5 test coverage against spec — all integration scenarios covered
  - File: N/A
  - Acceptance criteria:
    - All six integration scenarios have passing tests
    - Branch coverage ≥ 90% (verified via `cargo tarpaulin --branch`) for `src/mapper/`, `src/ir.rs`, `src/sync/lww.rs`, `src/sync/deps.rs`

#### Verification

- **Run tests:** `cargo test && docker-compose -f tests/integration/docker-compose.yml run integration-tests`
- **Fidelity review:** Compare test coverage to spec Phase 5
- **Manual checks:** Review test output for flaky tests

---

### Phase 6: Hardening & Documentation

**Goal:** Edge case hardening, credential security guidance, v2 roadmap documentation, and final README.

**Description:** Documents the 8 known limitations, adds security guidance for credential storage, and produces the v2 roadmap.

#### Tasks

- **Write README and configuration reference** `implementation` `small`
  - Description: Create `README.md` and `docs/configuration.md`. README: installation, quick-start (5 steps), known limitations (all 14 v1 known limitations). Configuration reference: all config fields, types, defaults, and examples. Security note: config file must have `0600` permissions; use `CALDAWARRIOR_PASSWORD` env var as a safer alternative.
  - File: `README.md`
  - Acceptance criteria:
    - Quick-start covers: install, configure (with security note), register UDA, first sync, dry-run
    - All 14 v1 known limitations are listed by name
    - All config fields documented with types, defaults, examples

- **Document v2 roadmap** `implementation` `small`
  - Description: Create `docs/v2-roadmap.md` covering: recurring task support (RRULE), field-level LWW merging (three-way merge), CANCELLED propagation to TW (opt-in), PARENT/CHILD hierarchy support, RFC 6578 sync tokens for incremental fetch, system keychain credential storage.
  - File: `docs/v2-roadmap.md`
  - Acceptance criteria:
    - Each v2 feature includes a brief design note and open questions
    - Roadmap referenced from README

- **Final fidelity review** `verification` `fidelity`
  - Description: Full spec fidelity review of the entire implementation
  - File: N/A
  - Acceptance criteria:
    - All phases implemented as specified
    - No unresolved critical deviations

#### Verification

- **Run tests:** `cargo test`
- **Fidelity review:** Final full spec fidelity review
- **Manual checks:** End-to-end manual test of quick-start from README; verify config file security note
