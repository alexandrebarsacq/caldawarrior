# Phase 5: CI/CD and Packaging - Research

**Researched:** 2026-03-19
**Domain:** GitHub Actions CI/CD, Rust build tooling, cargo-deny security audit, release automation
**Confidence:** HIGH

## Summary

Phase 5 is entirely greenfield -- no `.github/` directory or `deny.toml` exists. The project needs two GitHub Actions workflow files: `ci.yml` (lint, test, E2E, audit on every push/PR) and `release.yml` (build and publish binary on `v*` tags). The Rust ecosystem has well-established patterns for this using `dtolnay/rust-toolchain`, `Swatinem/rust-cache`, `EmbarkStudios/cargo-deny-action`, and `softprops/action-gh-release`.

Two critical pre-existing issues must be resolved before CI can pass: **157 rustfmt violations** across the codebase and **47 clippy warnings** (predominantly `result_large_err` at 21 occurrences, plus `collapsible_if`, `ptr_arg`, and others). These must be fixed or explicitly allowed BEFORE the CI workflow is useful, otherwise the very first CI run will fail. The RF E2E tests use Docker Compose which is pre-installed on GitHub Actions `ubuntu-latest` runners -- the existing Makefile commands work directly in CI.

For the release binary, the project uses `reqwest` with both `default-tls` (OpenSSL) and `rustls-tls` features enabled. To produce a statically-linked binary, the release workflow must build with `x86_64-unknown-linux-musl` target AND set `default-features = false` for reqwest to exclude the OpenSSL dependency, or alternatively accept a dynamically-linked glibc binary. The MUSL approach is recommended for true portability.

**Primary recommendation:** Fix fmt/clippy issues first, then create ci.yml and release.yml using the standard action ecosystem (dtolnay toolchain, Swatinem cache, cargo-deny-action, softprops gh-release).

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Tag pattern: `v*` semver (e.g., v1.0.0, v1.2.3-rc1)
- Binary naming: `caldawarrior-v{version}-x86_64-linux` (version baked into filename)
- Standalone binary -- no tar.gz archive, single statically-linked executable
- SHA256 checksum file alongside binary: `caldawarrior-v{version}-x86_64-linux.sha256`
- Separate `release.yml` workflow file, triggered only on `v*` tags
- Single `ci.yml` workflow file with multiple parallel jobs
- Triggered on every push and PR
- Jobs: lint (fmt + clippy), test (unit + integration), e2e (RF), audit (cargo-deny)
- Cache cargo registry and target/ between runs for faster builds
- RF tests are a blocking check -- must pass before merge
- Run Docker Compose directly on the GitHub Actions Ubuntu runner (Docker pre-installed)
- Same commands as local Makefile -- no service container rewrite needed

### Claude's Discretion
- cargo-deny: configure a sensible default deny.toml covering licenses, advisories, and banned crates

### Deferred Ideas (OUT OF SCOPE)
- aarch64-linux and macOS binary releases -- v2 (PKG-03)
- crates.io publishing via cargo install -- v2 (PKG-04)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| PKG-01 | GitHub Actions CI pipeline runs unit tests, integration tests, RF E2E tests, and cargo-deny security audit | ci.yml workflow with 4 parallel jobs (lint, test, e2e, audit), using dtolnay/rust-toolchain + Swatinem/rust-cache + EmbarkStudios/cargo-deny-action. Pre-requisite: fix 157 fmt violations and 47 clippy warnings. |
| PKG-02 | Pre-built binary releases published on GitHub Releases for x86_64-linux | release.yml workflow triggered on `v*` tags, building with MUSL target for static binary, using softprops/action-gh-release@v2 to upload binary + SHA256 checksum. |
</phase_requirements>

## Standard Stack

### Core

| Tool/Action | Version | Purpose | Why Standard |
|-------------|---------|---------|--------------|
| dtolnay/rust-toolchain | @stable (or @master with toolchain: 1.85.0+) | Install Rust toolchain in CI | De facto standard, maintained, replaces deprecated actions-rs |
| Swatinem/rust-cache | @v2 | Cache cargo registry + target/ | Purpose-built for Rust, smart cache invalidation |
| EmbarkStudios/cargo-deny-action | @v2 | Run cargo-deny checks | Official action from cargo-deny maintainers |
| softprops/action-gh-release | @v2 | Create GitHub Release + upload assets | Most popular release action, supports file globs |
| actions/checkout | @v4 | Checkout code | Standard GitHub Action |
| actions/upload-artifact | @v4 | Upload RF test results on failure | Standard GitHub Action for artifact preservation |

