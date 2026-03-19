# Phase 6: Documentation and Release - Research

**Researched:** 2026-03-19
**Domain:** Documentation (README, CHANGELOG, compatibility matrix), release preparation (version bump)
**Confidence:** HIGH

## Summary

Phase 6 is a documentation-only phase with no code logic changes. The work consists of four deliverables: (1) updating the existing README.md with binary install instructions, a config reference table, and scheduling examples; (2) creating a CHANGELOG.md in Keep a Changelog format from curated git history; (3) adding a compatibility matrix section to the README; and (4) bumping the Cargo.toml version from 0.1.0 to 1.0.0.

All source material already exists in the codebase. The Config struct in `src/config.rs` is the definitive reference for DOC-02. The git log (90 commits spanning 6 phases of hardening work) provides CHANGELOG material. Compatibility evidence comes from `docs/compatibility/tasks-org.md` and the `09_compatibility.robot` E2E test suite. The release workflow (`.github/workflows/release.yml`) defines the binary naming convention for download links.

**Primary recommendation:** Treat this as 2 plans -- (1) README updates + version bump, (2) CHANGELOG creation. All work is Markdown/TOML editing with no compilation or runtime testing needed. The only verification is that the README is accurate against the actual code and that the version bump does not break `cargo build`.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Lead install section with pre-built binary download from GitHub Releases (simplest path), then cargo install, then build-from-source as alternatives
- Add inline Config Reference section in README with a table of all config.toml options (type, default, description, example) -- includes `completed_cutoff_days`, `allow_insecure_tls`, `caldav_timeout_seconds`, env vars `CALDAWARRIOR_PASSWORD` and `CALDAWARRIOR_CONFIG`
- Add dedicated Scheduling section with cron example (with flock) and systemd timer/service unit examples
- Keep all 14 known limitations inline in README -- important for visibility before adoption
- Keep-a-changelog format (keepachangelog.com): grouped by Added/Changed/Fixed/Removed under version headers
- Hand-curated from git history -- read commits, write human-friendly entries that summarize what users care about (not a raw commit dump)
- Version header: `## [1.0.0] - 2026-03-XX` (date filled at release time)
- Compatibility matrix in README as a dedicated Compatibility section with three tiers: Tested, Expected, Unknown/Untested
- Servers: Radicale (Tested), Nextcloud (Expected), Baikal (Expected)
- Clients: tasks.org + DAVx5 (Tested for basic sync; DEPENDS-ON tested but invisible), Thunderbird (Expected)
- DEPENDS-ON invisibility: footnote in compatibility matrix noting RELATED-TO;RELTYPE=DEPENDS-ON is preserved through sync but not displayed by any tested client, plus a new known limitation entry (#15)
- Bump Cargo.toml version from 0.1.0 to 1.0.0
- Phase prepares everything (docs, version bump, changelog) but does NOT create the v1.0.0 tag -- user tags manually when ready
- CI green is sufficient pre-release validation -- no additional manual smoke test required

### Claude's Discretion
- Exact README section ordering and headings
- CHANGELOG entry granularity (how many commits to group per entry)
- Systemd unit file details (timer interval, service options)
- Compatibility matrix table layout

### Deferred Ideas (OUT OF SCOPE)
- aarch64-linux and macOS binary releases -- v2 (PKG-03)
- crates.io publishing -- v2 (PKG-04)
- Docker production image -- out of scope per PROJECT.md
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| DOC-01 | README covers installation, configuration, usage, and common workflows | Existing README (361 lines) has Quick Start, CLI Reference, Features, Testing. Needs: binary install leading, Config Reference table, Scheduling section, limitation #15, compatibility section. Existing structure preserved, sections added/updated in-place. |
| DOC-02 | All config.toml options documented with examples and defaults | Config struct in `src/config.rs:11-28` has 6 fields + CalendarEntry. Config loading in `src/config.rs:47-74` defines path resolution, env var override, permission check. All types and defaults are compile-time visible. |
| DOC-03 | CHANGELOG generated from git history | 90 commits across 6 phases. ~28 feat/fix/test commits to curate. Keep a Changelog format: Added/Changed/Fixed/Removed sections under `## [1.0.0] - 2026-03-XX`. |
| DOC-04 | Client/server compatibility matrix documenting tested combinations and known limitations | Evidence from: `docs/compatibility/tasks-org.md` (DEPENDS-ON visibility matrix), `09_compatibility.robot` (5 E2E tests for DATE-only, X-property preservation), all other E2E suites (Radicale-tested). |
</phase_requirements>

## Standard Stack

This phase involves no new libraries or tools. All deliverables are Markdown files and a single TOML version field edit.

### Core
| Tool | Purpose | Why Standard |
|------|---------|--------------|
| GitHub-Flavored Markdown | README.md, CHANGELOG.md formatting | Repository standard, renders on GitHub |
| Keep a Changelog 1.1.0 | CHANGELOG format | Industry standard, human-readable |
| Semantic Versioning 2.0.0 | Version numbering (0.1.0 -> 1.0.0) | Rust/Cargo convention |

### No External Dependencies
No packages to install. No build tool changes.

## Architecture Patterns

### README Section Ordering (Recommended)

The existing README has this structure. The update should preserve existing sections and insert new ones:

```
README.md (after update)
├── Title + description (existing, keep)
├── Features (existing, keep)
├── Installation (NEW -- replaces current "Step 1" in Quick Start)
│   ├── Pre-built binary (GitHub Releases)
│   ├── cargo install
│   └── Build from source
├── Quick Start (existing, restructured)
│   ├── Step 1 -- Configure (existing Step 2)
│   ├── Step 2 -- Register UDA (existing Step 3)
│   ├── Step 3 -- Dry-run (existing Step 4)
│   └── Step 4 -- First sync (existing Step 5)
├── Config Reference (NEW -- DOC-02)
├── CLI Reference (existing, keep)
├── Scheduling (NEW -- cron + systemd)
├── Field Mapping (existing, keep)
│   └── Status Mapping (existing, keep)
├── Compatibility (NEW -- DOC-04)
├── v1 Known Limitations (existing, add #15)
├── Testing (existing, keep)
├── v2 Roadmap (existing, keep)
└── License (existing, keep)
```

### Pattern 1: Config Reference Table

**What:** A table listing every config.toml option with its type, default, description, and example value.
**When to use:** DOC-02 requirement.
**Source of truth:** `src/config.rs:11-28` (struct definition) and `src/config.rs:30-74` (defaults, loading, validation).

The Config struct defines these fields:
```
| Option | Type | Default | Required | Description |
|--------|------|---------|----------|-------------|
| server_url | String | (none) | Yes | Base URL of CalDAV server |
| username | String | (none) | Yes | CalDAV username |
| password | String | (none) | Yes | CalDAV password (overridden by CALDAWARRIOR_PASSWORD env) |
| completed_cutoff_days | u32 | 90 | No | Days of completed/deleted task history to sync |
| allow_insecure_tls | bool | false | No | Skip TLS certificate verification |
| caldav_timeout_seconds | u64 | 30 | No | HTTP request timeout for CalDAV operations |
| [[calendar]] | Array | [] | No | Calendar-to-project mappings |
| calendar.project | String | (none) | (per entry) | TW project name |
| calendar.url | String | (none) | (per entry) | CalDAV collection URL |
```

Plus environment variables:
```
| Variable | Purpose |
|----------|---------|
| CALDAWARRIOR_PASSWORD | Overrides password field in config |
| CALDAWARRIOR_CONFIG | Alternative config file path |
```

Config path resolution: `--config` flag > `CALDAWARRIOR_CONFIG` env > `~/.config/caldawarrior/config.toml`

### Pattern 2: Compatibility Matrix

**What:** Three-tier matrix (Tested/Expected/Unknown) for server and client combinations.
**Source:** `docs/compatibility/tasks-org.md` and E2E test suite (all tests run against Radicale).

Evidence inventory:
- **Radicale (server):** ALL E2E tests (73 scenarios, ~64 passing, 9 skipped) run against Radicale in Docker. TESTED tier.
- **Nextcloud (server):** XML parser handles Nextcloud namespace prefixes (Phase 1 quick-xml rewrite). ETag normalization handles weak ETags from Nextcloud (AUDIT-04). No E2E tests against Nextcloud server. EXPECTED tier.
- **Baikal (server):** Same XML parser improvements apply. No E2E tests. EXPECTED tier.
- **tasks.org + DAVx5 (client):** Basic VTODO sync works (documented). DEPENDS-ON invisible but preserved (documented in `docs/compatibility/tasks-org.md`). TESTED for basic sync with caveat.
- **Thunderbird (client):** Not tested. CalDAV VTODO support exists. EXPECTED tier.

### Pattern 3: Keep a Changelog Format

**What:** CHANGELOG.md following keepachangelog.com 1.1.0 format.
**Structure:**
```markdown
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-03-XX

### Added
- ...

### Changed
- ...

### Fixed
- ...
```

Six categories available: Added, Changed, Deprecated, Removed, Fixed, Security. Use only those that have entries.

### Pattern 4: Systemd Timer/Service Units

**What:** Example systemd units for automated scheduling.
**Recommended approach:**

Timer unit (`caldawarrior.timer`):
- 15-minute interval (reasonable default for task sync)
- `Persistent=true` (catch up after sleep/shutdown)

Service unit (`caldawarrior.service`):
- `Type=oneshot` (sync runs and exits)
- `ExecStart=/usr/local/bin/caldawarrior sync`
- No `RemainAfterExit` needed (oneshot without it prevents overlap naturally via systemd)

### Anti-Patterns to Avoid
- **Inventing config options that don't exist:** Every documented option MUST trace back to `src/config.rs`. Do not document planned or imagined features.
- **Raw commit dump as CHANGELOG:** Commits like "docs(01-01): complete plan" are internal. CHANGELOG entries should describe user-facing changes.
- **Overstating compatibility:** Only Radicale has E2E verification. Nextcloud/Baikal are "Expected" based on standards compliance, not testing.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Binary download URL construction | Hardcoded URLs | Pattern: `https://github.com/alexandrebarsacq/caldawarrior/releases/latest/download/caldawarrior-v{VERSION}-x86_64-linux` | Release workflow generates this naming automatically |
| CHANGELOG from git | Script parsing `git log --format` | Hand curation with Keep a Changelog format | User-facing entries require human judgment on what matters |

## Common Pitfalls

### Pitfall 1: Config Reference Drift
**What goes wrong:** Config documentation lists options that don't exist or misses new ones.
**Why it happens:** Documentation written from memory instead of code inspection.
**How to avoid:** Every config option in the table MUST have a corresponding field in `src/config.rs:11-28`. Cross-reference the struct definition when writing the table.
**Warning signs:** Config table has more rows than struct fields.

### Pitfall 2: Incorrect Default Values
**What goes wrong:** Documenting wrong defaults (e.g., "completed_cutoff_days defaults to 30" when it's actually 90).
**Why it happens:** Guessing instead of reading `default_completed_cutoff_days()` and `default_caldav_timeout_seconds()` functions.
**How to avoid:** Read lines 30-35 of `src/config.rs` for authoritative defaults.
**Warning signs:** User follows docs, gets unexpected behavior.

### Pitfall 3: Binary Download URL Mismatch
**What goes wrong:** README download instructions point to wrong filename pattern.
**Why it happens:** Release workflow binary naming (`caldawarrior-${VERSION}-x86_64-linux`) doesn't match what README says.
**How to avoid:** Read `.github/workflows/release.yml` lines 28-31 for the exact naming convention: `caldawarrior-v{TAG}-x86_64-linux`. Note the `v` prefix comes from `GITHUB_REF_NAME` which is the tag name (e.g., `v1.0.0`).
**Warning signs:** 404 on download link after release.

### Pitfall 4: Forgetting Known Limitation #15
**What goes wrong:** Compatibility matrix mentions DEPENDS-ON invisibility but the known limitations list stays at 14.
**Why it happens:** Treating compatibility matrix and limitations as separate concerns.
**How to avoid:** Add limitation #15 explicitly about DEPENDS-ON client invisibility. Reference it from the compatibility matrix footnote.
**Warning signs:** Compatibility matrix and limitations list contradict.

### Pitfall 5: Version Bump Breaking Build
**What goes wrong:** Changing Cargo.toml version causes unexpected issues.
**Why it happens:** Unlikely but worth verifying -- sometimes lockfile regeneration or CI caching uses version.
**How to avoid:** After bumping version, run `cargo build --release` to verify. The release workflow cache key (`release-musl`) does not include the version, so no issue expected.
**Warning signs:** CI failure after version bump commit.

### Pitfall 6: CHANGELOG Scope Creep
**What goes wrong:** Including pre-hardening history or internal planning commits in the CHANGELOG.
**Why it happens:** Running `git log` without filtering.
**How to avoid:** CHANGELOG covers the hardening milestone only (commits from `eb4818e` "docs: initialize project" onward, or more specifically the feat/fix commits). The initial implementation commits (`c4e7955` and earlier) are pre-v1 development, not changelog-worthy changes TO v1.
**Warning signs:** CHANGELOG mentions "Robot Framework test suite" or "Foundry spec" -- these are development infrastructure, not user-facing changes.

## Code Examples

### Example: Binary Install Section
```markdown
### Pre-built Binary (Recommended)

Download the latest release:

\`\`\`bash
# Download binary and checksum
curl -LO https://github.com/alexandrebarsacq/caldawarrior/releases/latest/download/caldawarrior-v1.0.0-x86_64-linux
curl -LO https://github.com/alexandrebarsacq/caldawarrior/releases/latest/download/caldawarrior-v1.0.0-x86_64-linux.sha256

# Verify checksum
sha256sum -c caldawarrior-v1.0.0-x86_64-linux.sha256

# Install
chmod +x caldawarrior-v1.0.0-x86_64-linux
sudo mv caldawarrior-v1.0.0-x86_64-linux /usr/local/bin/caldawarrior
\`\`\`
```

**Note:** Since the user tags manually after phase completion, the README should use the pattern `caldawarrior-v1.0.0-x86_64-linux` as the example but acknowledge that users should check the releases page for the actual latest version.

### Example: Config Reference Table
```markdown
## Config Reference

### Options

| Option | Type | Default | Required | Description |
|--------|------|---------|----------|-------------|
| `server_url` | string | -- | Yes | Base URL of the CalDAV server |
| `username` | string | -- | Yes | CalDAV username for authentication |
| `password` | string | -- | Yes | CalDAV password (see Environment Variables below) |
| `completed_cutoff_days` | integer | `90` | No | Number of days of completed/deleted task history to include in sync |
| `allow_insecure_tls` | boolean | `false` | No | Skip TLS certificate verification (for self-signed certificates) |
| `caldav_timeout_seconds` | integer | `30` | No | HTTP request timeout in seconds for CalDAV operations |

### Calendar Entries

Each `[[calendar]]` section maps a TaskWarrior project to a CalDAV collection:

| Option | Type | Required | Description |
|--------|------|----------|-------------|
| `project` | string | Yes | TaskWarrior project name |
| `url` | string | Yes | Full URL of the CalDAV calendar collection |

### Environment Variables

| Variable | Description |
|----------|-------------|
| `CALDAWARRIOR_PASSWORD` | Overrides the `password` field in config.toml |
| `CALDAWARRIOR_CONFIG` | Path to config file (overrides default `~/.config/caldawarrior/config.toml`) |

### Config Path Resolution

1. `--config` CLI flag (highest priority)
2. `CALDAWARRIOR_CONFIG` environment variable
3. `~/.config/caldawarrior/config.toml` (default)
```

### Example: Cron with flock
```markdown
## Scheduling

### Cron

\`\`\`bash
# Sync every 15 minutes with flock to prevent overlapping runs
*/15 * * * * /usr/bin/flock -n /tmp/caldawarrior.lock /usr/local/bin/caldawarrior sync >> /var/log/caldawarrior.log 2>&1
\`\`\`
```

### Example: Systemd Timer
```markdown
### Systemd Timer

Create two files:

**`~/.config/systemd/user/caldawarrior.service`**
\`\`\`ini
[Unit]
Description=Sync TaskWarrior with CalDAV

[Service]
Type=oneshot
ExecStart=/usr/local/bin/caldawarrior sync
\`\`\`

**`~/.config/systemd/user/caldawarrior.timer`**
\`\`\`ini
[Unit]
Description=Run caldawarrior sync periodically

[Timer]
OnBootSec=1min
OnUnitActiveSec=15min
Persistent=true

[Install]
WantedBy=timers.target
\`\`\`

Enable and start:
\`\`\`bash
systemctl --user enable --now caldawarrior.timer
\`\`\`
```

### Example: Compatibility Matrix
```markdown
## Compatibility

### Servers

| Server | Tier | Notes |
|--------|------|-------|
| Radicale | Tested | Full E2E test suite runs against Radicale. All features verified. |
| Nextcloud | Expected | XML parser handles Nextcloud namespaces. Weak ETag normalization implemented. Not E2E tested. |
| Baikal | Expected | Standard CalDAV compliance. Not E2E tested. |

### Clients

| Client | Tier | Notes |
|--------|------|-------|
| TaskWarrior 3.x | Tested | Primary sync target. Full bidirectional sync verified. |
| tasks.org + DAVx5 | Tested* | Basic VTODO sync works. DEPENDS-ON relations preserved but invisible (see note below). |
| Thunderbird | Expected | CalDAV VTODO support available. Not tested with caldawarrior. |

*DEPENDS-ON note:* caldawarrior syncs task dependencies using `RELATED-TO;RELTYPE=DEPENDS-ON` ([RFC 9253](https://datatracker.ietf.org/doc/html/rfc9253)). This property is preserved through sync by Radicale and DAVx5, but no tested CalDAV client currently renders DEPENDS-ON relationships in its UI. Dependencies work fully between TaskWarrior instances syncing through a CalDAV server. See Known Limitation #15.
```

### Example: Known Limitation #15
```markdown
### 15. DEPENDS-ON relations invisible to CalDAV clients

Task dependencies synced via `RELATED-TO;RELTYPE=DEPENDS-ON` (RFC 9253) are preserved on the
CalDAV server but not displayed by any tested client (tasks.org, Thunderbird). Dependencies
work correctly between TaskWarrior instances syncing through the same CalDAV server.

**Workaround:** Use TaskWarrior directly to view and manage task dependencies. CalDAV clients
will show tasks individually without dependency relationships.
```

## CHANGELOG Curation Guide

### Commit-to-Entry Mapping

The git history contains 90 commits. For CHANGELOG purposes, group by user-facing impact:

**Added (new capabilities):**
- `--fail-fast` CLI flag (commit `4b377e4`)
- E2E test suite with Robot Framework (not user-facing, omit or mention briefly)
- CI pipeline and release workflow (mention as release infrastructure)
- Pre-built binary releases on GitHub Releases

**Changed (modified behavior):**
- Cyclic dependency handling: cyclic tasks now sync all fields except RELATED-TO (was: entire task skipped) (commit `cbf7066`)
- CalDAV CATEGORIES now mapped bidirectionally to TW tags (was: TW-to-CalDAV only) (commit `23f04d8` area)
- Task update mechanism: `task modify` with tag/annotation diff (was: `task import` which dropped caldavuid) (commit `2d956a4`)

**Fixed (bug fixes):**
- CATEGORIES comma-escaping: tags with commas no longer silently split (commits `9ca8a59`, `a1284fb`)
- XML parser: replaced with namespace-aware quick-xml parser for non-Radicale server compatibility (commit `9a4d0e6`)
- ETag normalization: weak ETags handled correctly, preventing 412 Precondition Failed loops (commit `9a4d0e6`)
- Error context: replaced `unwrap_or_default()` calls with error-preserving alternatives (commit `4a8b73a`)
- CANCELLED propagation: fixed asymmetry in CalDAV-to-TW deletion handling (commit `397af45`)
- DATE-only DUE/DTSTART values preserved through sync round-trips (commits `ca5baf4`, `d510947`)
- DST ambiguity: fall-back resolved to standard time, spring-forward gap falls back to UTC (commit `ca5baf4`)

**Granularity recommendation:** Group related commits into single entries. For example, all Phase 1 XML/ETag work becomes one "Fixed" entry about CalDAV server compatibility. All Phase 3 field mapping fixes become one entry about bidirectional sync completeness. Aim for 10-15 total entries, not 28.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `cargo install --path .` only install option | Pre-built binary from GitHub Releases | Phase 5 (release.yml) | Binary download is now the primary install path |
| No config documentation | Inline config reference in README | This phase | Users can discover all options without reading source |
| No CHANGELOG | Keep a Changelog format | This phase | Users understand what changed between versions |
| Version 0.1.0 | Version 1.0.0 | This phase | Signals production readiness |

## Open Questions

1. **Exact release date for CHANGELOG header**
   - What we know: User will tag manually when ready. CHANGELOG header should be `## [1.0.0] - 2026-03-XX`.
   - What's unclear: The exact date. Context says "date filled at release time."
   - Recommendation: Use `2026-03-XX` as placeholder in the CHANGELOG. User fills the date when tagging.

2. **GitHub repository URL for install instructions**
   - What we know: Remote is `git@github.com:alexandrebarsacq/caldawarrior.git`. Public visibility not confirmed.
   - What's unclear: Whether the repo will be public at release time.
   - Recommendation: Use `https://github.com/alexandrebarsacq/caldawarrior` in all documentation links. This is the canonical form regardless of SSH remote.

3. **Pre-hardening commit inclusion in CHANGELOG**
   - What we know: Context says "covering the hardening milestone." The initial implementation predates the hardening project.
   - What's unclear: Whether the initial implementation features (basic sync, LWW, dry-run) should appear as "Added" in 1.0.0.
   - Recommendation: Include core features as "Added" entries in the 1.0.0 CHANGELOG since this is the first release. Users have no prior version to compare against, so listing what exists is more useful than only listing what changed during hardening.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust unit/integration) + Robot Framework (E2E) |
| Config file | `Cargo.toml` (Rust tests), `tests/robot/docker-compose.yml` (RF) |
| Quick run command | `cargo test --lib` |
| Full suite command | `cargo test --lib && cargo test --test integration && make test-robot` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DOC-01 | README covers installation, config, usage, scheduling | manual-only | Visual review of README.md | N/A (documentation) |
| DOC-02 | All config options documented with types/defaults | manual-only | Cross-reference `src/config.rs` struct fields against README table | N/A (documentation) |
| DOC-03 | CHANGELOG in Keep a Changelog format | manual-only | Visual review of CHANGELOG.md structure | N/A (documentation) |
| DOC-04 | Compatibility matrix with tested/expected tiers | manual-only | Visual review of compatibility section | N/A (documentation) |

**Manual-only justification:** All four requirements are documentation deliverables. There is no runtime behavior to test. Verification is structural review: does the README contain the required sections, does the config table match the code, does the CHANGELOG follow the format, does the compatibility matrix have the required tiers.

**Build verification:** After version bump, `cargo build --release` must succeed. This is the only automated check for this phase.

### Sampling Rate
- **Per task commit:** `cargo build --release` (verifies version bump didn't break anything)
- **Per wave merge:** `cargo test --lib` (ensures no regressions)
- **Phase gate:** `cargo build --release` green + documentation review

### Wave 0 Gaps
None -- this phase produces documentation files and a version field edit. No test infrastructure is needed.

## Sources

### Primary (HIGH confidence)
- `src/config.rs` -- Config struct definition with all 6 fields, defaults, loading logic, validation
- `Cargo.toml` -- Current version (0.1.0), package metadata
- `.github/workflows/release.yml` -- Binary naming convention (`caldawarrior-${GITHUB_REF_NAME}-x86_64-linux`)
- `README.md` -- Existing 361-line README with established patterns (field mapping tables, limitation format, code blocks)
- `docs/compatibility/tasks-org.md` -- DEPENDS-ON visibility matrix with confidence levels
- `tests/robot/suites/09_compatibility.robot` -- 5 E2E compatibility tests (DATE-only, X-property)
- `git log --oneline` -- 90 commits, 28 feat/fix/test commits for CHANGELOG curation
- keepachangelog.com/en/1.1.0/ -- Format specification (6 change types, version header format)

### Secondary (MEDIUM confidence)
- `docs/compatibility/tasks-org.md` -- tasks.org/DAVx5 DEPENDS-ON behavior (based on documentation analysis, not device testing)
- Nextcloud/Baikal compatibility tier -- Based on XML parser improvements and standards compliance, not E2E testing

### Tertiary (LOW confidence)
- Thunderbird compatibility -- Listed as "Expected" based on CalDAV VTODO support existence, no testing or documentation reviewed

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- No libraries involved, just Markdown and TOML editing
- Architecture: HIGH -- All source material exists in codebase, patterns established by existing README
- Pitfalls: HIGH -- Concrete pitfalls identified from actual code inspection (config defaults, binary naming, limitation numbering)
- CHANGELOG curation: MEDIUM -- Judgment required on which commits map to user-facing entries and what granularity to use

**Research date:** 2026-03-19
**Valid until:** No expiration -- documentation of existing stable codebase
