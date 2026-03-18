# Project Research Summary

**Project:** caldawarrior
**Domain:** CalDAV/VTODO bidirectional sync tool (TaskWarrior <-> CalDAV)
**Researched:** 2026-03-18
**Confidence:** HIGH

## Executive Summary

Caldawarrior is a mature single-binary Rust tool that syncs TaskWarrior and CalDAV servers bidirectionally, and this milestone is a hardening effort rather than a greenfield build. The core sync engine — VTODO field mapping, LWW conflict resolution, ETag-conditional writes, dependency graph handling, and the hand-rolled iCal parser — is already implemented and backed by 170 Rust tests and a 30-scenario Robot Framework E2E suite. The research confirms that the fundamental architecture is sound and that the primary goal now is closing the gap between "implemented" and "production-quality": E2E test coverage for all field operations, RFC 5545 compliance validation, and CI/CD infrastructure for repeatable releases.

The recommended approach is to sequence work by risk, not by feature. Three critical issues require code-level fixes before test expansion is meaningful: the CATEGORIES comma-escaping bug (splits tags with literal commas), the string-based XML parser in the CalDAV adapter (breaks on any server other than Radicale), and incomplete ETag normalization (will cause 412 loops against Nextcloud or Baikal). Once these are fixed, the E2E test suite should be expanded to cover the planned-but-unimplemented scenario groups (S-70..S-79 bulk operations, S-80..S-89 multi-sync journeys, cyclic dependency S-42), and then CI and packaging infrastructure completes the hardening arc.

The single highest risk is the XML parsing fragility: the CalDAV REPORT response parser uses naive string matching that only works with Radicale's specific namespace prefix. Any other CalDAV server silently drops VTODOs, making sync appear to work while losing data. Replacing this with `quick-xml` or `roxmltree` is non-negotiable before claiming multi-server compatibility. The DEPENDS-ON dependency feature, while correctly implemented, will be invisible to all current CalDAV clients (they support PARENT/CHILD, not DEPENDS-ON) — this is not a bug to fix but a limitation to document clearly.

## Key Findings

### Recommended Stack

The core stack is fixed and correct: Rust 2024 edition, reqwest 0.12 with rustls-tls, chrono 0.4, serde/serde_json 1, clap 4, anyhow 1, thiserror 2. No new Cargo dependencies are needed for the hardening milestone. The hardening toolchain consists entirely of external CI tools and one new crate for XML parsing.

**Core hardening tools:**
- `quick-xml` or `roxmltree`: replace string-based XML parser — required for multi-server CalDAV compatibility
- `cargo-deny` (0.19.0): license compliance + security advisory scanning — all-in-one supply chain auditing; subsumes cargo-audit
- `cargo-llvm-cov` (0.8.4): LLVM-based code coverage — more accurate than tarpaulin, cross-platform
- `Swatinem/rust-cache` + `dtolnay/rust-toolchain`: GitHub Actions CI — industry standard; replaces deprecated `actions-rs/*`
- `archlinux:base` runner in Dockerfile: required to ship TaskWarrior 3.x — Debian/Ubuntu only have TW 2.6.x; same base as existing test Dockerfile
- `git-cliff` (2.12.0): changelog generation from conventional commits — appropriate for v0.x release cadence
- Python `icalendar` (5.0.12): RFC 5545 validation in E2E tests — already in test stack, zero new dependency cost

**Files to create:** `Dockerfile` (production, slimmed from test Dockerfile), `deny.toml`, `.github/workflows/ci.yml`, `config.example.toml`, `CHANGELOG.md`

See `.planning/research/STACK.md` for full rationale and alternatives considered.

### Expected Features

The feature audit confirms that all table-stakes VTODO features are implemented (UID, DTSTAMP, SUMMARY, DESCRIPTION, STATUS, PRIORITY, DUE, DTSTART, COMPLETED, CATEGORIES, LAST-MODIFIED, ETag-conditional writes, iCal escaping, line folding, TZID parsing, extra_props preservation, bidirectional sync, LWW conflict resolution, loop prevention, dry-run). The hardening milestone is about verifying these work correctly end-to-end, not building new ones.