### Supporting

| Tool | Purpose | When to Use |
|------|---------|-------------|
| cargo-deny (deny.toml) | License, advisory, ban, source policy config | Created once, checked every CI run |
| x86_64-unknown-linux-musl | MUSL target for static binary | Release workflow only |
| musl-tools (apt package) | MUSL linker for static compilation | Release workflow only |
| sha256sum | Generate checksum file | Release workflow only |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| dtolnay/rust-toolchain | actions-rust-lang/setup-rust-toolchain | setup-rust-toolchain adds problem matchers but is less established |
| softprops/action-gh-release | gh CLI release create | gh CLI works but softprops handles asset uploads more cleanly |
| MUSL static build | glibc dynamic build | Dynamic is simpler but not portable across Linux distros |
| Separate lint/test jobs | Single monolith job | Parallel jobs give faster feedback but slightly more YAML |

## Architecture Patterns

### Recommended Project Structure
```
.github/
  workflows/
    ci.yml            # Lint + test + E2E + audit (every push/PR)
    release.yml       # Build + publish binary (v* tags only)
deny.toml             # cargo-deny policy configuration
```

### Pattern 1: Parallel CI Jobs with Shared Toolchain Setup

**What:** Each CI job independently sets up the Rust toolchain and cache, running in parallel.
**When to use:** Always -- parallel jobs give faster wall-clock time.
**Example:**

```yaml
# Source: dtolnay/rust-toolchain + Swatinem/rust-cache official READMEs
name: CI
on:
  push:
  pull_request:

env:
  CARGO_TERM_COLOR: always

jobs:
  lint:
    name: Lint
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --check
      - run: cargo clippy -- -D warnings

  test:
    name: Test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --lib
      - run: cargo test --test integration

  e2e:
    name: E2E (Robot Framework)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: mkdir -p tests/robot/results
      - run: |
          CURRENT_UID=$(id -u) CURRENT_GID=$(id -g) \
            docker compose -f tests/robot/docker-compose.yml run --rm robot

  audit:
    name: Security Audit
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: EmbarkStudios/cargo-deny-action@v2
```

### Pattern 2: Tag-Triggered Release with MUSL Static Build

**What:** Build a statically-linked binary using MUSL target when a version tag is pushed.
**When to use:** release.yml only.
**Example:**

```yaml
# Source: combined from dtolnay/rust-toolchain + softprops/action-gh-release docs
name: Release
on:
  push:
    tags:
      - "v*"

permissions:
  contents: write

jobs:
  release:
    name: Build and Release
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: x86_64-unknown-linux-musl
      - name: Install musl-tools
        run: sudo apt-get update && sudo apt-get install -y musl-tools
      - uses: Swatinem/rust-cache@v2
      - name: Build static binary
        run: cargo build --release --target x86_64-unknown-linux-musl
      - name: Prepare release assets
        run: |
          VERSION="${GITHUB_REF_NAME}"
          BINARY="caldawarrior-${VERSION}-x86_64-linux"
          cp target/x86_64-unknown-linux-musl/release/caldawarrior "${BINARY}"
          sha256sum "${BINARY}" > "${BINARY}.sha256"
      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          generate_release_notes: true
          files: |
            caldawarrior-v*-x86_64-linux
            caldawarrior-v*-x86_64-linux.sha256
```

### Pattern 3: cargo-deny Configuration (deny.toml)

**What:** Sensible-default deny.toml covering licenses, advisories, and banned crates.
**When to use:** Created once at project root, checked every CI run.
**Example:**

```toml
# Source: EmbarkStudios/cargo-deny template + project-specific config
[graph]
targets = [{ triple = "x86_64-unknown-linux-gnu" }]

[advisories]
vulnerability = "deny"
unmaintained = "warn"
unsound = "warn"
yanked = "warn"

[licenses]
unlicensed = "deny"
allow = [
    "MIT",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Unicode-3.0",
    "Unicode-DFS-2016",
    "Zlib",
]

[[licenses.clarify]]
name = "ring"
expression = "MIT AND ISC AND OpenSSL"
license-files = [{ path = "LICENSE", hash = 0xbd0eed23 }]

[bans]
multiple-versions = "warn"
wildcards = "allow"

[sources]
unknown-registry = "warn"
unknown-git = "warn"
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
allow-git = []
```

