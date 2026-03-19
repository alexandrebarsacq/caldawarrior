# Phase 6: Documentation and Release - Context

**Gathered:** 2026-03-19
**Status:** Ready for planning

<domain>
## Phase Boundary

README updates (install, config reference, scheduling), CHANGELOG creation, compatibility matrix, version bump to 1.0.0, and release preparation. A new user can install, configure, and use caldawarrior from the README alone, with known limitations clearly documented.

</domain>

<decisions>
## Implementation Decisions

### README updates
- Lead install section with pre-built binary download from GitHub Releases (simplest path), then cargo install, then build-from-source as alternatives
- Add inline Config Reference section in README with a table of all config.toml options (type, default, description, example) — includes `completed_cutoff_days`, `allow_insecure_tls`, `caldav_timeout_seconds`, env vars `CALDAWARRIOR_PASSWORD` and `CALDAWARRIOR_CONFIG`
- Add dedicated Scheduling section with cron example (with flock) and systemd timer/service unit examples
- Keep all 14 known limitations inline in README — important for visibility before adoption

### CHANGELOG format
- Keep-a-changelog format (keepachangelog.com): grouped by Added/Changed/Fixed/Removed under version headers
- Hand-curated from git history — read commits, write human-friendly entries that summarize what users care about (not a raw commit dump)
- Version header: `## [1.0.0] - 2026-03-XX` (date filled at release time)

### Compatibility matrix
- In README, as a dedicated Compatibility section
- Three tiers: Tested (E2E verified), Expected (should work per spec compliance, not E2E tested), Unknown/Untested
- Servers: Radicale (Tested), Nextcloud (Expected), Baikal (Expected)
- Clients: tasks.org + DAVx5 (Tested for basic sync; DEPENDS-ON tested but invisible), Thunderbird (Expected)
- DEPENDS-ON invisibility: footnote in compatibility matrix noting RELATED-TO;RELTYPE=DEPENDS-ON is preserved through sync but not displayed by any tested client, plus a new known limitation entry (#15)

### Release preparation
- Bump Cargo.toml version from 0.1.0 to 1.0.0
- Phase prepares everything (docs, version bump, changelog) but does NOT create the v1.0.0 tag — user tags manually when ready
- CI green is sufficient pre-release validation — no additional manual smoke test required

### Claude's Discretion
- Exact README section ordering and headings
- CHANGELOG entry granularity (how many commits to group per entry)
- Systemd unit file details (timer interval, service options)
- Compatibility matrix table layout

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` — DOC-01 (README), DOC-02 (config reference), DOC-03 (CHANGELOG), DOC-04 (compatibility matrix)
- `.planning/ROADMAP.md` — Phase 6 success criteria (4 conditions that must be TRUE)

### Existing documentation
- `README.md` — Current README with Quick Start, Field Mapping, CLI Reference, 14 Known Limitations, Testing, v2 Roadmap. Must be updated, not rewritten.
- `src/config.rs` — Config struct definition with all options, types, defaults, and validation logic. Source of truth for DOC-02.

### Build and release infrastructure
- `.github/workflows/release.yml` — Release workflow triggered on `v*` tags. Produces `caldawarrior-v{version}-x86_64-linux` binary + SHA256 checksum.
- `.github/workflows/ci.yml` — CI pipeline (fmt, clippy, tests, E2E, cargo-deny)
- `Cargo.toml` — Package version (currently 0.1.0, must bump to 1.0.0)

### Prior phase context
- `.planning/phases/05-ci-cd-and-packaging/05-CONTEXT.md` — Release workflow decisions: tag pattern `v*`, binary naming convention, standalone binary (no tar.gz)
- `.planning/phases/02-relation-verification/02-CONTEXT.md` — DEPENDS-ON client visibility findings (invisible but preserved)

### Git history
- `git log --oneline` — Source for hand-curated CHANGELOG entries covering the hardening milestone

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `README.md` — Comprehensive existing README (~300 lines). Quick Start, Field Mapping table, CLI Reference, 14 Known Limitations with workarounds, Testing section, v2 Roadmap. Update in-place, don't rewrite.
- `src/config.rs:11-28` — Config struct with 6 fields: `server_url`, `username`, `password`, `completed_cutoff_days` (u32, default 90), `allow_insecure_tls` (bool, default false), `caldav_timeout_seconds` (u64, default 30). Plus `calendars: Vec<CalendarEntry>`.
- `src/config.rs:47-74` — Config loading logic: path resolution order (--config flag > CALDAWARRIOR_CONFIG env > ~/.config/caldawarrior/config.toml), CALDAWARRIOR_PASSWORD override, permission check, validation.

### Established Patterns
- README uses GitHub-flavored Markdown with tables for field mapping and status mapping
- Known limitations use numbered sections with bold title, description paragraph, and `**Workaround:**` block
- Config example uses TOML code blocks with comments

### Integration Points
- `Cargo.toml:3` — Version field to bump from `0.1.0` to `1.0.0`
- `.github/workflows/release.yml` — Uses Cargo.toml version for binary naming; version bump must happen before tagging

</code_context>

<specifics>
## Specific Ideas

- Binary install should be the first/easiest option — the README currently doesn't mention GitHub Releases at all
- Config reference must be derived from actual code in `src/config.rs`, not invented — the struct is the source of truth
- CHANGELOG should read like "what changed for users" not "what commits were made"
- DEPENDS-ON limitation is a key differentiator caveat — caldawarrior supports dependency sync but no client currently displays it

</specifics>

<deferred>
## Deferred Ideas

- aarch64-linux and macOS binary releases — v2 (PKG-03)
- crates.io publishing — v2 (PKG-04)
- Docker production image — out of scope per PROJECT.md

</deferred>

---

*Phase: 06-documentation-and-release*
*Context gathered: 2026-03-19*
