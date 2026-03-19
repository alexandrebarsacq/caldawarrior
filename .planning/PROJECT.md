# Caldawarrior

## What This Is

A Rust CLI tool that bidirectionally syncs TaskWarrior tasks with CalDAV servers via the VTODO standard. Supports task dependency relations (depends/blocks) via RELATED-TO — something no other TW-CalDAV bridge implements. Ships as a single binary with full CI/CD pipeline.

## Core Value

Reliable bidirectional sync between TaskWarrior and CalDAV, including task dependency relationships that no other tool supports.

## Requirements

### Validated

- ✓ Bidirectional sync of tasks between TW and CalDAV — v1.0
- ✓ Field mapping: description↔SUMMARY, status, priority, due, scheduled, wait, tags, annotations↔DESCRIPTION — v1.0
- ✓ LWW conflict resolution using TW.modified vs LAST-MODIFIED — v1.0
- ✓ Dependency resolution: TW depends UUIDs → CalDAV RELATED-TO UIDs with cycle detection — v1.0
- ✓ CATEGORIES comma-escaping bug fixed — v1.0
- ✓ XML parser handles arbitrary namespace prefixes (quick-xml NsReader) — v1.0
- ✓ ETag normalization handles weak ETags — v1.0
- ✓ Error messages include original context — v1.0
- ✓ DATE-only values and DST timezone handling — v1.0
- ✓ X-property preservation across sync — v1.0
- ✓ Idempotent sync (re-run produces zero writes) — v1.0
- ✓ CI pipeline (lint/test/e2e/audit) and binary releases — v1.0
- ✓ Full README, config reference, compatibility matrix — v1.0
- ✓ 80 Robot Framework E2E tests, 192 Rust unit tests — v1.0

### Active

- [ ] Nextcloud CalDAV full E2E test suite
- [ ] Baikal CalDAV full E2E test suite
- [ ] Binary releases for aarch64-linux and macOS
- [ ] Published on crates.io via cargo install
- [ ] WebDAV-Sync (sync-token) for efficient incremental fetching
- [ ] Performance benchmarks for 500+ tasks

### Out of Scope

- Daemon/scheduler mode — sync binary, user controls invocation via cron/hooks
- Parent/child subtask hierarchy — TW has no native subtask model
- PERCENT-COMPLETE mapping — TW has no percent-complete concept
- VALARM/reminder sync — TW has no alarm concept
- Recurring VTODO sync (RRULE) — different semantics between TW and CalDAV
- GUI or web interface — CLI tool for CLI users
- Multi-user/multi-account — run separate instances

## Context

- **Origin**: Clone of [pcaro90/twcaldav](https://github.com/pcaro90/twcaldav/) (Python), rewritten in Rust with relation support
- **Primary clients**: TaskWarrior 3.x, Radicale 3.3, tasks.org (Android)
- **Codebase**: 8,400 lines of Rust, 80 RF E2E tests, 192 unit tests, 18 integration tests
- **Sync model**: Bidirectional LWW. Tasks correlated via `caldavuid` UDA. No intermediate sync database.
- **Shipped**: v1.0 on 2026-03-19 after 7-phase hardening milestone

## Constraints

- **Tech stack**: Rust, must stay compatible with TaskWarrior 3.x CLI interface
- **CalDAV compliance**: Pragmatic RFC 5545 — prioritize tasks.org and common clients
- **Testing**: Docker-based E2E tests with real TW + Radicale instances (no mocks for blackbox tests)

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust rewrite of twcaldav | Performance, type safety, single binary distribution | ✓ Good |
| RELATED-TO for depends | RFC 5545 standard property for task relations | ✓ Good — works E2E, invisible to clients but preserved |
| LWW conflict resolution | Simple, predictable, matches twcaldav approach | ✓ Good |
| No sync database | Stateless design, correlation via caldavuid UDA | ✓ Good |
| quick-xml NsReader | Namespace-aware XML parsing for any CalDAV server | ✓ Good — handles arbitrary prefixes |
| task modify over task import | task import drops caldavuid UDA in TW3 Docker | ✓ Good — fixed perpetual re-sync bug |
| DST .latest() fallback | Standard-time interpretation for ambiguous fall-back times | ✓ Good — matches RFC 5545 |
| Cargo edition 2024 with let chains | Cleaner code with chained if-let expressions | ⚠️ Revisit — required bumping Docker Rust from 1.85 to 1.94 |

---
*Last updated: 2026-03-19 after v1.0 milestone*
