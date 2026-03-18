# Requirements: Caldawarrior Hardening

**Defined:** 2026-03-18
**Core Value:** Reliable bidirectional sync between TaskWarrior and CalDAV, including task dependency relationships that no other tool supports.

## v1 Requirements

Requirements for hardening milestone. Each maps to roadmap phases.

### Code Audit

- [ ] **AUDIT-01**: CATEGORIES comma-escaping bug fixed — tags containing commas no longer silently split
- [ ] **AUDIT-02**: XML parser replaced with proper XML library — CalDAV responses from non-Radicale servers parse correctly
- [ ] **AUDIT-03**: Error messages improved — no swallowed context from unwrap_or_default paths
- [ ] **AUDIT-04**: ETag normalization handles weak ETags — no 412 loops on Nextcloud/Baikal

### Relation Verification

- [ ] **REL-01**: DEPENDS-ON relation syncs end-to-end with real Radicale server — TW depends UUIDs map to CalDAV RELATED-TO UIDs and back
- [ ] **REL-02**: Cycle detection works end-to-end — circular dependencies are detected, warned, and skipped without data loss
- [ ] **REL-03**: tasks.org compatibility verified — DEPENDS-ON properties preserved through tasks.org+DAVx5 round-trip (or documented limitation)
- [ ] **REL-04**: blocks (inverse depends) mapping verified — TW blocks correctly maps to RELATED-TO in reverse direction

### Field Mapping

- [ ] **FIELD-01**: All mapped fields have E2E tests covering create, update, and clear operations (SUMMARY, DESCRIPTION, STATUS, PRIORITY, DUE, DTSTART, COMPLETED, CATEGORIES, RELATED-TO, X-TASKWARRIOR-WAIT)
- [ ] **FIELD-02**: All status transitions tested E2E — pending↔completed, pending→deleted→CANCELLED, and reverse paths
- [ ] **FIELD-03**: Deletion propagation tested both directions — TW delete→CalDAV CANCELLED, CalDAV orphan→TW handling
- [ ] **FIELD-04**: Idempotent sync verified — re-running sync after any operation produces no changes

### Compatibility

- [ ] **COMPAT-01**: XML parser handles Radicale, Nextcloud, and Baikal response formats without data loss
- [ ] **COMPAT-02**: DATE-only DUE values (YYYYMMDD) parse and round-trip correctly
- [ ] **COMPAT-03**: TZID datetime handling works for common timezones including DST transitions
- [ ] **COMPAT-04**: Non-standard properties (X-props) from other clients survive round-trip sync

### Packaging

- [ ] **PKG-01**: GitHub Actions CI pipeline runs unit tests, integration tests, RF E2E tests, and cargo-deny security audit
- [ ] **PKG-02**: Pre-built binary releases published on GitHub Releases for x86_64-linux

### Documentation

- [ ] **DOC-01**: README covers installation, configuration, usage, and common workflows
- [ ] **DOC-02**: All config.toml options documented with examples and defaults
- [ ] **DOC-03**: CHANGELOG generated from git history
- [ ] **DOC-04**: Client/server compatibility matrix documenting tested combinations and known limitations

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Multi-Server

- **SERV-01**: Nextcloud CalDAV full E2E test suite
- **SERV-02**: Baikal CalDAV full E2E test suite

### Packaging

- **PKG-03**: Binary releases for aarch64-linux and macOS
- **PKG-04**: Published on crates.io via cargo install

### Performance

- **PERF-01**: WebDAV-Sync (sync-token) for efficient incremental fetching
- **PERF-02**: Performance benchmarks for 500+ tasks with dependencies

## Out of Scope

| Feature | Reason |
|---------|--------|
| Docker production image | CLI tool — users have TW installed locally, Docker adds friction for zero benefit |
| Daemon/scheduler mode | Sync binary — user controls invocation via cron, hooks, etc. |
| Parent/child subtask hierarchy | TW has no native subtask model; mapping is lossy; tasks.org subtask sync has documented bugs |
| PERCENT-COMPLETE mapping | TW has no percent-complete concept; known source of sync bugs across clients |
| VALARM/reminder sync | TW has no alarm concept; client-specific implementations |
| Recurring VTODO sync (RRULE) | Different semantics between TW and CalDAV; known interop nightmare |
| GUI or web interface | CLI tool for CLI users |
| Multi-user/multi-account | Single-account design; run separate instances for multiple accounts |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| AUDIT-01 | TBD | Pending |
| AUDIT-02 | TBD | Pending |
| AUDIT-03 | TBD | Pending |
| AUDIT-04 | TBD | Pending |
| REL-01 | TBD | Pending |
| REL-02 | TBD | Pending |
| REL-03 | TBD | Pending |
| REL-04 | TBD | Pending |
| FIELD-01 | TBD | Pending |
| FIELD-02 | TBD | Pending |
| FIELD-03 | TBD | Pending |
| FIELD-04 | TBD | Pending |
| COMPAT-01 | TBD | Pending |
| COMPAT-02 | TBD | Pending |
| COMPAT-03 | TBD | Pending |
| COMPAT-04 | TBD | Pending |
| PKG-01 | TBD | Pending |
| PKG-02 | TBD | Pending |
| DOC-01 | TBD | Pending |
| DOC-02 | TBD | Pending |
| DOC-03 | TBD | Pending |
| DOC-04 | TBD | Pending |

**Coverage:**
- v1 requirements: 22 total
- Mapped to phases: 0
- Unmapped: 22 ⚠️

---
*Requirements defined: 2026-03-18*
*Last updated: 2026-03-18 after initial definition*
