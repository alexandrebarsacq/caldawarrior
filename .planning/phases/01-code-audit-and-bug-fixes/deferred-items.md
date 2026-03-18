# Deferred Items -- Phase 01

## Pre-existing: caldav_adapter.rs normalize_etag compile error

**Found during:** Plan 01-01, Task 1 verification
**Issue:** `cargo test --lib` (unfiltered) fails to compile because `src/caldav_adapter.rs` tests reference a function `normalize_etag` that does not exist yet. These tests were added by commit `ba1d317` (plan 01-02 TDD RED phase) and are intended to fail, but they prevent compilation of the full test binary.
**Impact:** Cannot run `cargo test --lib` without a test filter. Filtered runs (e.g. `cargo test --lib ical::tests`) work fine.
**Resolution:** Plan 01-02 will implement `normalize_etag` during its GREEN phase. No action needed here.