### Anti-Patterns to Avoid
- **Using `actions-rs/toolchain`:** Unmaintained since 2022, has known bugs. Use `dtolnay/rust-toolchain` instead.
- **Single monolith CI job:** Runs fmt, clippy, test, E2E sequentially -- wastes time. Use parallel jobs.
- **Caching the entire `target/` directory:** Swatinem/rust-cache handles this correctly; manual `actions/cache` is error-prone for Rust.
- **Using `docker-compose` (hyphenated):** GitHub Actions ubuntu-latest only has Docker Compose v2 (`docker compose` with space). The Makefile already uses the correct syntax.
- **Building release binary with default glibc target:** Produces dynamically-linked binary that may not work across all Linux distros.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Rust toolchain install | Shell script with rustup | `dtolnay/rust-toolchain` | Handles caching, components, targets properly |
| Cargo caching | Manual `actions/cache` with key management | `Swatinem/rust-cache@v2` | Knows Rust-specific cache invalidation (lockfile, toolchain) |
| Dependency audit | Custom license/advisory scanning | `cargo-deny` + `EmbarkStudios/cargo-deny-action@v2` | Checks RustSec DB, SPDX licenses, crate bans, source trust |
| GitHub Release creation | gh CLI scripts with asset loops | `softprops/action-gh-release@v2` | Handles idempotent release creation + multi-file upload |
| SHA256 checksum | Custom hashing script | `sha256sum` (coreutils) | Pre-installed on ubuntu-latest, single command |

**Key insight:** GitHub Actions has mature, maintained actions for every step of the Rust CI/release pipeline. Hand-rolling any of these introduces maintenance burden for zero benefit.

## Common Pitfalls

### Pitfall 1: Pre-existing fmt/clippy Violations Cause Immediate CI Failure
**What goes wrong:** Enabling `cargo fmt --check` and `cargo clippy -- -D warnings` in CI when the codebase has violations means CI fails from day one.
**Why it happens:** Current state: 157 rustfmt violations, 47 clippy warnings (21x `result_large_err`, 6x `collapsible_if`, and others).
**How to avoid:** Fix all fmt/clippy issues BEFORE enabling CI, or as the first task in this phase. Run `cargo fmt` to auto-fix formatting. For clippy, either fix warnings or add targeted `#[allow()]` attributes for intentional deviations.
**Warning signs:** CI fails on the very first push after workflow creation.

### Pitfall 2: Reqwest default-tls Breaks MUSL Static Build
**What goes wrong:** Building with `--target x86_64-unknown-linux-musl` fails because reqwest pulls in OpenSSL via the `default-tls` feature (which is enabled by default).
**Why it happens:** `Cargo.toml` has `reqwest = { version = "0.12", features = ["blocking", "rustls-tls"] }` -- this ADDS rustls-tls but does NOT disable default-tls. The dependency tree currently includes `openssl-sys`, `native-tls`.
**How to avoid:** Change Cargo.toml to: `reqwest = { version = "0.12", default-features = false, features = ["blocking", "rustls-tls"] }`. This removes the OpenSSL dependency entirely. Verify with `cargo tree | grep openssl` after the change.
**Warning signs:** Linker errors mentioning `openssl` or `ssl` during MUSL build.

### Pitfall 3: Docker Compose Build Context in CI
**What goes wrong:** The RF Dockerfile uses `context: ../..` (project root) to COPY Cargo.toml, Cargo.lock, and src/. If the checkout is shallow or paths differ, the build fails.
**Why it happens:** Docker Compose resolves the build context relative to the docker-compose.yml location.
**How to avoid:** `actions/checkout@v4` does a full checkout by default into `$GITHUB_WORKSPACE` which is the working directory. The Makefile command works as-is because it specifies `-f tests/robot/docker-compose.yml` from the project root.
**Warning signs:** Docker build fails with "COPY failed: file not found" in CI.