**Must verify (blocks "production-quality" claim):**
- DEPENDS-ON relation E2E with real Radicale server (currently unit-tested only)
- All field-clear operations: removing DUE/priority on one side must clear the other
- All status transitions E2E including CANCELLED propagation
- Deletion propagation in both directions (TW delete -> CalDAV CANCELLED; CalDAV delete -> TW orphan handling)
- Idempotent sync: zero writes on consecutive runs for every scenario

**Should verify (important for reliability):**
- DATE-only DUE values from tasks.org (`DUE;VALUE=DATE:20260315`)
- TZID timezone handling with DST edge cases
- Error recovery: auth failure, malformed VTODO, network timeout
- Non-standard property preservation: extra_props round-trip with real server and real client writes

**Defer to after hardening:**
- WebDAV-Sync (sync-token) for efficient fetching
- Custom field mapping configuration
- DURATION computation from DTSTART

**Anti-features (never build):** recurring VTODO sync, PERCENT-COMPLETE bidirectional mapping, subtask hierarchy, daemon mode, GUI, multi-account, VALARM mapping, SEQUENCE management, VTIMEZONE embedding.

See `.planning/research/FEATURES.md` for the full verification matrix.

### Architecture Approach

The testing architecture follows an inverted pyramid: more integration/E2E tests than unit tests, because sync correctness cannot be verified without real servers. The existing three-tier structure (148 unit tests, 18 integration tests via Docker, 30 RF E2E scenarios) is well-designed and extensible. Hardening work extends each tier without changing the architecture.

**Major components:**
1. **Unit Tests** (`src/**/mod.rs`) — pure business logic (mappers, LWW, iCal parsing, cycle detection); solid at 148 tests; primary gap is compliance assertions on raw VTODO output
2. **Rust Integration Tests** (`tests/integration/`) — full `run_sync()` calls against dockerized Radicale + TW via TestHarness; needs expansion for multi-calendar, RELATED-TO round-trips, ETag stress tests
3. **Robot Framework E2E** (`tests/robot/`) — black-box binary subprocess tests; 7 suites, 30 scenarios; needs 2 new suites (S-70..S-79 bulk operations, S-80..S-89 multi-sync journeys) plus S-42 cyclic fix
4. **Compliance Audit** (proposed, as unit tests) — property-level RFC 5545 assertions on generated VTODO output; catches bugs Radicale's leniency hides
5. **Client Fixture Import** (proposed) — real `.ics` files from tasks.org/Thunderbird as test fixtures; verifies caldawarrior handles real-world client quirks

**Critical finding on client compatibility:** No major CalDAV client currently renders `RELATED-TO;RELTYPE=DEPENDS-ON`. Clients that support RELATED-TO use PARENT/CHILD (Apple/Nextcloud convention, RFC 5545). DEPENDS-ON is defined in RFC 9253 (2022) and has no known client rendering support. DEPENDS-ON relations will be stored opaquely — test for preservation, not display.

**Build order for components:** Compliance audit (no infra, immediate) → E2E expansion (RF suites) → Integration expansion (Rust harness) → Client fixture collection (manual effort) → Fixture import tests → Manual verification protocol.

See `.planning/research/ARCHITECTURE.md` for the full component breakdown, patterns to follow, and anti-patterns to avoid.

### Critical Pitfalls

1. **XML parsing fragility** (PITFALL 6, CRITICAL) — `caldav_adapter.rs` uses string matching on `D:response` tags; breaks with any other namespace prefix or CDATA or compact XML. Fix: replace with `quick-xml` or `roxmltree`. Highest-risk technical debt in the codebase.

2. **CATEGORIES escaping/splitting bug** (PITFALL 1, CRITICAL) — `ical.rs` splits CATEGORIES on raw commas without unescaping first; tags with literal commas are silently split and never recombined across sync cycles. Fix: unescape before splitting, escape on serialize.

3. **Sync loop from new field additions** (PITFALL 2, HIGH) — every new synced field must be added to `content_identical()` in `lww.rs`; missing entries cause ping-pong loops where every sync triggers a write. Mitigation: consecutive-sync integration tests are mandatory for every field change.

4. **Ghost tasks after TW purge** (PITFALL 5, HIGH) — `task purge` severs the caldavuid link; CalDAV VTODO has no matching TW task and re-imports as a new task on next sync. Mitigation: verify CANCELLED VTODOs are skipped during import; document `task purge` limitation explicitly.

