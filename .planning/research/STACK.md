# Technology Stack: Hardening & Ship-Readiness

**Project:** caldawarrior
**Researched:** 2026-03-18
**Mode:** Ecosystem research for hardening milestone
**Scope:** Testing, auditing, packaging, CI/CD, documentation -- NOT core sync stack (already decided)

## Context

The core stack is fixed: Rust 2024 edition, reqwest 0.12 (blocking + rustls-tls), chrono 0.4, serde/serde_json 1, clap 4, anyhow 1, thiserror 2. The hand-rolled iCal parser in `src/ical.rs` handles VTODO serialization/deserialization directly without third-party crate dependencies. This research covers tools and libraries needed to harden, test, audit, package, and ship.

---

## 1. VTODO/iCalendar Validation

### Recommendation: Python `icalendar` library (already in test stack) + RFC 5545 property-level assertions in Robot Framework

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| Python `icalendar` | 5.0.12 (pinned in test Dockerfile) | Parse and validate VTODO output in E2E tests | Already in the test stack. Proven RFC 5545 parser. Robot Framework tests already import it for VTODO inspection. Adding validation keywords is incremental, not a new dependency. | HIGH |
| iCalendar.org online validator | N/A (web tool) | Manual spot-check during development | Free, RFC 5545-compliant, runs client-side. Use for ad-hoc checks, not CI. | HIGH |

### What NOT to use

| Alternative | Why Not |
|-------------|---------|
| Rust `icalendar` crate (0.17.6) | Adds a runtime dependency for something only needed in tests. caldawarrior's hand-rolled parser is simpler and purpose-built. The Rust crate is a builder/parser, not a validator -- it won't catch RFC violations any better than our own parser. |
| Rust `calcard` crate (0.3.2) | Stalwart Labs' new crate. Promising (Postel's law approach, JSCalendar conversion), but v0.3.x signals immaturity. Only ~25K downloads. Overkill for VTODO-only validation. |
| CalDAVTester (CalConnect) | Python XML-scripted test framework for CalDAV server compliance. Tests the *server*, not the *client*. We're validating our output, not Radicale's protocol handling. Wrong tool for the job. |
| `caldav-tester` (Debian package) | Same as CalDAVTester, Debian-packaged. Still server-oriented. |

### Validation Strategy

The right approach is NOT adding an iCalendar validation library to the Rust binary. Instead:

1. **Property-level assertions in Robot Framework tests** -- after each sync, parse the VTODO output with Python `icalendar` and assert required properties (UID, DTSTAMP, VERSION:2.0), correct escaping, line folding <= 75 octets, CRLF line endings.
2. **Rust unit tests for `ical.rs`** -- already has 14 tests covering round-trip, escaping, folding, TZID, RELATED-TO. Add tests for edge cases: empty categories, multi-valued params, Unicode in SUMMARY/DESCRIPTION, DATE-only values.
3. **Spot-check with icalendar.org/validator** during development -- paste generated VTODO output, verify no RFC violations flagged.