### Pitfall 4: UID/GID Mismatch in GitHub Actions
**What goes wrong:** The Docker Compose setup passes `CURRENT_UID` and `CURRENT_GID` for file permissions. In GitHub Actions, the runner user is UID 1001 (runner), not 1000.
**Why it happens:** docker-compose.yml uses `user: "${CURRENT_UID:-1000}:${CURRENT_GID:-1000}"` which falls back to 1000 if not set.
**How to avoid:** The Makefile correctly uses `$(shell id -u)` and `$(shell id -g)`. In CI, either use `make test-robot` or replicate the env vars: `CURRENT_UID=$(id -u) CURRENT_GID=$(id -g) docker compose ...`. The Makefile approach is preferred.
**Warning signs:** Permission denied errors writing to `/results` in the RF container.

### Pitfall 5: Rust Edition 2024 Requires Minimum 1.85
**What goes wrong:** CI uses `@stable` which resolves to the latest stable Rust, but future Rust updates could introduce new clippy lints that break CI.
**Why it happens:** The project uses `edition = "2024"` which requires Rust 1.85+. Using `@stable` always tracks the latest stable.
**How to avoid:** Using `@stable` is fine -- it always satisfies the 1.85 minimum. For clippy lint stability, consider adding a `rust-toolchain.toml` to pin a specific version, or accept that new lints may need to be addressed periodically.
**Warning signs:** Unexpected clippy failures after a Rust stable update.

### Pitfall 6: ring Crate License Clarification for cargo-deny
**What goes wrong:** cargo-deny cannot automatically determine the license for the `ring` crate (used by rustls) because it uses a custom license file that combines MIT, ISC, and OpenSSL licenses.
**Why it happens:** ring's license doesn't match standard SPDX identifiers cleanly.
**How to avoid:** Add a `[[licenses.clarify]]` section for `ring` in deny.toml (see code examples). The hash value must be computed from the actual LICENSE file in the ring crate version used.
**Warning signs:** cargo-deny fails with "failed to satisfy license requirements" for ring.

### Pitfall 7: RF E2E Tests Require Docker Image Build (Slow First Run)
**What goes wrong:** First CI run takes a very long time because Docker needs to build the RF test runner image from scratch (Rust compilation inside Docker + Arch Linux package install).
**Why it happens:** Docker layer caching is not persistent across GitHub Actions runs by default.
**How to avoid:** Accept the initial build time (~5-10 min). Docker BuildKit layer caching via `docker/setup-buildx-action` + `docker/build-push-action` with cache-to/cache-from could speed things up but adds complexity. For a small project, the build time is acceptable.
**Warning signs:** E2E job takes 10+ minutes consistently.

## Code Examples

### ci.yml Complete Workflow

```yaml
# Source: Official docs for dtolnay/rust-toolchain, Swatinem/rust-cache, EmbarkStudios/cargo-deny-action
name: CI

on:
  push:
  pull_request:

env:
  CARGO_TERM_COLOR: always

jobs:
  lint:
    name: Lint (fmt + clippy)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - name: Check formatting
        run: cargo fmt --check
      - name: Run clippy
        run: cargo clippy -- -D warnings

  test:
    name: Unit + Integration Tests
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Run unit tests
        run: cargo test --lib
      - name: Run integration tests
        run: cargo test --test integration

  e2e:
    name: E2E (Robot Framework)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Create results directory
        run: mkdir -p tests/robot/results
      - name: Run RF E2E tests
        run: |
          CURRENT_UID=$(id -u) CURRENT_GID=$(id -g) \
            docker compose -f tests/robot/docker-compose.yml run --rm robot
      - name: Upload RF results on failure
        if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: robot-results
          path: tests/robot/results/

  audit:
    name: Security Audit (cargo-deny)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: EmbarkStudios/cargo-deny-action@v2
```

### release.yml Complete Workflow

```yaml
# Source: Official docs for softprops/action-gh-release, dtolnay/rust-toolchain
name: Release

on:
  push:
    tags:
      - "v*"

permissions:
  contents: write

jobs:
  release:
    name: Build and Publish
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: x86_64-unknown-linux-musl
      - name: Install musl-tools
        run: sudo apt-get update && sudo apt-get install -y musl-tools
      - uses: Swatinem/rust-cache@v2
        with:
          key: release-musl
      - name: Build static binary
        run: cargo build --release --target x86_64-unknown-linux-musl
      - name: Prepare release assets
        run: |
          VERSION="${GITHUB_REF_NAME}"
          BINARY_NAME="caldawarrior-${VERSION}-x86_64-linux"
          cp target/x86_64-unknown-linux-musl/release/caldawarrior "${BINARY_NAME}"
          sha256sum "${BINARY_NAME}" > "${BINARY_NAME}.sha256"
      - name: Publish to GitHub Releases
        uses: softprops/action-gh-release@v2
        with:
          generate_release_notes: true
          files: |
            caldawarrior-v*-x86_64-linux
            caldawarrior-v*-x86_64-linux.sha256
```

