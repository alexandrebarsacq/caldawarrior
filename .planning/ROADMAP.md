# Roadmap: Caldawarrior Hardening

## Overview

Caldawarrior's core sync engine is implemented and backed by 170 Rust tests and 30 RF E2E scenarios, but three confirmed bugs block production-quality claims and key capabilities (relations, field completeness, multi-server compatibility) lack end-to-end verification. This roadmap sequences work by risk: fix known bugs first so tests validate correct behavior, then verify the differentiator (dependency relations), core correctness (all fields), and interoperability (other servers/clients), then automate and ship.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Code Audit and Bug Fixes** - Fix CATEGORIES escaping, XML parser, ETag normalization, and error handling before any test expansion
- [ ] **Phase 2: Relation Verification** - Prove dependency relations work end-to-end with real servers and document client limitations
- [ ] **Phase 3: Field and Sync Correctness** - Verify all mapped fields, status transitions, deletion, and idempotency with E2E tests
- [ ] **Phase 4: Compatibility** - Verify multi-server XML parsing, DATE-only values, timezone handling, and property preservation
- [ ] **Phase 5: CI/CD and Packaging** - Automated pipeline running all tests, security audit, and binary release publishing
- [ ] **Phase 6: Documentation and Release** - README, config reference, changelog, compatibility matrix, and ship

## Phase Details

