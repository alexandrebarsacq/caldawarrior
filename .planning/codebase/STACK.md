# Technology Stack

**Analysis Date:** 2026-03-18

## Languages

**Primary:**
- Rust 2024 edition - Core binary `caldawarrior` that syncs TaskWarrior ↔ CalDAV

**Secondary:**
- Python 3 - Robot Framework blackbox test libraries in `tests/robot/resources/`
- Shell/TOML - Configuration and build scripts

## Runtime

**Environment:**
- Rust toolchain (cargo)

**Package Manager:**
- Cargo (Rust package manager)
- Lockfile: `Cargo.lock` present (tracked in git)

## Frameworks

**Core:**
- reqwest 0.12 - HTTP client with rustls-tls for CalDAV HTTP requests
- serde + serde_json 1 - JSON serialization/deserialization for TaskWarrior JSON export/import
- chrono 0.4 + chrono-tz 0.10 - DateTime handling with timezone support for RFC 5545 iCalendar

**Testing:**
- Robot Framework 6+ - Blackbox end-to-end behavioral tests (Docker-based)
- Rust built-in #[test] macros - Unit and integration tests in `tests/integration/`

**Build/Dev:**
- clap 4 - CLI argument parsing (derive-based)
- thiserror 2 - Error type derivation
- anyhow 1 - Error context chains
- uuid 1 - UUID generation for CalDAV task IDs (v4, serde-enabled)
- toml 0.8 - TOML config file parsing

## Key Dependencies

**Critical:**
- reqwest 0.12 with `blocking` and `rustls-tls` features - CalDAV HTTP communication with strict TLS by default
- chrono 0.4 with `serde` - DateTime serialization matches TaskWarrior's compact format (YYYYMMDDTHHMMSSZ)
- chrono-tz 0.10 - TZID timezone parameter parsing in RFC 5545 timestamps

**Infrastructure:**
- tempfile 3 (dev) - Temporary test files and directories
- Docker Compose v2 - Containerized Radicale (CalDAV server) for integration tests
- Docker - TaskWarrior container image for Robot Framework tests

**CLI & Error Handling:**
- clap 4 - Parses `caldawarrior [--config PATH] <sync|help>`
- thiserror 2 - Error enum derives `Display` via `#[error(...)]`
- anyhow 1 - Error context via `.context()` and `.with_context()`

## Configuration

**Environment:**
- Config file: `~/.config/caldawarrior/config.toml` (default) or `CALDAWARRIOR_CONFIG` env var
- Password override: `CALDAWARRIOR_PASSWORD` env var (highest precedence, used in CI/scripting)
- Home directory: resolved via `HOME` env var
- Format: TOML (parsed by `toml` crate)

**Key Configs Required:**
- `server_url` - CalDAV server base URL (e.g., https://dav.example.com)
- `username` - HTTP Basic Auth username
- `password` - HTTP Basic Auth password (or env override)
- `[[calendar]]` entries - Project-to-calendar mappings (name, URL)
- `completed_cutoff_days` - How far back to sync completed tasks (default: 90)
- `allow_insecure_tls` - Bypass TLS verification for self-signed certs (default: false)
- `caldav_timeout_seconds` - HTTP timeout (default: 30)

**Build:**
- `Cargo.toml` - Manifest with bin target `caldawarrior` pointing to `src/main.rs`
- `Cargo.lock` - Locked dependency versions for reproducible builds
- Rust edition: 2024

## Platform Requirements

**Development:**
- Rust 1.70+ (for edition 2024 support)
- Cargo
- Docker & Docker Compose v2 (for running integration tests)
- TaskWarrior 3.x installed locally (for CLI adapter; alternatively runs in Docker)

**Production:**
- Linux/macOS/Windows with Rust runtime (statically linked via rustls)
- CalDAV-compatible server (tested with Radicale 3.3, Nextcloud, FastMail, iCloud, Baikal)
- TaskWarrior 3.x installation (binary `task` on PATH)

---

*Stack analysis: 2026-03-18*
