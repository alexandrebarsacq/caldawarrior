# Project Retrospective

*A living document updated after each milestone. Lessons feed forward into future planning.*

## Milestone: v1.0 — Caldawarrior Hardening

**Shipped:** 2026-03-19
**Phases:** 7 | **Plans:** 15

### What Was Built
- Fixed 4 bug categories (CATEGORIES escaping, XML parser, ETag normalization, error context)
- Proved dependency relations work E2E with cycle detection and blocks/inverse mapping
- Verified all 10 mapped fields with create/update/clear E2E tests + idempotency suite
- DATE-only preservation, DST timezone handling, X-property round-trip survival
- CI/CD pipeline (4-job GitHub Actions) and tag-triggered binary releases
- Full README, config reference, CHANGELOG, compatibility matrix, 15 known limitations

### What Worked
- Risk-ordered phases: fixing bugs first (Phase 1) meant all subsequent test expansion validated correct behavior
- Parallel phase execution: Phases 2/3/4 were independent after Phase 1, enabling fast progress
- Gap closure cycle: Phase 3 initial verification found gaps → 03-03 plan fixed task import → re-verification passed
- Milestone audit caught README inaccuracies (MISS-01) → Phase 7 fixed them before shipping

### What Was Inefficient
- Phase 3 tw.update() used task import initially, which dropped caldavuid UDA in Docker TW3 — required a full gap closure plan (03-03) to revert to task modify. Could have been caught earlier with integration tests before E2E expansion.
- Docker Rust version pinned at 1.85 while codebase used let chains (requires 1.89+) — broke CI E2E builds. Caught late, after all phases were complete.
- SUMMARY.md frontmatter `requirements_completed` was empty for 06-01 — metadata oversight that showed up in audit cross-reference.

### Patterns Established
- Tag/annotation diff via `+tag/-tag` and `annotate/denotate` instead of full task import — correct approach for TW3
- `content_identical` must check all fields that affect writeback decisions (expanded from 8 to 10 fields)
- DST ambiguity resolution: `.latest()` for fall-back, `naive.and_utc()` for spring-forward gaps

### Key Lessons
1. Pin Docker build images to versions that support the language features your code uses — check MSRV against Dockerfile
2. Integration tests should run before expanding E2E tests — catches API-level bugs (like task import dropping UDAs) earlier
3. Milestone audits with 3-source cross-reference (VERIFICATION + SUMMARY + REQUIREMENTS) catch metadata gaps that single-source checks miss

### Cost Observations
- Model mix: primarily opus for execution, sonnet for verification and integration checking
- Notable: parallel phase execution after Phase 1 significantly compressed the timeline

---

## Cross-Milestone Trends

### Process Evolution

| Milestone | Phases | Plans | Key Change |
|-----------|--------|-------|------------|
| v1.0 | 7 | 15 | Gap closure cycle (verify → plan gaps → execute → re-verify) |

### Cumulative Quality

| Milestone | Rust Tests | RF E2E Tests | Test Total |
|-----------|-----------|-------------|------------|
| v1.0 | 192 unit + 18 integration | 80 (75 pass, 5 skip) | 290 |

### Top Lessons (Verified Across Milestones)

1. Fix bugs before expanding tests — tests against buggy code validate wrong behavior
2. Pin toolchain versions in CI to match what the code requires
