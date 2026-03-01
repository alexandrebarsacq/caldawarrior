# Fidelity Review: caldawarrior

**Spec ID:** caldawarrior-2026-02-26-001
**Scope:** phase (phase: phase-1)
**Verdict:** pass
**Date:** 2026-02-28T19:44:38.861793

## Summary

Phase 0 requirements are substantively met across all three tasks. The Rust project was initialized correctly with the [[bin]] entry, all dependency candidates (including icalendar = "0.16" at commit 1253344, later removed after evaluation as intended), and a passing cargo build. The Docker/Radicale stack is functional with a confirmed VTODO PUT/REPORT round-trip, though the host-side port is 5233 rather than the spec-stated 5232 — a low-severity deviation. The empirical research script (tw-behavior-research.sh) covers all 9 required behavioral items, achieved 13/13 passing tests against TW 3.4.2, and the findings are thoroughly documented in two well-structured ADRs that directly gate Phase 3 design decisions.

## Requirement Alignment
**Status:** yes

task-1-1: [[bin]] entry (name=caldawarrior, path=src/main.rs) present in Cargo.toml. All listed dependency candidates confirmed in the Phase 0 scaffold commit (1253344): serde/serde_json, reqwest (blocking+rustls-tls), chrono/chrono-tz, uuid, thiserror, anyhow, clap, icalendar=0.16. The icalendar crate was included at Phase 0 and later removed in subsequent phases after evaluation — this is exactly the behaviour implied by '(evaluated in Phase 0)'. Cargo.lock was committed in 1253344. task-1-2: docker-compose.yml + radicale.config created; VTODO round-trip verified per journal. task-1-3: Shell script covers all 9 behavioral items; ADRs written with required depth, decision tables, state machine, worked examples, and explicit Phase 3 gating section.

## Success Criteria
**Status:** yes

AC 1-1-a ([[bin]] name=caldawarrior): PASS — confirmed in Cargo.toml lines 6-8. AC 1-1-b (Cargo.lock committed): PASS — visible in commit 1253344 file list. AC 1-1-c (cargo build succeeds): PASS — journal records 232 packages compiled successfully. AC 1-2-a (Radicale at localhost:5232): PARTIAL — container listens on 5232 internally, but docker-compose maps host port 5233 → container 5232; external URL is localhost:5233, not localhost:5232. Functional requirement met; exact AC literal missed. AC 1-2-b (PUT/REPORT round-trip): PASS — journal documents MKCOL + PUT (201) + REPORT (207 with UID confirmed). AC 1-3-a (all 9 items in ADRs): PASS — tw-behavior-research.sh covers items 1-9; both ADRs document all findings with decision tables. AC 1-3-b (loop-prevention.md two-layer design with examples): PASS — four worked examples (A-D), state machine pseudocode, rationale for caldavuid anchor, and explicit Phase 3 requirements section. AC 1-3-c (tw-field-clearing.md field-clear and status transitions): PASS — covers trailing-colon clearing (caldavuid, due, scheduled), omit=clear semantics, all status transitions table, idempotency findings. AC 1-3-d (Phase 0 gates Phase 3): PASS — loop-prevention.md section 'Implementation Requirements for Phase 3' lists 7 concrete constraints derived from empirical findings.

## Deviations

- **[LOW]** docker-compose.yml exposes Radicale on host port 5233 (mapping 5233:5232) rather than the spec AC's stated localhost:5232.
  - Justification: Likely intentional to avoid collision with a user's existing local Radicale instance on the standard port. The radicale.config correctly binds internally to 0.0.0.0:5232, and the journal confirms full round-trip verification succeeded. The functional intent of the AC is fulfilled; only the literal port number differs.
- **[LOW]** The radicale.config enables htpasswd authentication (auth.type=htpasswd), requiring an htpasswd volume mount. The spec's AC does not specify auth configuration; the docker-compose.yml verify-radicale.sh (mentioned in journal but not read) presumably accounts for this. No auth=none as might be expected for a bare research setup.
  - Justification: Adds minor complexity for the research harness but does not violate any spec requirement. Credentials are supplied via volume mount and the round-trip was confirmed working.

## Test Coverage
**Status:** sufficient

Phase 0 is a scaffolding and research phase; automated unit/integration tests are not applicable. The empirical research script (tw-behavior-research.sh) serves as the Phase 0 'test suite' and is comprehensive: 13 assertions across 9 behavioral areas all passed against TW 3.4.2. Script uses isolated TASKDATA environments (mktemp), cleans up via trap, and includes both pass/fail assertions and design notes. This is appropriate coverage for a research phase.

## Code Quality

These are minor quality notes in research/scaffolding artifacts. No production Rust code was written in Phase 0, so there are no Rust code quality concerns at this stage.

- tw-behavior-research.sh item 2 contains a dead code path (lines 98-100: a failed/abandoned python3 approach immediately overridden on line 103). This is minor cleanup debt in a research script.
- The research script uses set -uo pipefail but not set -e, meaning individual command failures (other than pipefail-triggered ones) are absorbed by explicit exit-code checks. This is a deliberate design choice for test scripts but is worth noting.
- loop-prevention.md state machine pseudocode references 'last_synced_modified' as a derived value (not stored) but the pseudocode compares against it as if it were tracked — a slight conceptual tension that is explained in prose but could confuse Phase 3 implementors. The prose correctly clarifies the no-database design.

## Documentation
**Status:** adequate

Both ADRs are well-structured with date, status, context, problem statement, design rationale, worked examples, and explicit downstream requirements. tw-field-clearing.md includes a decision table directly usable by Phase 3 implementors. loop-prevention.md covers the 'why' (problem), 'what' (two layers), 'how' (state machine + invariants), and 'so what' (Phase 3 requirements). The research script itself is documented with section headers and design notes. No gaps for a Phase 0 deliverable.

## Issues

- Host port 5233 used in docker-compose.yml instead of spec AC's stated localhost:5232 — functional but deviates from the acceptance criterion letter.
- Dead code path in tw-behavior-research.sh item 2 (unused python3 block before the correct implementation on line 103).

## Recommendations

- Update docker-compose.yml to map port 5232:5232, or update any downstream scripts/docs that reference the Radicale URL to use port 5233, to eliminate the discrepancy with the spec AC.
- Remove the dead code block in tw-behavior-research.sh lines 98-100 (the first, unused DELETED_JSON_RAW approach) to keep the research script clean for future reference.
- Consider adding a brief note in loop-prevention.md clarifying that 'last_synced_modified' in the state machine is derived at runtime from TW.modified after a successful sync (not a stored value), to prevent Phase 3 implementors from introducing unnecessary state storage.

---
*Generated by Foundry MCP Fidelity Review*