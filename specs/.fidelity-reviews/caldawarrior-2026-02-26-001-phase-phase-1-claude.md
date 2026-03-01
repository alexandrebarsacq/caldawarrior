# Fidelity Review: caldawarrior

**Spec ID:** caldawarrior-2026-02-26-001
**Scope:** phase (phase: phase-1)
**Verdict:** pass
**Date:** 2026-02-28T15:45:44.777619
**Provider:** claude

## Summary

Phase 0 (Scaffolding & Empirical Research) is substantially complete. All three tasks were implemented: the Rust project scaffolds correctly with the required [[bin]] entry and dependency candidates, the Docker/Radicale compose file exists and was verified for the VTODO round-trip, and the empirical research script covers all 9 behavioral items with results documented in two well-structured ADRs. Two deviations of medium/low severity were found: (1) a port mismatch between docker-compose.yml (host port 5233) and the verify-radicale.sh script and spec AC (port 5232); (2) the `icalendar` crate listed as a dependency candidate in the spec is absent from Cargo.toml without an explicit ADR documenting the rejection decision. Neither deviation undermines the research outcomes or Phase 0's gating role for Phase 3.

## Requirement Alignment
**Status:** partial

task-1-1 (Rust project init): All ACs met — [[bin]] entry present with name=caldawarrior and path=src/main.rs; all specified dependency candidates are present except `icalendar`; extra `toml` crate added without spec mention; journal confirms cargo build passed and Cargo.lock committed. task-1-2 (Docker/Radicale): docker-compose.yml exists with the tomsquest/docker-radicale image, but the host-side port is 5233 (not 5232 as the AC specifies: 'Radicale at http://localhost:5232'). verify-radicale.sh hardcodes port 5232, creating an internal inconsistency—running docker compose up and then verify-radicale.sh as written would fail because the container is exposed on 5233. The journal records a successful round-trip, so verification was apparently done, but the artifacts are inconsistent. task-1-3 (Empirical research): fully aligned — research script covers all 9 items, 13/13 tests passed per journal, both ADRs are comprehensive, and loop-prevention.md closes with explicit Phase 3 implementation requirements.

## Success Criteria
**Status:** partial

task-1-1: [[bin]] entry AC met; Cargo.lock AC accepted on journal evidence; cargo build AC met per journal. task-1-2 AC1 ('docker-compose up starts Radicale at http://localhost:5232') — NOT strictly met because docker-compose.yml maps host port 5233:5232, so Radicale listens on localhost:5233 on the host, not 5232. AC2 (VTODO PUT/REPORT round-trip verified) — met per journal; verify-radicale.sh script exists and performs full MKCOL/PUT/REPORT/UID-check sequence. task-1-3: all four ACs met — 9 behavioral items documented, loop-prevention.md has two-layer design with 4 worked examples and a state machine, tw-field-clearing.md covers all field-clear and status-transition findings, and the 'Implementation Requirements for Phase 3' section explicitly gates Phase 3 decisions.

## Deviations

- **[MEDIUM]** docker-compose.yml maps Radicale to host port 5233 (5233:5232), but the spec AC and verify-radicale.sh both expect port 5232. The script would not successfully connect to the Radicale instance started by docker-compose as-shipped.
  - Justification: Journal reports a successful round-trip, suggesting verification was run with the port aligned at some point. The mismatch may have been introduced during a subsequent cleanup commit or was a deliberate choice to avoid conflicts with a local Radicale. No documentation explains the change. The inconsistency between docker-compose.yml and verify-radicale.sh is a functional issue that could break Phase 5 integration tests if not resolved.
- **[LOW]** The `icalendar` crate (listed in the spec task description as a dependency candidate to be evaluated in Phase 0) is absent from Cargo.toml. No ADR documents the decision to drop or defer it.
  - Justification: The spec language is 'icalendar crate (evaluated in Phase 0)', implying the evaluation could result in rejection. A `toml` crate was added in its place (not mentioned in the spec), appropriate for configuration parsing. Without an explicit decision record, the rationale is implicit. Later phases implement iCalendar serialization manually or via another approach, but the Phase 0 ADR gap is a documentation omission rather than a functional problem.

## Test Coverage
**Status:** sufficient

Phase 0 is a scaffolding and research phase; formal automated tests are not required. The tw-behavior-research.sh script functions as an empirical test harness with 13 discrete pass/fail assertions covering all 9 behavioral items. verify-radicale.sh exercises the full CalDAV round-trip. Both scripts are committed to tests/integration/. No unit or integration test failures are reported.

## Code Quality

The research scripts are well-structured with clear section headers, isolated TASKDATA environments, and a summary pass/fail counter. The ADRs are high-quality, actionable, and correctly reference empirical findings. The Cargo.toml dependency selection is reasonable; using rustls-tls instead of native-tls is a sensible choice for cross-platform builds.

- verify-radicale.sh uses set -euo pipefail and hardcodes BASE_URL to localhost:5232, which is inconsistent with docker-compose.yml port 5233. Running the two together as-is would produce a connection-refused error.
- tw-behavior-research.sh uses set -uo pipefail (without -e) — intentional for a research script so individual test failures don't abort the run, but the omission of -e should be noted so future contributors understand this is deliberate.
- tw-behavior-research.sh has a dead-code block (lines 97-101) where a Python parsing attempt is assigned then immediately overwritten by the correct implementation on line 103. The leftover block should be removed.

## Documentation
**Status:** adequate

docs/adr/tw-field-clearing.md and docs/adr/loop-prevention.md are thorough and production-quality. Both include motivation, empirical evidence, decision tables, implications for sync design, and cross-references. The only gap is the absence of an ADR or inline comment explaining why the `icalendar` crate was not added to Cargo.toml despite being listed as a Phase 0 evaluation candidate.

## Issues

- Port mismatch: docker-compose.yml exposes Radicale on host port 5233, but verify-radicale.sh and spec AC target port 5232 — scripts are mutually incompatible as committed.
- Missing `icalendar` crate from Cargo.toml with no documented rationale for its omission after Phase 0 evaluation.

## Recommendations

- Align docker-compose.yml and verify-radicale.sh on a single port. If 5232 is preferred (per spec AC), change `5233:5232` to `5232:5232` in docker-compose.yml. If 5233 is preferred (e.g., to avoid conflict with local Radicale), update verify-radicale.sh BASE_URL and add a comment explaining the choice.
- Add a brief ADR entry or inline Cargo.toml comment documenting the decision to omit `icalendar` (e.g., 'evaluated in Phase 0; iCalendar parsing implemented manually in Phase 1 to avoid crate limitations').
- Remove the dead-code block in tw-behavior-research.sh (lines 97-101) to reduce confusion for future contributors.
- Consider adding a `# no -e: individual test failures must not abort the research run` comment to tw-behavior-research.sh to clarify the intentional set -uo pipefail choice.

---
*Generated by Foundry MCP Fidelity Review*