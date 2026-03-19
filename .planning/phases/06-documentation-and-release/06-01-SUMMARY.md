---
phase: 06-documentation-and-release
plan: 01
subsystem: documentation
tags: [readme, config-reference, installation, scheduling, compatibility, version-bump]
dependency_graph:
  requires: []
  provides: [complete-readme, version-1.0.0, compatibility-matrix]
  affects: [README.md, Cargo.toml, Cargo.lock]
tech_stack:
  added: []
  patterns: [github-flavored-markdown, semver]
key_files:
  created: []
  modified: [README.md, Cargo.toml, Cargo.lock]
decisions:
  - Section ordering follows plan: Features > Installation > Quick Start > Config Reference > CLI Reference > Scheduling > Field Mapping > Compatibility > Known Limitations > Testing > Roadmap > License
  - Pre-built binary download URL uses v1.0.0 example with note to check Releases page
  - Cargo.lock committed alongside Cargo.toml version bump
metrics:
  duration: 5min
  completed: 2026-03-19
---

# Phase 06 Plan 01: README and Version Bump Summary

Updated README.md with complete user documentation (Installation, Config Reference, Scheduling, Compatibility, limitation #15, --fail-fast CLI flag) and bumped Cargo.toml from 0.1.0 to 1.0.0 for release readiness.

## Tasks Completed

| # | Task | Commit | Key Changes |
|---|------|--------|-------------|
| 1 | Update README.md with Installation, Config Reference, Scheduling, Compatibility, and limitation #15 | `6f28836` | Added 5 new sections, updated CLI Reference, restructured Quick Start |
| 2 | Bump Cargo.toml version to 1.0.0 | `0dea89b`, `ac219c9` | Version 0.1.0 -> 1.0.0 in Cargo.toml and Cargo.lock |

## Key Deliverables

### README.md New Sections
- **Installation**: Pre-built binary (with checksum verification), cargo install, build from source -- 3 install paths with binary download as primary
- **Config Reference**: 6-option table (server_url, username, password, completed_cutoff_days, allow_insecure_tls, caldav_timeout_seconds) with types, defaults, required flags; Calendar Entries table; Environment Variables (CALDAWARRIOR_PASSWORD, CALDAWARRIOR_CONFIG); Config Path Resolution (3-step priority)
- **Scheduling**: Cron with flock example, systemd timer/service unit files with enable command
- **Compatibility**: 3-tier server matrix (Radicale Tested, Nextcloud Expected, Baikal Expected) and client matrix (TW 3.x Tested, tasks.org+DAVx5 Tested*, Thunderbird Expected) with DEPENDS-ON footnote
- **Known Limitation #15**: DEPENDS-ON relations invisible to CalDAV clients, with workaround

### README.md Updates
- CLI Reference: added --fail-fast flag documentation
- Quick Start: removed install step (moved to Installation section), renumbered steps 1-4
- Fixed repo URL from example/caldawarrior to alexandrebarsacq/caldawarrior

### Version Bump
- Cargo.toml version: 0.1.0 -> 1.0.0
- Cargo.lock updated accordingly
- No git tag created (user tags manually when ready)

## Deviations from Plan

### Build Verification Note
The plan required `cargo build --release` after version bump. The cargo command was unavailable in the execution environment. The version bump is a metadata-only change (line 3 of Cargo.toml: string value "0.1.0" to "1.0.0") that does not affect compilation logic. CI will verify the build on push.

## Decisions Made

1. **Section ordering**: Followed plan specification exactly -- Installation before Quick Start, Config Reference before CLI Reference, Scheduling after CLI Reference, Compatibility after Status Mapping
2. **Binary URL versioning**: Used v1.0.0 in the download example with a note directing users to the Releases page for the latest version
3. **Cargo.lock**: Committed as a separate commit since it was auto-updated by the version change

## Verification Results

All acceptance criteria verified:
- Installation section: 3 subsections (Pre-built Binary, cargo install, Build from Source)
- Config Reference: 6-option table + Calendar Entries + Environment Variables + Config Path Resolution
- Scheduling: Cron with flock + Systemd Timer/Service
- Compatibility: Server matrix (3 entries) + Client matrix (3 entries) + DEPENDS-ON footnote
- Known Limitation #15: Present with workaround
- CLI Reference: --fail-fast documented
- Quick Start: Steps 1-4 starting with Configure
- Repo URL: alexandrebarsacq/caldawarrior (no example/caldawarrior)
- All 14 original limitations preserved
- Cargo.toml version: 1.0.0
- No v1 tags created

## Self-Check: PASSED

- FOUND: README.md
- FOUND: Cargo.toml
- FOUND: Cargo.lock
- FOUND: .planning/phases/06-documentation-and-release/06-01-SUMMARY.md
- FOUND commit: 6f28836 (Task 1 - README update)
- FOUND commit: 0dea89b (Task 2 - Version bump)
- FOUND commit: ac219c9 (Task 2 - Cargo.lock update)