5. **ETag quoting for non-Radicale servers** (PITFALL 3, MEDIUM) — weak ETags (`W/"abc123"`) are mishandled by current normalization (`trim_matches('"')` strips the closing quote only), causing perpetual 412 loops on Nextcloud/Baikal. Fix: normalize on receipt (strip `W/` prefix, ensure surrounding quotes).

6. **DEPENDS-ON is RFC 9253, not RFC 5545** (PITFALL 4, HIGH) — clients only knowing RFC 5545 will ignore or misinterpret DEPENDS-ON RELTYPE. Mitigation: verify extra_props round-trip preserves DEPENDS-ON through tasks.org + DAVx5; document the limitation clearly in README.

See `.planning/research/PITFALLS.md` for the complete 17-pitfall catalog with phase-specific warnings.

## Implications for Roadmap

Based on the research, the hardening milestone decomposes into six phases ordered by dependency and risk.

### Phase 1: Code Audit and Bug Fixes
**Rationale:** Three confirmed code-level bugs must be fixed before test expansion is meaningful — writing tests against buggy code validates the wrong behavior. This phase has no dependencies and unblocks all subsequent phases.
**Delivers:** Fixed CATEGORIES escaping (split-after-unescape), proper XML parser replacing string matching, ETag normalization for weak ETags, floating datetime warning, PERCENT-COMPLETE=100 read-as-COMPLETED compatibility.
**Addresses:** FEATURES prerequisites; PITFALLS 1 (CATEGORIES), 3 (ETag quoting), 6 (XML parsing), 7 (floating datetime), 10 (PERCENT-COMPLETE).
**Avoids:** Writing E2E tests that validate incorrect behavior as correct.

### Phase 2: RFC Compliance Verification
**Rationale:** Compliance assertions catch bugs that Radicale's leniency hides but stricter servers and clients expose. Run after code fixes to establish a clean RFC 5545 baseline. Pure unit tests — no infrastructure changes required.
**Delivers:** Unit tests asserting: required VTODO properties present (UID, DTSTAMP), datetime format correctness (UTC Z suffix), RELATED-TO;RELTYPE=DEPENDS-ON structure, line folding at exactly 75 octets, CRLF line endings, PRIORITY in range 0-9, STATUS limited to four valid values.
**Uses:** Rust `#[test]` in `src/ical.rs`; Python `icalendar` library for property-level assertions in RF fixtures.
**Avoids:** PITFALLS 7 (floating datetime silently treated as UTC), 8 (VTIMEZONE stripping), 12 (UTF-8 multi-byte fold boundary).

### Phase 3: E2E Test Expansion
**Rationale:** The RF infrastructure is proven and the CATALOG.md already defines the unimplemented scenario groups. This phase fills planned gaps to achieve comprehensive behavioral coverage.
**Delivers:** S-42 (cyclic dependency, needs CLI-level verification); S-70..S-79 (bulk operations: 100+ tasks, multi-calendar routing); S-80..S-89 (multi-sync journeys: field clear, all status transitions, deletion propagation both directions, DEPENDS-ON round-trip); idempotency (second-sync zero-writes) assertions added to all existing tests.
**Implements:** Architecture Patterns 1 (Seed-Sync-Assert), 2 (Multi-Cycle Stability), 4 (Raw iCal Inspection for RELATED-TO structure).
**Avoids:** PITFALL 15 (Radicale timing — use `Wait Until Keyword Succeeds`), PITFALL 16 (annotation edge cases — test combinatorial annotation states).

### Phase 4: Integration Test Expansion
**Rationale:** Rust integration tests exercise the sync engine directly at library level and are faster than RF for property-level verification. Parallelize with Phase 3 (different test harness, independent work stream).
**Delivers:** Multi-calendar sync tests (project -> calendar routing), RELATED-TO round-trip integration tests, ETag conflict stress tests, field-clear integration tests, consecutive-sync stability assertions for every new scenario.
**Uses:** Existing `TestHarness` struct; `DockerizedTaskRunner`; Radicale Docker Compose.
**Avoids:** PITFALL 2 (sync loops) — stability assertions are mandatory in every new test.