**Source:** [icalendar.org validator](https://icalendar.org/validator.html), [icalendar.dev validator](https://icalendar.dev/validator/), [Python icalendar docs](https://icalendar.readthedocs.io/en/stable/)

---

## 2. CalDAV Compliance Testing

### Recommendation: Extend existing Robot Framework E2E suite (no new tools needed)

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| Robot Framework | 7.0.1 (pinned) | E2E behavioral tests against real Radicale | Already the test framework. CalDAV compliance for a *client* means "does the output work with real servers?" -- and that's exactly what the RF suite tests. | HIGH |
| Radicale Docker image | 3.3.0.0 (pinned) | CalDAV server for E2E | Already running in docker-compose.yml. Real server validates our REPORT/PUT/DELETE requests. | HIGH |

### What NOT to use

| Alternative | Why Not |
|-------------|---------|
| CalDAVTester | Designed for testing CalDAV *servers*, not clients. It sends HTTP requests to a server and validates responses. We need the inverse -- validate that our client sends correct requests and produces valid VTODO. |
| Second CalDAV server (Nextcloud, Baikal) | Nice-to-have for compatibility matrix, but premature. Focus on Radicale first. Add a second server only if tasks.org users report issues with specific servers. |

### Compliance Strategy

1. **Extend RF tests** to cover all CalDAV operations: PUT with `If-Match` (conditional), `REPORT` with calendar-multiget, `DELETE`, `MKCALENDAR`.
2. **Add RF tests for VTODO property compliance** -- verify tasks.org-required fields (SUMMARY, UID, DTSTAMP, STATUS) are always emitted.
3. **Test RELATED-TO round-trip** -- create tasks with dependencies via TW, sync to CalDAV, verify RELATED-TO;RELTYPE=DEPENDS-ON appears in Radicale's stored VTODO.
4. **Test with tasks.org's VTODO expectations** -- tasks.org requires specific fields and behaviors. Document and test against those expectations.

**Source:** [CalConnect CalDAVTester](https://github.com/CalConnect/caldavtester), [RFC 4791](https://www.ietf.org/rfc/rfc4791.txt), [RFC 5545 VTODO](https://icalendar.org/iCalendar-RFC-5545/3-6-2-to-do-component.html)

---

## 3. Docker Packaging for Production

### Recommendation: Multi-stage build with `rust:1.85-bookworm` builder + `archlinux:base` runner

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| `rust:1.85-bookworm` | 1.85 | Builder stage | Already used in test Dockerfile. Rust 1.85 is minimum for edition 2024. Bookworm provides a stable build environment. | HIGH |
| `archlinux:base` | rolling | Runner stage | **Critical constraint**: caldawarrior shells out to `task` (TaskWarrior 3.x). Debian/Ubuntu repos only ship TW 2.6.x. Arch packages TW 3.x. This is the same approach used in the test Dockerfile and it works. | HIGH |
| Docker BuildKit cache mounts | N/A | Build speedup | `--mount=type=cache` for cargo registry and target dir. Already used in test Dockerfile. | HIGH |

### What NOT to use

| Alternative | Why Not |
|-------------|---------|
| `FROM scratch` | Cannot run -- caldawarrior requires `task` binary at runtime. No shell, no package manager, no way to install TaskWarrior. |
| `gcr.io/distroless/static` | Same problem -- no TaskWarrior, no shell for `Command::new("task")`. |
| Alpine + musl | Alpine ships TW 2.6.x (if at all). musl static linking won't help because we need the `task` binary in the image, not just our binary. |
| Debian-based runner | Debian repos have TW 2.6.x only. Would need to compile TW 3.x from source in Docker, adding build complexity and fragility. |

### Production Dockerfile Strategy

The production Dockerfile should be a slimmed-down version of the existing test Dockerfile:

```dockerfile
# Stage 1: Build caldawarrior
FROM rust:1.85-bookworm AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    cargo build --release && \
    cp /build/target/release/caldawarrior /build/caldawarrior

# Stage 2: Runtime with TaskWarrior 3.x
FROM archlinux:base
RUN pacman -Syu --noconfirm && \
    pacman -S --noconfirm task ca-certificates && \
    pacman -Scc --noconfirm
COPY --from=builder /build/caldawarrior /usr/local/bin/caldawarrior
ENTRYPOINT ["caldawarrior"]
```

Key differences from test Dockerfile: no Python, no Robot Framework, no pip packages. Just the binary + TaskWarrior + CA certs for HTTPS.

**Estimated image size:** ~400-500 MB (archlinux:base is ~400 MB). Not minimal, but necessary due to the TaskWarrior runtime dependency. Trade-off is acceptable -- users self-hosting CalDAV sync don't care about image size the way microservice deployments do.

### Binary Release (non-Docker)

For users who have TaskWarrior already installed, provide standalone binaries:

| Technology | Purpose | Why | Confidence |
|------------|---------|-----|------------|
| `cargo build --release` | Standard release builds | Produces dynamically-linked binary. Users on Arch/Fedora (TW 3.x available) can use directly. | HIGH |
| `cross` (cross-rs) | Cross-compilation | For building x86_64 and aarch64 Linux binaries in CI. Not needed for musl -- dynamic linking to glibc is fine since the user's system already has TW installed. | MEDIUM |
| GitHub Releases | Binary distribution | Upload release binaries as GitHub release assets. | HIGH |

**Source:** [muslrust](https://github.com/clux/muslrust), [Distroless](https://github.com/GoogleContainerTools/distroless), [Docker multi-stage builds for Rust](https://oneuptime.com/blog/post/2026-01-07-rust-minimal-docker-images/view)

---

## 4. CI/CD Pipeline

### Recommendation: GitHub Actions with Swatinem/rust-cache + Docker Compose for E2E

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| GitHub Actions | N/A (platform) | CI/CD platform | Industry standard for open-source Rust projects. Free for public repos. | HIGH |
| `Swatinem/rust-cache` | v2 | Cargo dependency caching | Smart cache key generation for Rust projects. Caches `~/.cargo` and `target/`. Standard in Rust CI. | HIGH |
| `docker compose` (v2) | Bundled with GHA runners | E2E test orchestration | Already used locally for Robot Framework tests. GHA runners have Docker Compose v2 pre-installed. | HIGH |
| `actions/checkout@v4` | v4 | Code checkout | Standard. | HIGH |
| `dtolnay/rust-toolchain@stable` | Latest | Rust toolchain setup | De facto standard for GHA Rust setup. Lighter than `actions-rs/toolchain` (deprecated). | HIGH |

### CI Workflow Structure

```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --check
      - run: cargo clippy -- -D warnings
      - run: cargo test

  e2e:
    runs-on: ubuntu-latest
    needs: check
    steps:
      - uses: actions/checkout@v4
      - run: CURRENT_UID=$(id -u) CURRENT_GID=$(id -g) docker compose -f tests/robot/docker-compose.yml run --rm robot
```

### What NOT to use

| Alternative | Why Not |
|-------------|---------|
| `actions-rs/*` | Deprecated/unmaintained. `dtolnay/rust-toolchain` is the replacement. |
| `sccache` | Overkill for a single-crate project. Swatinem/rust-cache handles dependency caching. sccache shines in large workspaces. |
| Self-hosted runners | Unnecessary complexity for a small project. GHA free tier is sufficient. |

### Linting & Formatting

| Tool | Configuration | Why | Confidence |
|------|--------------|-----|------------|
| `cargo fmt --check` | Default rustfmt | Enforce consistent formatting. No config needed. | HIGH |
| `cargo clippy -- -D warnings` | Default + deny warnings | Catch common mistakes. `-D warnings` fails CI on any warning. | HIGH |
| `clippy::pedantic` | Do NOT enable in CI | Too many false positives. Aggressive lints cause noise and `#[allow]` clutter. Use default clippy lint level. | HIGH |

**Source:** [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache), [dtolnay/rust-toolchain](https://github.com/dtolnay/rust-toolchain), [Clippy docs](https://doc.rust-lang.org/stable/clippy/lints.html)

---

## 5. Security Auditing

### Recommendation: cargo-deny (comprehensive) + cargo-audit (advisory-focused)

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| `cargo-deny` | 0.19.0 | License compliance, advisory scanning, duplicate detection, source auditing | All-in-one supply chain tool. Checks licenses (important for distribution), security advisories, banned crates, and duplicate versions. Covers everything cargo-audit does plus more. | HIGH |
| `cargo-audit` | 0.22.1 | RustSec advisory scanning | Lighter alternative if you only want vulnerability scanning. But cargo-deny subsumes it. Include in CI as a belt-and-suspenders check. | HIGH |

### Configuration

```toml
# deny.toml (at project root)
[advisories]
vulnerability = "deny"
unmaintained = "warn"

[licenses]
allow = ["MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Unicode-3.0"]
confidence-threshold = 0.8

[bans]
multiple-versions = "warn"
wildcards = "deny"

[sources]
unknown-registry = "deny"
unknown-git = "deny"
```

### CI Integration

```yaml
- run: cargo install cargo-deny --locked
- run: cargo deny check
```

### What NOT to use

| Alternative | Why Not |
|-------------|---------|
| `cargo-semver-checks` | Useful for library crates published to crates.io. caldawarrior is a binary -- no public API to check for semver violations. |
| `cargo-auditable` | Embeds dependency info in binary for post-build auditing. Nice for enterprises, overkill for a personal tool. |

**Source:** [cargo-deny docs](https://embarkstudios.github.io/cargo-deny/checks/cfg.html), [RustSec](https://rustsec.org/), [cargo-audit crate](https://crates.io/crates/cargo-audit)

---

## 6. Code Coverage

### Recommendation: cargo-llvm-cov

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| `cargo-llvm-cov` | 0.8.4 | Source-based code coverage | More accurate than tarpaulin (LLVM instrumentation vs ptrace). Supports HTML, LCOV, and JSON output. Works on Linux, macOS, Windows. Fast -- only instruments necessary crates. | HIGH |

### CI Integration

```yaml
- run: cargo install cargo-llvm-cov --locked
- run: cargo llvm-cov --lcov --output-path lcov.info
- uses: codecov/codecov-action@v4
  with:
    files: lcov.info
```

### What NOT to use

| Alternative | Why Not |
|-------------|---------|
| `cargo-tarpaulin` | Linux-only, ptrace-based (less accurate). cargo-llvm-cov is the modern standard. |

**Source:** [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov), [Rust coverage docs](https://doc.rust-lang.org/beta/rustc/instrument-coverage.html)

---

## 7. Documentation

### Recommendation: rustdoc (API) + README-driven (user docs)

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| `cargo doc` (rustdoc) | Built-in | API documentation | Standard Rust doc tooling. Doc comments compile as tests. Zero additional deps. | HIGH |
| README.md | N/A | User-facing documentation | For a single-binary CLI tool, README + `--help` is sufficient. No need for mdBook or a docs site. | HIGH |
| `clap` derive `--help` | 4.x (existing) | CLI usage documentation | Already generates help text from struct attributes. Ensure all arguments have `#[arg(help = "...")]`. | HIGH |

### What NOT to use

| Alternative | Why Not |
|-------------|---------|
| mdBook | For book-length documentation (The Rust Programming Language, etc.). caldawarrior is a single-purpose CLI -- a README with config examples, a quick-start guide, and architecture notes is sufficient. |
| Dedicated docs site | Overengineering for a niche tool. README is the right format. |

### Documentation Strategy

1. **README.md** -- Installation, quick start, configuration reference, Docker usage.
2. **`cargo doc`** -- Run in CI to catch broken doc links/tests. Don't publish to docs.rs (binary crate).
3. **CHANGELOG.md** -- Maintain manually or generate with git-cliff (see Release below).
4. **Config example** -- Ship `config.example.toml` in the repo.

**Source:** [rustdoc book](https://doc.rust-lang.org/rustdoc/what-is-rustdoc.html), [mdBook](https://rust-lang.github.io/mdBook/)

---

## 8. Release Automation

### Recommendation: git-cliff for changelog + manual releases (for now)

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| `git-cliff` | 2.12.0 | Changelog generation | Rust-native, conventional-commits-aware, highly customizable templates. Generates CHANGELOG.md from git history. | MEDIUM |
| GitHub Releases | N/A | Binary distribution | Upload release binaries as assets. Manual or CI-triggered. | HIGH |

### What NOT to use (yet)

| Alternative | Why Not (for now) |
|-------------|-------------------|
| `release-plz` | Full automation (PR-based releases, crates.io publish, changelog). Excellent for library crates published frequently. Overkill for a v0.1 binary tool with manual release cadence. Revisit after v1.0. |
| `cargo-release` | Primarily for crates.io publishing workflow. caldawarrior is distributed as binary, not published to crates.io. |

### Release Strategy

1. Use conventional commits (`feat:`, `fix:`, `chore:`) for meaningful git history.
2. Run `git cliff --output CHANGELOG.md` before tagging a release.
3. Tag with `git tag v0.2.0`, push tag.
4. GitHub Actions workflow builds binaries and creates GitHub Release with assets.

**Source:** [git-cliff](https://crates.io/crates/git-cliff), [release-plz](https://release-plz.dev/), [Orhun's blog on automated Rust releases](https://blog.orhun.dev/automated-rust-releases/)

---

## Complete Stack Summary

### Dev Dependencies to Add

```toml
# Cargo.toml -- no changes needed for hardening
# All new tooling is external (cargo install, CI tools, Docker)
```

### External Tools

```bash
# CI tools (installed in GitHub Actions, not locally required)
cargo install cargo-deny --locked
cargo install cargo-llvm-cov --locked
cargo install cargo-audit --locked
cargo install git-cliff --locked
```

### Files to Create

| File | Purpose |
|------|---------|
| `Dockerfile` (project root) | Production Docker image |
| `deny.toml` | cargo-deny configuration |
| `.github/workflows/ci.yml` | CI pipeline |
| `config.example.toml` | Example configuration for users |
| `CHANGELOG.md` | Release history |

---

## Alternatives Considered (Global)

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| iCal validation | Python icalendar in RF tests | Rust icalendar/calcard crate | Adds unnecessary runtime dep; test-only concern belongs in test stack |
| CalDAV testing | Extend RF suite | CalDAVTester | Server-side tool; we're a client |
| Docker runner | archlinux:base | scratch/distroless/alpine | Need TaskWarrior 3.x at runtime; only Arch packages it |
| CI caching | Swatinem/rust-cache | sccache | Overkill for single-crate project |
| Coverage | cargo-llvm-cov | cargo-tarpaulin | LLVM-based is more accurate, cross-platform |
| Security audit | cargo-deny | cargo-audit alone | cargo-deny includes advisory scanning plus license/source checks |
| Docs | README + rustdoc | mdBook | Over-engineering for a CLI tool |
| Release | git-cliff + manual | release-plz | Premature automation for v0.x |

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| VTODO validation approach | HIGH | Python icalendar already in stack; strategy proven in existing tests |
| Docker packaging | HIGH | Archlinux runner constraint verified (TW 3.x only available there); existing test Dockerfile validates approach |
| CI/CD pipeline | HIGH | Standard GHA patterns, well-documented, widely used |
| Security auditing | HIGH | cargo-deny 0.19.0 and cargo-audit 0.22.1 verified on crates.io |
| Code coverage | HIGH | cargo-llvm-cov 0.8.4 is the community standard |
| Documentation | HIGH | Standard tooling, no novel choices |
| Release automation | MEDIUM | git-cliff is solid, but the "when to automate" question depends on release cadence |

---

## Sources

- [icalendar crate (crates.io)](https://crates.io/crates/icalendar) - v0.17.6, updated 2025-12-14
- [calcard crate (crates.io)](https://crates.io/crates/calcard) - v0.3.2, updated 2025-12-12
- [cargo-deny (crates.io)](https://crates.io/crates/cargo-deny) - v0.19.0, updated 2026-01-08
- [cargo-llvm-cov (crates.io)](https://crates.io/crates/cargo-llvm-cov) - v0.8.4, updated 2026-02-06
- [cargo-audit (crates.io)](https://crates.io/crates/cargo-audit) - v0.22.1, updated 2026-02-04
- [git-cliff (crates.io)](https://crates.io/crates/git-cliff) - v2.12.0, updated 2026-01-20
- [release-plz (crates.io)](https://crates.io/crates/release-plz) - v0.3.157, updated 2026-03-07
- [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache) - v2
- [dtolnay/rust-toolchain](https://github.com/dtolnay/rust-toolchain)
- [CalConnect CalDAVTester](https://github.com/CalConnect/caldavtester)
- [RFC 5545 VTODO](https://icalendar.org/iCalendar-RFC-5545/3-6-2-to-do-component.html)
- [iCalendar.org validator](https://icalendar.org/validator.html)
- [Distroless containers](https://github.com/GoogleContainerTools/distroless)
- [Docker multi-stage Rust builds](https://oneuptime.com/blog/post/2026-01-07-rust-minimal-docker-images/view)
- [Clippy documentation](https://doc.rust-lang.org/stable/clippy/lints.html)
- [rustdoc book](https://doc.rust-lang.org/rustdoc/what-is-rustdoc.html)