### Phase 1: Code Audit and Bug Fixes
**Goal**: Known bugs are fixed so that all subsequent testing validates correct behavior
**Depends on**: Nothing (first phase)
**Requirements**: AUDIT-01, AUDIT-02, AUDIT-03, AUDIT-04
**Success Criteria** (what must be TRUE):
  1. Tags containing commas survive a full sync round-trip without being split into separate tags
  2. CalDAV REPORT responses using arbitrary XML namespace prefixes parse correctly (not just Radicale's `D:response`)
  3. Sync errors include the original context (task UUID, field name, server URL) instead of swallowed defaults
  4. Weak ETags from Nextcloud/Baikal normalize correctly and do not cause 412 Precondition Failed loops
**Plans**: 3 plans

Plans:
- [x] 01-01-PLAN.md — Fix CATEGORIES comma-escaping and TW tags-to-CATEGORIES mapping (AUDIT-01)
- [ ] 01-02-PLAN.md — Replace XML parser with quick-xml NsReader and add ETag normalization (AUDIT-02, AUDIT-04)
- [ ] 01-03-PLAN.md — Fix error context swallowing and add --fail-fast flag (AUDIT-03)

### Phase 2: Relation Verification
**Goal**: Dependency relations -- caldawarrior's differentiator -- are proven to work end-to-end with real servers
**Depends on**: Phase 1
**Requirements**: REL-01, REL-02, REL-03, REL-04
**Success Criteria** (what must be TRUE):
  1. A TW task with `depends:UUID` syncs to CalDAV as `RELATED-TO;RELTYPE=DEPENDS-ON` with the correct UID, and syncing back restores the dependency
  2. A circular dependency chain (A depends B depends C depends A) is detected during sync, logged as a warning, and skipped without corrupting any task
  3. DEPENDS-ON properties survive a round-trip through tasks.org + DAVx5, or the limitation is documented with evidence
  4. TW `blocks` relationships (inverse depends) produce the correct `RELATED-TO` mapping in CalDAV
**Plans**: 2 plans

Plans:
- [ ] 02-01-PLAN.md — Change cyclic entry handling from skip to sync-without-deps (REL-02)
- [ ] 02-02-PLAN.md — E2E dependency tests, blocks verification, dep removal, and tasks.org compatibility docs (REL-01, REL-02, REL-03, REL-04)

### Phase 3: Field and Sync Correctness
**Goal**: Every mapped field creates, updates, clears, and round-trips correctly, and sync is idempotent
**Depends on**: Phase 1
**Requirements**: FIELD-01, FIELD-02, FIELD-03, FIELD-04
**Success Criteria** (what must be TRUE):
  1. Every mapped field (SUMMARY, DESCRIPTION, STATUS, PRIORITY, DUE, DTSTART, COMPLETED, CATEGORIES, RELATED-TO, X-TASKWARRIOR-WAIT) has passing E2E tests for create, update, and clear operations
  2. All status transitions (pending to completed, pending to deleted/CANCELLED, completed back to pending) work bidirectionally with correct field side-effects (COMPLETED timestamp set/cleared)
  3. Deleting a task on either side propagates correctly: TW delete produces CalDAV CANCELLED, CalDAV-side orphan is handled without creating ghost tasks
  4. Running sync twice consecutively after any operation produces zero writes on the second run
**Plans**: 2 plans

Plans:
- [ ] 03-01-PLAN.md — Fix CANCELLED propagation bug, CalDAVLibrary infrastructure fixes, status transition and deletion E2E tests (FIELD-02, FIELD-03)
- [ ] 03-02-PLAN.md — Comprehensive field create/update/clear E2E tests and dedicated idempotency suite (FIELD-01, FIELD-04)

### Phase 4: Compatibility
**Goal**: Caldawarrior handles real-world data formats from multiple servers and clients without data loss
**Depends on**: Phase 1
**Requirements**: COMPAT-01, COMPAT-02, COMPAT-03, COMPAT-04
**Success Criteria** (what must be TRUE):
  1. CalDAV REPORT responses from Radicale, Nextcloud, and Baikal (or representative fixture data) parse without dropping any VTODOs
  2. DATE-only DUE values (YYYYMMDD without time component) parse correctly and survive a sync round-trip without gaining a spurious time component
  3. VTODO datetimes with TZID parameters for common timezones (including across DST transitions) parse and round-trip correctly
  4. Non-standard properties (X-APPLE-SORT-ORDER, X-OC-HIDESUBTASKS, etc.) written by other clients survive a caldawarrior sync without being stripped
**Plans**: TBD

Plans:
- [ ] 04-01: TBD
- [ ] 04-02: TBD

### Phase 5: CI/CD and Packaging
**Goal**: Every commit is automatically tested, audited, and release-ready binaries are one push away
**Depends on**: Phase 1
**Requirements**: PKG-01, PKG-02
**Success Criteria** (what must be TRUE):
  1. A GitHub Actions CI workflow runs cargo fmt check, clippy, unit tests, integration tests, RF E2E tests, and cargo-deny on every push and PR
  2. Tagging a release version triggers automatic build and publication of a pre-built x86_64-linux binary to GitHub Releases
**Plans**: TBD

Plans:
- [ ] 05-01: TBD

### Phase 6: Documentation and Release
**Goal**: A new user can install, configure, and use caldawarrior from the README alone, with known limitations clearly documented
**Depends on**: Phase 1, Phase 2, Phase 3, Phase 4, Phase 5
**Requirements**: DOC-01, DOC-02, DOC-03, DOC-04
**Success Criteria** (what must be TRUE):
  1. README covers installation (binary and cargo install), config.toml setup, first sync walkthrough, and cron/systemd scheduling
  2. Every config.toml option is documented with its type, default value, and an example
  3. CHANGELOG exists with entries generated from git history covering the hardening milestone
  4. A compatibility matrix documents which server/client combinations are tested, which are expected to work, and what limitations exist (including DEPENDS-ON client visibility)
**Plans**: TBD

Plans:
- [ ] 06-01: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3 -> 4 -> 5 -> 6
Note: Phases 2, 3, 4 can execute in parallel after Phase 1 completes. Phase 5 can overlap with 2-4.

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Code Audit and Bug Fixes | 3/3 | Complete | 2026-03-18 |
| 2. Relation Verification | 2/2 | Complete | 2026-03-19 |
| 3. Field and Sync Correctness | 0/2 | Planned | - |
| 4. Compatibility | 0/? | Not started | - |
| 5. CI/CD and Packaging | 0/? | Not started | - |
| 6. Documentation and Release | 0/? | Not started | - |
