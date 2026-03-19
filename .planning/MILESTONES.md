# Milestones

## v1.0 Caldawarrior Hardening (Shipped: 2026-03-19)

**Phases completed:** 7 phases, 15 plans
**Timeline:** 21 days (2026-02-26 → 2026-03-19)
**Rust LOC:** 8,400

**Key accomplishments:**
- Fixed CATEGORIES comma-escaping, XML parser (quick-xml NsReader), ETag normalization, and error context
- Proved dependency relations (DEPENDS-ON/blocks) work end-to-end with real Radicale, including cycle detection
- Verified all 10 mapped fields create/update/clear/round-trip correctly with idempotent sync (80 RF E2E tests)
- Handled DATE-only values, DST timezone ambiguity, and X-property preservation across sync cycles
- CI pipeline (lint/test/e2e/audit) and tag-triggered binary releases via GitHub Actions
- Full README with installation, config reference, scheduling, compatibility matrix, and 15 known limitations
- Reverted tw.update() from task import to task modify with tag/annotation diff — fixed caldavuid UDA persistence

**Tech debt accepted:**
- CATALOG.md missing S-96–S-100 entries (tests pass, docs gap)
- --fail-fast behavioral path has no E2E test
- CHANGELOG.md date placeholder needs replacement before tagging
- Nyquist validation scaffolds present but not complete for any phase

**Audit:** `.planning/milestones/v1.0-MILESTONE-AUDIT.md`
**Archive:** `.planning/milestones/v1.0-ROADMAP.md`, `.planning/milestones/v1.0-REQUIREMENTS.md`

---