### deny.toml Sensible Defaults

```toml
# Source: cargo-deny template + project dependency analysis
# Caldawarrior dependency policy
# Docs: https://embarkstudios.github.io/cargo-deny/

[graph]
targets = [
    { triple = "x86_64-unknown-linux-gnu" },
    { triple = "x86_64-unknown-linux-musl" },
]

[advisories]
vulnerability = "deny"
unmaintained = "warn"
unsound = "warn"
yanked = "warn"

[licenses]
unlicensed = "deny"
allow = [
    "MIT",
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Unicode-3.0",
    "Unicode-DFS-2016",
    "Zlib",
]

# ring uses a non-standard license file combining MIT + ISC + OpenSSL
# The hash must be verified against the actual ring version in Cargo.lock
[[licenses.clarify]]
name = "ring"
expression = "MIT AND ISC AND OpenSSL"
license-files = [{ path = "LICENSE", hash = 0xbd0eed23 }]

[bans]
multiple-versions = "warn"
wildcards = "allow"

[sources]
unknown-registry = "warn"
unknown-git = "warn"
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
allow-git = []
```

### Fixing Reqwest for MUSL Build

```toml
# In Cargo.toml -- CRITICAL change for static binary
# Before (pulls in OpenSSL via default-tls):
# reqwest = { version = "0.12", features = ["blocking", "rustls-tls"] }

# After (rustls only, no OpenSSL):
reqwest = { version = "0.12", default-features = false, features = ["blocking", "rustls-tls"] }
```

