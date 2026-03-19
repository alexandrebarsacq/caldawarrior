# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-03-XX

### Added

- Bidirectional sync between TaskWarrior 3.x and CalDAV VTODO calendars with last-write-wins conflict resolution
- Task dependency sync via `RELATED-TO;RELTYPE=DEPENDS-ON` (RFC 9253) -- unique among CalDAV sync tools
- Dry-run mode (`--dry-run`) to preview sync changes without writing
- `--fail-fast` flag to stop on first sync error instead of continuing
- Cyclic dependency detection with graceful handling -- cyclic tasks sync all fields except relations
- CalDAV CATEGORIES bidirectional mapping to TaskWarrior tags
- DATE-only DUE/DTSTART value preservation through sync round-trips
- Timezone-aware datetime handling with DST ambiguity resolution
- Non-standard iCalendar property preservation (X-APPLE-SORT-ORDER, X-OC-HIDESUBTASKS, etc.)
- Pre-built x86_64 Linux binary published to GitHub Releases on each tagged version
- CI pipeline with cargo fmt, clippy, unit tests, integration tests, Robot Framework E2E tests, and cargo-deny security audit

### Changed

- Cyclic dependency handling: cyclic tasks now sync all fields except RELATED-TO (previously entire task was skipped)
- Task update mechanism uses `task modify` with tag/annotation diff instead of `task import` (fixes caldavuid UDA loss in Docker TW3)

### Fixed

- CATEGORIES comma-escaping: tags containing commas no longer silently split into separate tags
- XML parser: replaced with namespace-aware quick-xml parser, fixing compatibility with non-Radicale CalDAV servers (Nextcloud, Baikal)
- ETag normalization: weak ETags from Nextcloud/Baikal handled correctly, preventing 412 Precondition Failed loops
- Error context: sync errors now include task UUID, field name, and server URL instead of swallowed defaults
- CANCELLED status propagation: fixed asymmetry in CalDAV-to-TaskWarrior deletion handling
- DST fall-back ambiguity resolved to standard-time interpretation; spring-forward gap falls back to UTC