### Phase 5: CI/CD and Packaging
**Rationale:** With tests passing and bugs fixed, establish the automation infrastructure that keeps them passing. This phase is a prerequisite for release claims.
**Delivers:** `.github/workflows/ci.yml` (fmt + clippy + cargo test + E2E; cargo-deny; cargo-llvm-cov + Codecov upload); `deny.toml` (license + advisory config); production `Dockerfile` (archlinux:base runner, no Python/RF, pinned TW version); `config.example.toml`; binary release workflow (GitHub Releases with assets).
**Uses:** GitHub Actions, `Swatinem/rust-cache`, `dtolnay/rust-toolchain`, `cargo-deny`, `cargo-llvm-cov`, `archlinux:base`.
**Avoids:** PITFALL 13 (Docker cache invalidation — two-stage dep caching), PITFALL 14 (rolling Arch breaks TW version — pin `task=3.x.y-z` in Dockerfile), PITFALL 17 (static linking — verify with `ldd` before publishing binaries).

### Phase 6: Documentation and Release
**Rationale:** Final hardening step. README, CHANGELOG, and the release process formalize the v0.x ship. Client fixture collection and manual verification protocol document what automated tests cannot cover.
**Delivers:** `README.md` (installation, quick-start, config reference, Docker usage, cron/systemd examples, known limitations including DEPENDS-ON visibility and `task purge` warning); `CHANGELOG.md` (generated with `git-cliff`); GitHub Release with Linux x86_64 and aarch64 binaries; client fixture directory (`tests/robot/fixtures/tasks-org/`, `tests/robot/fixtures/thunderbird/`); manual verification protocol for tasks.org + Thunderbird.
**Uses:** `git-cliff`; `cargo doc` in CI (catch broken doc links); clap `--help` text audit.
**Implements:** Architecture component 5 (Client Fixture Import) and the Manual Verification Protocol.

### Phase Ordering Rationale

- Phase 1 (bugs) must precede Phases 2, 3, and 4 — tests written against buggy code validate wrong behavior.
- Phases 3 and 4 (E2E + integration expansion) can run in parallel since they use independent test harnesses.
- Phase 5 (CI) can begin in parallel with Phase 3/4 — the workflow skeleton is independent of test count.
- Phase 6 (docs/release) must follow all others — it documents what the passing tests verify.
- The XML parser replacement (Phase 1) is the highest-risk item; front-loading it reduces risk for all subsequent phases and prevents the "works with Radicale only" trap.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 1 (XML parser replacement):** Evaluate `quick-xml` vs `roxmltree` API fit for namespace-aware REPORT response parsing. Neither is currently in Cargo.toml; assess API surface and maintenance status.
- **Phase 3 (client fixture collection):** tasks.org fixture collection requires a real Android device with DAVx5 + Radicale; this is manual effort that must be scheduled as a distinct activity, not an automated task.
- **Phase 6 (binary release):** Musl static linking feasibility depends on transitive deps; verify with `cargo build --target x86_64-unknown-linux-musl` and `ldd` before committing to binary distribution format.