Verify after change:
```bash
# Should return no results:
cargo tree | grep openssl
cargo tree | grep native-tls
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `actions-rs/toolchain` | `dtolnay/rust-toolchain` | 2023 (actions-rs unmaintained) | Must use dtolnay for reliable CI |
| `docker-compose` (hyphenated) | `docker compose` (space) | 2024 (Compose v2 default) | Ubuntu-latest only has v2 |
| Manual `actions/cache` for Rust | `Swatinem/rust-cache@v2` | 2023+ (v2 stable) | Smart cache keys, less config |
| `EmbarkStudios/cargo-deny-action@v1` | `@v2` | 2024 | Simplified inputs, faster |
| `softprops/action-gh-release@v1` | `@v2` | 2024 | Cross-platform support |
| MUSL libc 1.2.3 | MUSL libc 1.2.5 | Rust 1.93 (Jan 2026) | Better DNS resolver in static binaries |

**Deprecated/outdated:**
- `actions-rs/*` (all actions in this namespace): Unmaintained since 2022, known bugs
- `docker-compose` CLI v1: Removed from ubuntu-latest, use `docker compose` v2
- `actions/upload-release-asset`: Deprecated, use `softprops/action-gh-release` instead

## Open Questions

1. **ring license hash value**
   - What we know: ring needs a `[[licenses.clarify]]` entry in deny.toml with a hash of its LICENSE file
   - What's unclear: The exact hash value `0xbd0eed23` is commonly cited but must be verified against the specific ring version in Cargo.lock (currently ring 0.17.14)
   - Recommendation: Run `cargo deny check licenses` locally after creating deny.toml; if the hash is wrong, cargo-deny will report the correct hash

2. **Clippy warning resolution strategy**
   - What we know: 47 clippy warnings exist, 21 are `result_large_err` (large Error enum variant)
   - What's unclear: Whether the user prefers to fix all warnings properly or use `#[allow()]` attributes for some
   - Recommendation: Fix the easy ones (collapsible_if, redundant_closure, etc.) and use project-level `#[allow(clippy::result_large_err)]` in lib.rs for the Error enum issue (refactoring it would be a significant change outside this phase's scope)

3. **Docker layer caching for E2E job speed**
   - What we know: First RF E2E build will be slow (~5-10 min) due to Rust compilation + Arch Linux package install in Docker
   - What's unclear: Whether the build time is acceptable or caching is needed
   - Recommendation: Start without Docker layer caching; optimize later if E2E job time becomes a pain point

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust built-in) + Robot Framework 7.0.1 (Docker) |
| Config file | Cargo.toml (test config) + tests/robot/docker-compose.yml (E2E) |
| Quick run command | `cargo test --lib` |
| Full suite command | `cargo test && make test-robot` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PKG-01 | CI pipeline runs all checks on push/PR | manual-only | Push to GitHub and verify Actions pass | N/A -- workflow files are tested by running them |
| PKG-01a | cargo fmt --check passes | unit | `cargo fmt --check` | N/A -- built-in tool |
| PKG-01b | cargo clippy passes | unit | `cargo clippy -- -D warnings` | N/A -- built-in tool |
| PKG-01c | Unit tests pass | unit | `cargo test --lib` | Existing: 192 tests |
| PKG-01d | Integration tests pass | integration | `cargo test --test integration` | Existing: 18 tests |
| PKG-01e | RF E2E tests pass | e2e | `make test-robot` | Existing: 9 suites |
| PKG-01f | cargo-deny audit passes | smoke | `cargo deny check` | deny.toml must be created |
| PKG-02 | Release binary published on tag push | manual-only | Push a `v*` tag and verify GitHub Release | N/A -- workflow tested by running |
| PKG-02a | Static binary builds | smoke | `cargo build --release --target x86_64-unknown-linux-musl` | N/A -- tested in release workflow |

### Sampling Rate
- **Per task commit:** `cargo fmt --check && cargo clippy -- -D warnings && cargo test --lib`
- **Per wave merge:** `cargo test && cargo deny check`
- **Phase gate:** Push workflow files to GitHub, trigger CI run, verify all jobs pass green

### Wave 0 Gaps
- [ ] `.github/workflows/ci.yml` -- CI workflow file (PKG-01)
- [ ] `.github/workflows/release.yml` -- release workflow file (PKG-02)
- [ ] `deny.toml` -- cargo-deny configuration (PKG-01f)
- [ ] Fix 157 rustfmt violations -- `cargo fmt` auto-fix (PKG-01a)
- [ ] Fix/allow 47 clippy warnings (PKG-01b)
- [ ] Update `Cargo.toml` reqwest to `default-features = false` for MUSL build (PKG-02a)

## Sources

### Primary (HIGH confidence)
- [dtolnay/rust-toolchain](https://github.com/dtolnay/rust-toolchain) - Toolchain setup action, usage patterns
- [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache) - Cache configuration, v2 API
- [EmbarkStudios/cargo-deny-action](https://github.com/EmbarkStudios/cargo-deny-action) - Usage YAML, matrix strategy
- [EmbarkStudios/cargo-deny template](https://github.com/EmbarkStudios/cargo-deny/blob/main/deny.template.toml) - deny.toml template
- [softprops/action-gh-release](https://github.com/softprops/action-gh-release) - Release creation, asset upload, permissions
- [actions/checkout@v4](https://github.com/actions/checkout) - Standard checkout action
- [actions/upload-artifact@v4](https://github.com/actions/upload-artifact) - Artifact upload for failure debugging

### Secondary (MEDIUM confidence)
- [LukeMathWalker CI gist](https://gist.github.com/LukeMathWalker/5ae1107432ce283310c3e601fac915f3) - Verified CI workflow patterns
- [shift.click Rust GH Actions recipes](https://shift.click/blog/github-actions-rust/) - Additional workflow patterns
- [Rust blog: Updating musl 1.2.5](https://blog.rust-lang.org/2025/12/05/Updating-musl-1.2.5/) - MUSL version info for Rust 1.93

### Tertiary (LOW confidence)
- ring license hash `0xbd0eed23` -- commonly cited but must be verified locally against ring 0.17.14

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All actions verified from official GitHub repositories and READMEs
- Architecture: HIGH - Patterns are well-established for Rust CI/CD on GitHub Actions
- Pitfalls: HIGH - Pre-existing fmt/clippy issues verified locally; reqwest OpenSSL dependency confirmed via `cargo tree`
- deny.toml: MEDIUM - Template is from official source but ring license hash needs local verification

**Research date:** 2026-03-19
**Valid until:** 2026-04-19 (stable ecosystem, 30-day validity)
