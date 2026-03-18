# Caldawarrior

## What This Is

A Rust CLI tool that bidirectionally syncs TaskWarrior tasks with CalDAV servers (primarily Radicale) via the VTODO standard. Born as a clone of [twcaldav](https://github.com/pcaro90/twcaldav/) but with support for task relations (depends/blocks) — something twcaldav lists as unimplemented. The primary use case is TaskWarrior + Radicale + tasks.org (Android), but it should work with any VTODO-compliant client.

## Core Value

Reliable bidirectional sync between TaskWarrior and CalDAV, including task dependency relationships that no other tool supports.

## Requirements

### Validated

<!-- Inferred from existing codebase — these capabilities exist today. -->

- ✓ Bidirectional sync of tasks between TW and CalDAV — existing
- ✓ Field mapping: description↔SUMMARY, status, priority, due, scheduled, wait, tags, annotations — existing
- ✓ LWW conflict resolution using TW.modified vs LAST-MODIFIED — existing
- ✓ IR-based sync pipeline (pair → classify → plan → execute) — existing
- ✓ ETag-based conditional writes with retry — existing
- ✓ Dependency resolution: TW depends UUIDs → CalDAV RELATED-TO UIDs — existing
- ✓ Cycle detection in dependency graphs — existing
- ✓ Project-to-calendar mapping via TOML config — existing
- ✓ Dry-run mode — existing
- ✓ Basic auth with env var password override — existing
- ✓ Completed task cutoff filtering — existing
- ✓ Recurring VTODO filtering (skip with warning) — existing
- ✓ Robot Framework blackbox test suite — existing
- ✓ Rust unit + integration tests with mocked adapters — existing

### Active

<!-- Current scope — hardening, assessment, and ship-readiness. -->

- [ ] Full code audit of all sync logic (fields, conflicts, create/update/delete, relations)
- [ ] Verify depends/blocks relation mapping works correctly end-to-end
- [ ] Verify bidirectional sync correctness for all field types
- [ ] E2E test robustification — cover gaps, edge cases, relation scenarios
- [ ] tasks.org compatibility verification (VTODO compliance)
- [ ] RFC 5545 VTODO compliance (pragmatic — don't break the spec)
- [ ] Docker image for self-hosting
- [ ] Ship-ready documentation (README, config examples)
- [ ] Binary release packaging

### Out of Scope

- Daemon/scheduler mode — caldawarrior is a sync binary, user decides invocation (cron, hooks, etc.)
- Parent/child hierarchical relations — only depends/blocks for now
- Real-time push notifications — pull-based sync only
- GUI or web interface — CLI only
- twcaldav feature parity gaps that don't apply (twcaldav has no relations either)

## Context

- **Origin**: Clone of [pcaro90/twcaldav](https://github.com/pcaro90/twcaldav/) (Python), rewritten in Rust with relation support added
- **twcaldav gap**: twcaldav has zero relation support — `depends` is listed as unimplemented roadmap item. Caldawarrior's relation handling is entirely original work.
- **Primary clients**: TaskWarrior 3.x, Radicale 3.3, tasks.org (Android). Should work with any VTODO client.
- **Current test coverage**: 170 Rust tests (148 unit + 4 main + 18 integration), 30 Robot Framework E2E tests. Three completed specs: field-mapping-fix, blackbox-integration-tests, native-lww-sync.
- **Sync model**: Bidirectional LWW. Tasks correlated via `caldavuid` UDA. No intermediate sync database.
- **Relation mapping**: TW `depends` (UUIDs) → CalDAV `RELATED-TO` with UIDs. Cycle detection via three-colour DFS. Current state: implemented but untested in production with real clients.

## Constraints

- **Tech stack**: Rust, must stay compatible with TaskWarrior 3.x CLI interface
- **CalDAV compliance**: Pragmatic RFC 5545 — prioritize tasks.org and common clients, but don't violate the spec
- **Testing**: Docker-based E2E tests with real TW + Radicale instances (no mocks for blackbox tests)

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust rewrite of twcaldav | Performance, type safety, single binary distribution | ✓ Good |
| RELATED-TO for depends | RFC 5545 standard property for task relations | — Pending verification |
| LWW conflict resolution | Simple, predictable, matches twcaldav approach | ✓ Good |
| No sync database | Stateless design, correlation via caldavuid UDA | ✓ Good |
| Pragmatic VTODO compliance | tasks.org primary, but don't break spec for other clients | — Pending |

---
*Last updated: 2026-03-18 after initialization*