Phases with standard patterns (skip research-phase):
- **Phase 2 (compliance):** RFC 5545 requirements are authoritative and unambiguous; no research needed.
- **Phase 4 (integration):** TestHarness patterns are established; incremental expansion only.
- **Phase 5 (CI/CD):** GitHub Actions + cargo toolchain is well-documented; the CI YAML structure is fully spelled out in STACK.md research.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All tool versions verified on crates.io; archlinux:base constraint empirically confirmed by existing test Dockerfile; no speculative choices |
| Features | HIGH | Table stakes derived from RFC 5545 (authoritative); differentiators confirmed against competing tools; anti-features supported by multiple real-world client bug reports |
| Architecture | HIGH | Current test infrastructure examined directly in codebase; component boundaries are clear; client RELATED-TO limitation confirmed from multiple sources (DAVx5 FAQ, tasks.org docs, Thunderbird Bugzilla #194863) |
| Pitfalls | HIGH | Pitfalls 1, 2, 3, 5, 6, 7, 8, 12, 13, 14 directly observed in codebase with line citations; Pitfall 4 confirmed by RFC 9253 publication date and client documentation; only Pitfalls 9, 10, 15, 17 are MEDIUM (need runtime validation) |

**Overall confidence:** HIGH

### Gaps to Address

- **tasks.org PERCENT-COMPLETE emission behavior**: Research recommends treating PERCENT-COMPLETE=100 as COMPLETED, but tasks.org's exact output needs empirical verification with a real device. Handle during Phase 3 client fixture collection.
- **Musl static linking feasibility**: `rustls-tls` avoids the OpenSSL pitfall but transitive deps may pull in C libraries. Verify with `ldd` during Phase 5 before committing to binary release format.
- **DEPENDS-ON extra_props preservation through tasks.org + DAVx5**: Theoretically preserved via extra_props round-trip mechanism; needs E2E verification that tasks.org + DAVx5 does not strip unknown RELATED-TO RELTYPE values on write-back.
- **Multi-server ETag format behavior**: Radicale returns properly quoted ETags; Nextcloud/Baikal weak ETag behavior is inferred from other sync tool bug reports, not direct testing. Verify with unit tests for ETag normalization edge cases.

## Sources

### Primary (HIGH confidence)
- [RFC 5545 - iCalendar Specification](https://www.rfc-editor.org/rfc/rfc5545.html) — VTODO component, TEXT escaping, line folding, CATEGORIES, RELATED-TO
- [RFC 9253 - iCalendar Relationships](https://datatracker.ietf.org/doc/html/rfc9253) — DEPENDS-ON relationship type definition and semantics
- [RFC 4791 - CalDAV Access](https://www.ietf.org/rfc/rfc4791.txt) — REPORT, ETag usage, sync protocol
- [cargo-deny 0.19.0](https://crates.io/crates/cargo-deny), [cargo-llvm-cov 0.8.4](https://crates.io/crates/cargo-llvm-cov), [cargo-audit 0.22.1](https://crates.io/crates/cargo-audit), [git-cliff 2.12.0](https://crates.io/crates/git-cliff) — versions verified on crates.io
- [Swatinem/rust-cache v2](https://github.com/Swatinem/rust-cache), [dtolnay/rust-toolchain](https://github.com/dtolnay/rust-toolchain) — CI toolchain
- Internal codebase: `ical.rs`, `caldav_adapter.rs`, `sync/lww.rs`, `sync/writeback.rs`, `mapper/fields.rs` — direct analysis with line citations
- Internal docs: `tests/robot/docs/CATALOG.md`, `tests/robot/docs/GAP_ANALYSIS.md`, `docs/adr/tw-field-clearing.md`, `docs/adr/loop-prevention.md`

### Secondary (MEDIUM confidence)
- [sabre/dav - Building a CalDAV Client](https://sabre.io/dav/building-a-caldav-client/) — best practices: property preservation, ETag handling, sync strategies
- [tasks.org CalDAV Documentation](https://tasks.org/docs/caldav_intro.html) — server compatibility, VTODO support, X-APPLE-SORT-ORDER usage
- [DAVx5 FAQ: Advanced Task Features](https://www.davx5.com/faq/tasks/advanced-task-features) — client compatibility matrix for VTODO features
- [syncall tw-caldav](https://github.com/eigenmannmartin/syncall/blob/master/readme-tw-caldav.md) — competing tool; no relation support, no waiting task support
- [Outlook CalDav Synchronizer](https://github.com/aluxnimm/outlookcaldavsynchronizer) — sync loop prevention patterns, ETag handling patterns
- [tasks.org issues #3023, #932, #1261](https://github.com/tasks/tasks) — subtask hierarchy and recurring VTODO known bugs in tasks.org
- [ownCloud Tasks Issue #137](https://github.com/owncloud/tasks/issues/137) — PERCENT-COMPLETE=100 without STATUS:COMPLETED bug

### Tertiary (LOW confidence)
- [Thunderbird Bugzilla #194863](https://bugzilla.mozilla.org/show_bug.cgi?id=194863) — VTODO hierarchy support absent since 2003; current behavior needs validation
- [Raniz Blog - Rust MUSL Performance](https://raniz.blog/2025-02-06_rust-musl-malloc/) — musl allocator issues; relevant only if pursuing static binary distribution

---
*Research completed: 2026-03-18*
*Ready for roadmap: yes*
