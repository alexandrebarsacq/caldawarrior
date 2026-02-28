# Fidelity Review: caldawarrior

**Spec ID:** caldawarrior-2026-02-26-001
**Scope:** phase (phase: phase-1)
**Verdict:** pass
**Date:** 2026-02-26T15:13:38.555158

## Summary

All three Phase 0 tasks are fully implemented and satisfy their acceptance criteria. The Rust project is correctly scaffolded with the required [[bin]] entry, all dependency candidates, and a committed Cargo.lock. The Radicale Docker setup provides a complete VTODO PUT/REPORT round-trip harness. The empirical research script covers all 9 required behavioral items (13 sub-tests, 13/13 passing against TW 3.4.2), and both required ADRs are thorough, actionable, and explicitly gate Phase 3 implementation decisions. Minor issues are limited to low severity.

## Requirement Alignment
**Status:** yes

task-1-1: Cargo.toml has correct [[bin]] entry (name='caldawarrior', path='src/main.rs'), all required dependency candidates are present (serde/serde_json, reqwest with blocking+rustls-tls, chrono/chrono-tz, uuid, thiserror, anyhow, clap, icalendar), and Cargo.lock exists. task-1-2: docker-compose.yml binds Radicale to port 5232 with auth=none and filesystem storage; verify-radicale.sh performs the full MKCOL + PUT VTODO + REPORT round-trip. task-1-3: tw-behavior-research.sh covers all 9 behavioral items (14 test assertions); docs/adr/tw-field-clearing.md and docs/adr/loop-prevention.md document all findings with explicit Phase 3 decision tables and implementation requirements.

## Success Criteria
**Status:** yes

AC1-1a: [[bin]] name='caldawarrior' confirmed in Cargo.toml. AC1-1b: Cargo.lock present and committed per git log (commit 1253344). AC1-1c: cargo build confirmed via journal (232 packages). AC1-2a: Radicale exposed at http://localhost:5232 confirmed by docker-compose.yml port mapping. AC1-2b: verify-radicale.sh performs MKCOL + PUT + REPORT and checks UID in response. AC1-3a: All 9 behavioral items covered in tw-behavior-research.sh and documented in ADRs. AC1-3b: loop-prevention.md has two-layer design with four worked examples (A–D). AC1-3c: tw-field-clearing.md covers all field-clear methods (trailing colon, import-omit=clear) and all status transitions. AC1-3d: loop-prevention.md contains an explicit 'Implementation Requirements for Phase 3' section with 7 concrete requirements derived from empirical findings.

## Deviations

- **[LOW]** reqwest TLS feature is 'rustls-tls' rather than the generic 'tls' alias mentioned in the spec.
  - Justification: rustls-tls is a pure-Rust, more portable TLS implementation and a strictly valid choice. The spec's 'blocking+tls' description was not prescriptive about the TLS backend. No functional difference for the sync use case.
- **[LOW]** A foundry artifact directory (tests/integration/specs/) was committed in the initial Radicale setup commit, then removed in a follow-up cleanup commit.
  - Justification: Already resolved: a .gitignore entry now excludes the path. The commit history is slightly noisier but the working tree is clean.
- **[LOW]** No explicit ADR documenting the evaluation/selection of the 'icalendar' crate, which the spec describes as 'evaluated in Phase 0'.
  - Justification: The spec task description says the crate is a 'dependency candidate' to be evaluated; no separate ADR file is listed in the acceptance criteria for task-1-1. The crate is included in Cargo.toml. Formal evaluation doc is a Phase 1 concern.
- **[LOW]** The loop-prevention.md state machine introduces a 'last_synced_modified' field that could imply persistent external storage, in apparent tension with the spec's 'no-database design' mission.
  - Justification: Reading the ADR's combined invariant closely (TW.modified == CalDAV.LAST-MODIFIED → in sync), last_synced_modified is derivable at runtime from TW's stored modified timestamp — no separate DB is required. The notation is a presentation choice, not an implementation commitment. Phase 3 can resolve this explicitly.

## Test Coverage
**Status:** sufficient

tw-behavior-research.sh is the primary test artefact for Phase 0. It runs 13 assertions across 9 behavioral items (with sub-tests for item 7 and item 9) and reports 13/13 passing against TW 3.4.2. The script uses isolated TASKDATA environments to prevent pollution. verify-radicale.sh provides an end-to-end CalDAV round-trip check. Phase 0 is a scaffolding/research phase; unit or integration tests for production code are not expected until Phase 1–5.

## Code Quality

The research script is well-structured: isolated TASKDATA environments, helper functions (tw/twj/py), and a clear PASS/FAIL/NOTES summary. The ADRs are production-quality documentation with decision tables, implications, and explicit Phase 3 requirements. No security concerns for Phase 0 artefacts.

- tw-behavior-research.sh uses 'set -uo pipefail' but omits 'set -e'; this is intentional to handle expected non-zero exits (e.g., task delete on already-deleted task), but it means unhandled errors in helper calls could be silently swallowed in some paths.
- Item 8 (task delete idempotency) pass/fail logic: if the second delete message doesn't match the grep patterns, it falls through to a 'note' rather than an explicit 'pass' or 'fail', potentially understating a failure.
- src/main.rs and src/lib.rs are empty scaffolding stubs — expected for Phase 0 but worth noting for completeness.

## Documentation
**Status:** adequate

Both required ADR files are present and well-written. docs/adr/tw-field-clearing.md covers field clearing, import semantics, status transitions, and a decision table. docs/adr/loop-prevention.md covers the two-layer design rationale, four worked examples, a state machine, and concrete Phase 3 requirements. The research script itself is heavily commented. README is not expected until Phase 6.

## Issues

- No critical or high-severity issues found.
- Minor: rustls-tls vs tls alias (functionally equivalent, low severity).
- Minor: foundry artifact committed then cleaned up — already resolved.
- Minor: no icalendar evaluation ADR (not required by Phase 0 ACs).
- Minor: loop-prevention state machine notation could be clarified re: no-database invariant.

## Recommendations

- In Phase 1 or Phase 3, add a brief ADR or Cargo.toml comment documenting the icalendar crate evaluation rationale (version 0.16, feature set, any known limitations for VTODO serialization).
- In loop-prevention.md or the Phase 3 design, explicitly clarify that 'last_synced_modified' is derived from TW's stored modified field (not a separate database), reinforcing the no-database design invariant.
- Consider adding an 'exit 1' at the end of tw-behavior-research.sh when FAIL > 0, so CI/automation can detect research regressions automatically.
- For item 8 in the research script, tighten the pass/fail condition to cover the known TW 3.4.2 error message ('is not deletable') explicitly, to avoid silent note-instead-of-fail scenarios on future TW versions.

---
*Generated by Foundry MCP Fidelity Review*