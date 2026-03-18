# Domain Pitfalls

**Domain:** CalDAV/VTODO bidirectional sync tool (TaskWarrior <-> CalDAV)
**Project:** caldawarrior
**Researched:** 2026-03-18

## Critical Pitfalls

Mistakes that cause data loss, sync corruption, or require significant rework.

### Pitfall 1: CATEGORIES Escaping vs Splitting Mismatch

**What goes wrong:** CATEGORIES values are comma-separated per RFC 5545, but commas *inside* a category name must be backslash-escaped (`\,`). The current parser in `ical.rs` splits on raw commas without unescaping first. A tag like `"work,life"` (with a literal comma in the tag name) would be split into two separate categories `"work"` and `"life"` instead of preserved as one. Conversely, when serializing, escaped commas in tag names are not emitted.

**Why it happens:** The split-then-trim logic at line 69 of `ical.rs` calls `value.split(',')` without first checking for backslash-escaped commas. This is a common iCalendar parsing mistake because most real-world tags do not contain commas, so the bug goes undetected.

**Consequences:** Tag corruption on round-trip. A tag containing a comma would be silently split into multiple tags, and those tags would never recombine. Over multiple sync cycles, this creates phantom tags.

**Prevention:**
- Parse CATEGORIES using the same TEXT value unescaping logic (handle `\,` as literal comma, `\;` as literal semicolon, `\\` as backslash) *before* splitting on unescaped commas.
- Add unit tests with category names containing commas, semicolons, and backslashes.
- Serialize categories with proper escaping: commas in tag names must be emitted as `\,`.

**Detection:** Unit test that round-trips a VTODO with `CATEGORIES:work\,life,personal` and verifies two categories: `"work,life"` and `"personal"`.

**Phase relevance:** Code audit / field mapping verification phase.

**Confidence:** HIGH -- directly observed in codebase (`ical.rs` lines 68-75) and verified against RFC 5545 Section 3.8.1.2.

---

### Pitfall 2: Sync Loop From Write-Back Timestamp Bumps

**What goes wrong:** When TW wins LWW and caldawarrior writes to CalDAV, the server updates `LAST-MODIFIED` / ETag. On the next sync, CalDAV's `LAST-MODIFIED` is now *newer* than TW's `modified`, so CalDAV appears to win -- triggering a write back to TW that bumps TW's `modified`, which then triggers another CalDAV write on the *next* sync, ad infinitum.

**Why it happens:** Any write to either system updates its modification timestamp. Without content-identical detection (Layer 2 in LWW), timestamp-only comparison creates an infinite ping-pong loop.

**Consequences:** Every sync run triggers writes to both systems. Tasks are never quiescent. Performance degrades. In extreme cases with many tasks, sync runs take minutes instead of seconds.

**Prevention:**
- The existing Layer 2 content-identical check in `lww.rs` is the correct prevention mechanism. It compares 8 tracked fields and returns `Skip(Identical)` when content matches, regardless of timestamps.
- **The pitfall is that new fields added during hardening (e.g., PERCENT-COMPLETE, CATEGORIES-as-tags, additional annotations) MUST be added to the content-identical check.** If a field is synced but not compared in Layer 2, it will cause sync loops for that field.
- Every new synced field requires a corresponding entry in `content_identical()`.

**Detection:** Integration test that runs two consecutive syncs after a write and verifies the second sync produces zero writes. Currently covered for basic fields; must be extended for every new field.

**Phase relevance:** Every phase that adds new field mappings.

**Confidence:** HIGH -- the mechanism exists and is tested; the risk is incomplete coverage when adding fields.

---

### Pitfall 3: ETag Double-Quoting / Misquoting Causes 412 Loops

**What goes wrong:** HTTP ETags MUST be quoted strings per RFC 7232 (e.g., `"abc123"`), but some servers return them unquoted, some include the quotes in the value, and some return weak ETags (`W/"abc123"`). If the client stores the ETag with one quoting convention and sends `If-Match` with another, the server returns 412 Precondition Failed even though the resource has not changed.

**Why it happens:** The current code in `caldav_adapter.rs` (line 176) does `format!("\"{}\"", e.trim_matches('"'))` -- it strips quotes then re-adds them. This handles the common case but can fail with:
- Weak ETags (`W/"abc123"`) -- stripping `"` from this produces `W/abc123`, then re-quoting produces `"W/abc123"` which is wrong.
- ETags containing literal quote characters (rare but spec-legal).
- Servers that return unquoted ETags without quotes (some CalDAV servers do this).

**Consequences:** Perpetual 412 errors on updates. The retry logic re-fetches but gets the same ETag format, so it retries indefinitely (up to `MAX_ETAG_RETRIES = 3`, then fails).

**Prevention:**
- Normalize ETags on receipt: strip weak prefix, ensure surrounding quotes.
- Store ETags in a canonical format (always quoted, never weak-prefixed).
- Add unit tests that exercise ETag normalization with: unquoted, double-quoted, weak-prefixed, and empty ETag values.

**Detection:** Unit test for ETag normalization. Integration test with a mock server that returns various ETag formats.

**Phase relevance:** Code audit / CalDAV protocol hardening phase.

**Confidence:** MEDIUM -- observed the quoting logic in code; Radicale specifically returns properly quoted ETags, but this will bite if users connect to Nextcloud, Baikal, or SOGo.

---

### Pitfall 4: RELATED-TO DEPENDS-ON Is RFC 9253, Not RFC 5545

**What goes wrong:** The `DEPENDS-ON` value for the `RELTYPE` parameter is defined in RFC 9253 (published August 2022), not in the original RFC 5545. Many CalDAV clients and servers only recognize the RFC 5545 relationship types: `PARENT`, `CHILD`, `SIBLING`. When caldawarrior emits `RELATED-TO;RELTYPE=DEPENDS-ON:some-uid`, clients that do not understand RFC 9253 may:
- Silently discard the property entirely.
- Store it but not display it.
- Treat it as the default type (`PARENT`), creating incorrect hierarchical relationships.
- Error on the unrecognized RELTYPE value.

**Why it happens:** RFC 9253 is a relatively new standard (2022). Client support is patchy. tasks.org uses `RELATED-TO;RELTYPE=PARENT` for subtasks (the Apple/Nextcloud convention), not `DEPENDS-ON`. Thunderbird has limited VTODO relationship support. Apple Reminders does not surface dependency information.

**Consequences:** Dependencies synced from TaskWarrior appear as parent/child relationships in some clients, are invisible in others, or cause parse errors. Users who edit tasks in tasks.org may unintentionally strip `DEPENDS-ON` relations because tasks.org does not know to preserve them.

**Prevention:**
- **Preserve `extra_props` containing unknown RELATED-TO entries** (the current codebase already does this via the extra_props round-trip mechanism -- verify this works for RELATED-TO specifically).
- Document that dependency visualization requires RFC 9253-aware clients.
- Consider an optional config flag to emit PARENT instead of DEPENDS-ON for clients that don't support RFC 9253 (but this changes semantics; document the tradeoff).
- Test with tasks.org + DAVx5 to verify DEPENDS-ON relations survive round-trip (they may be preserved as opaque data even if not displayed).

**Detection:** E2E test: create task with DEPENDS-ON in CalDAV, modify task in tasks.org, verify DEPENDS-ON is still present after tasks.org saves.

**Phase relevance:** Relations verification / tasks.org compatibility phase.

**Confidence:** HIGH -- RFC 9253 publication confirmed; tasks.org uses PARENT for subtasks per documentation.

---

### Pitfall 5: Orphan CalDAV Tasks After TW Delete Without caldavuid

**What goes wrong:** When a TW task that has already been pushed to CalDAV is deleted via `task delete` (which sets `status=deleted`), the sync engine correctly marks the CalDAV side as CANCELLED. But if the TW task is *purged* (removed from TW's data file entirely via `task purge` or by the GC mechanism), the sync engine has no record of the task. The CalDAV VTODO remains on the server as a ghost -- it has no matching TW task, so on the next sync, it is treated as a "CalDAV-only" entry and *re-imported* into TW as a new task.

**Why it happens:** The stateless design (no sync database, correlation via `caldavuid` UDA) means the only link between TW and CalDAV is the `caldavuid` field on the TW task. If the TW task is purged, the link is severed, and the CalDAV VTODO appears as a new unmatched entry.

**Consequences:** Deleted tasks reappear in TaskWarrior. Users delete a task, it comes back. This is the single most complained-about bug in CalDAV sync tools.

**Prevention:**
- The `completed_cutoff_days` filter already prevents old completed tasks from being re-imported. Verify it also handles `CANCELLED` status VTODOs.
- Consider: CalDAV-only VTODOs with `CANCELLED` status should be skipped (not imported), matching the existing skip for `COMPLETED`.
- Consider: CalDAV-only VTODOs with `COMPLETED` status and a COMPLETED timestamp older than cutoff should be skipped.
- Document that `task purge` can cause ghost tasks and recommend against it (or suggest running a CalDAV cleanup).
- The current IR construction in `ir.rs` (lines 179-181) skips `tw_uuid` assignment for COMPLETED/CANCELLED CalDAV-only entries, which means they produce `Skip` ops. Verify this path works end-to-end.

**Detection:** E2E test: push task to CalDAV, delete in TW (not purge), sync, verify CANCELLED on CalDAV. Then: remove the TW task entirely (simulate purge), sync again, verify no re-import.

**Phase relevance:** Sync logic audit / ghost task prevention phase.

**Confidence:** HIGH -- this is a well-documented problem in bidirectional sync tools (Outlook CalDav Synchronizer, DAVx5 FAQ, python-caldav issues all discuss this pattern).

---

### Pitfall 6: XML Parsing Without a Real XML Parser

**What goes wrong:** The CalDAV REPORT response parser in `caldav_adapter.rs` uses string-based tag matching (`find("<D:response>")`) instead of a proper XML parser. This breaks when:
- The server uses a different namespace prefix (e.g., `<ns0:response>`, `<d:response>`, or no prefix at all with default namespace).
- The XML contains CDATA sections around calendar-data.
- Tags have attributes (e.g., `<D:response xmlns:D="DAV:">`).
- The calendar-data content contains XML-escaped characters (`&lt;`, `&amp;`).
- The server returns compact XML without whitespace between tags.

**Why it happens:** The string-based approach was a deliberate choice to avoid pulling in an XML parsing dependency. It works with Radicale's specific output format but is fragile against other servers.

**Consequences:** Silent data loss -- VTODOs are not parsed from the response, so they appear as "missing" on the CalDAV side. The sync engine treats them as deleted-on-server, potentially triggering re-pushes or orphan logic.

**Prevention:**
- Replace string-based XML parsing with `quick-xml` or `roxmltree` crate (both are lightweight, zero-copy parsers).
- At minimum, handle namespace prefix variations and XML entity decoding.
- Test against real REPORT responses from Radicale, Nextcloud, Baikal, and SOGo.

**Detection:** Unit test with REPORT XML using different namespace prefixes, CDATA calendar-data, and XML entities.

**Phase relevance:** CalDAV protocol hardening phase. This is the highest-risk technical debt in the current codebase.

**Confidence:** HIGH -- directly observed in `caldav_adapter.rs` lines 317-414; the code only handles `D:` and bare prefixes.

---

## Moderate Pitfalls

### Pitfall 7: Floating Datetime Treated as UTC Silently

**What goes wrong:** The iCalendar parser in `ical.rs` (line 447) treats floating datetimes (no `Z` suffix, no `TZID` parameter) as UTC. Per RFC 5545, floating time means "local time of the user" and is explicitly *not* UTC. If a CalDAV client (like tasks.org) emits a DUE date as floating time (e.g., `DUE:20260315T090000`), caldawarrior interprets it as 09:00 UTC, which may be hours off from the user's actual local time.

**Prevention:**
- Log a warning when a floating datetime is encountered (not a VTODO date-only value).
- Consider adding a config option for the user's default timezone, applied to floating datetimes.
- Date-only values (`YYYYMMDD` without `T`) are correctly treated as midnight UTC and are less problematic.

**Phase relevance:** Field mapping verification phase.

**Confidence:** HIGH -- observed in `ical.rs` line 447; RFC 5545 Section 3.3.5 explicitly defines floating time semantics.

---

### Pitfall 8: VTIMEZONE Components Not Preserved on Round-Trip

**What goes wrong:** When caldawarrior serializes a VTODO back to CalDAV (in `to_icalendar_string`), it always emits UTC timestamps (`Z` suffix) and never includes a VTIMEZONE component. If the original VCALENDAR contained a VTIMEZONE block (common when tasks.org or Thunderbird creates VTODOs with timezoned dates), that VTIMEZONE is stripped on write-back. Some picky clients may reject VTODOs that reference a TZID without a corresponding VTIMEZONE block -- but since caldawarrior converts everything to UTC, the TZID references are also removed, so this is safe *as long as* all datetimes are consistently UTC.

**Prevention:**
- Verify that the current approach (convert to UTC, emit Z suffix) is consistent across all datetime fields.
- If DTSTART/DUE are emitted without Z suffix by accident, clients with VTIMEZONE expectations will misinterpret them.
- Add a round-trip test: parse VTODO with TZID datetimes, serialize, verify all datetimes have Z suffix.

**Phase relevance:** RFC compliance verification phase.

**Confidence:** HIGH -- observed in serializer; approach is correct but must be verified exhaustively.

---

### Pitfall 9: tasks.org Manual Sort Order Destroyed by Sync

**What goes wrong:** tasks.org uses `X-APPLE-SORT-ORDER` (a non-standard extension) to persist manual task ordering. This property is stored in the VTODO and synced via CalDAV. The current codebase preserves unknown properties via `extra_props`, so X-APPLE-SORT-ORDER should survive round-trips. However, when caldawarrior *creates* a new VTODO from a TW task (TW-only push), no X-APPLE-SORT-ORDER is emitted. When tasks.org encounters such a VTODO, it may assign a default sort position, disrupting the user's manually ordered list.

**Prevention:**
- Verify that `extra_props` preservation works for X-APPLE-SORT-ORDER (test: fetch VTODO with this property, modify a different field, write back, verify property survives).
- For new tasks pushed from TW, accept that no sort order is set -- this is inherent and not fixable without TW-side metadata.
- Document that manual sort order in tasks.org may be disrupted when tasks are created from the TW side.

**Phase relevance:** tasks.org compatibility verification phase.

**Confidence:** MEDIUM -- based on tasks.org documentation confirming X-APPLE-SORT-ORDER usage; extra_props preservation logic is implemented but untested for this specific property.

---

### Pitfall 10: PERCENT-COMPLETE vs STATUS Mismatch Across Clients

**What goes wrong:** RFC 5545 defines both `STATUS` (NEEDS-ACTION/IN-PROCESS/COMPLETED) and `PERCENT-COMPLETE` (0-100) for VTODOs. Some clients set only PERCENT-COMPLETE=100 without setting STATUS=COMPLETED (ownCloud Tasks had this bug). Others set STATUS=COMPLETED without PERCENT-COMPLETE. caldawarrior currently maps status but ignores PERCENT-COMPLETE entirely.

**Consequences:** A task marked 100% complete in tasks.org but without STATUS:COMPLETED would appear as "pending" in TaskWarrior. Conversely, marking a task complete in TW sets STATUS:COMPLETED but not PERCENT-COMPLETE, so some clients may show it as incomplete.

**Prevention:**
- When reading: treat PERCENT-COMPLETE=100 as equivalent to COMPLETED, even if STATUS is absent or NEEDS-ACTION.
- When writing COMPLETED status: also emit PERCENT-COMPLETE:100.
- When writing NEEDS-ACTION status: also emit PERCENT-COMPLETE:0 (or omit it).
- Test with tasks.org to verify which combination it uses.

**Phase relevance:** tasks.org compatibility verification phase.

**Confidence:** MEDIUM -- based on ownCloud Tasks bug reports and Outlook CalDav Synchronizer handling; tasks.org's specific behavior needs testing.

---

### Pitfall 11: TW `task import` Fails Silently With Extra JSON Fields

**What goes wrong:** When pulling a CalDAV task into TW, the adapter builds a JSON object and pipes it through `task import`. If the JSON contains fields that TW does not recognize (typos, version mismatches), TW silently ignores them rather than erroring. More critically, TW 3.x changed the JSON format for some fields (e.g., `depends` can be a string or array), and `task import` may reject or misparse the wrong format.

**Prevention:**
- The existing `tw_depends` module handles the string/array duality correctly.
- Test `task import` with the exact JSON format the adapter produces, on the pinned TW 3.x version.
- Pin TW version in Docker tests and document minimum TW version.

**Phase relevance:** Testing hardening phase.

**Confidence:** MEDIUM -- `tw_depends` handles this, but other fields may have version-dependent quirks.

---

### Pitfall 12: Multi-Byte UTF-8 Characters Split Across Line Fold Boundary

**What goes wrong:** RFC 5545 requires line folding at 75 *octets* (bytes), not 75 characters. A multi-byte UTF-8 character (e.g., emoji, CJK characters) might straddle the 75-byte boundary. If the fold point lands inside a multi-byte sequence, the fold creates an invalid UTF-8 sequence that causes parsing errors on the other side.

**Current status:** The `fold_line` function in `ical.rs` (line 382-385) correctly walks backward to find a UTF-8 character boundary (`while !remaining.is_char_boundary(cut)`). This is well-implemented.

**Remaining risk:** The `unfold_lines` function must correctly rejoin sequences that were split at a character boundary by other (buggy) implementations that DO split inside a multi-byte sequence. The current unfold implementation works at the character level (Rust's `chars()` iterator), so it cannot process invalid UTF-8 at all -- it would panic or produce replacement characters.

**Prevention:**
- The fold implementation is correct. Verify with a test containing CJK/emoji characters that cross the 75-byte boundary.
- For unfold: if interoperating with buggy servers that split mid-sequence, consider byte-level unfolding before UTF-8 validation. This is an edge case unlikely to occur with Radicale but possible with other servers.

**Phase relevance:** RFC compliance verification phase.

**Confidence:** HIGH -- fold logic is correctly implemented (verified in code). Unfold risk is LOW for Radicale, MEDIUM for other servers.

---

## Minor Pitfalls

### Pitfall 13: Docker Build Cache Invalidation on Any Source Change

**What goes wrong:** The Dockerfile copies `src/` in one layer. Any change to any source file invalidates the cache for the entire `cargo build` step. With Rust's compilation times, this means rebuilding all dependencies from scratch on every code change during development.

**Prevention:**
- Use a two-stage dependency caching strategy: first copy only `Cargo.toml` + `Cargo.lock`, run `cargo build` (to cache dependencies), then copy `src/` and build again.
- The current Dockerfile uses `--mount=type=cache` for the cargo registry and target directories, which partially mitigates this. Verify it works correctly.

**Phase relevance:** Docker/packaging phase.

**Confidence:** HIGH -- standard Docker/Rust pattern; the current Dockerfile partially addresses this.

---

### Pitfall 14: Archlinux Rolling Base Image Breaks TW Version

**What goes wrong:** The test Dockerfile uses `archlinux:base` (rolling release) to get TaskWarrior 3.x. A `pacman -Syu` during build could upgrade TW to a version with breaking changes in the `task export` / `task import` JSON format, causing tests to fail without any code changes.

**Prevention:**
- Pin TW version in Dockerfile: `pacman -S --noconfirm task=3.x.y-z` (specific version).
- Or: use a Dockerfile build arg for the TW version and document the tested version.
- Add a test that checks `task --version` output and fails early with a clear message if the version is unexpected.

**Phase relevance:** Docker/packaging phase.

**Confidence:** HIGH -- rolling release distros are well-known for this problem.

---

### Pitfall 15: Robot Framework E2E Tests Sensitive to Radicale Timing

**What goes wrong:** E2E tests that create a task in TW, sync to CalDAV, then immediately read from CalDAV may hit a race condition if Radicale has not finished writing to disk. Similarly, rapid consecutive syncs (create-sync-modify-sync-verify) may see stale ETag values if the test outpaces Radicale's response cycle.

**Prevention:**
- The Docker Compose healthcheck ensures Radicale is up before tests start.
- Add small `Sleep` keywords (0.5-1s) between sync operations if flakiness is observed. Prefer explicit retries with assertion over blind sleeps.
- Use Robot Framework's `Wait Until Keyword Succeeds` for assertions that depend on CalDAV state.
- Run tests sequentially (not in parallel) to avoid cross-test interference via shared Radicale state.

**Phase relevance:** Testing hardening phase.

**Confidence:** MEDIUM -- no flakiness reported yet, but this is a common pattern in Docker-based integration tests.

---

### Pitfall 16: DESCRIPTION Field Round-Trip With Multiple Annotations

**What goes wrong:** The current field mapping uses only the first TW annotation for the CalDAV DESCRIPTION field (slot 0 invariant). If a CalDAV client adds a long DESCRIPTION with multiple paragraphs, only the first line/annotation-slot survives the round-trip to TW and back. Worse, if a user adds annotations in TW (slots 1+), and then the CalDAV DESCRIPTION changes, the merge logic must preserve slots 1+ while replacing slot 0.

**Current status:** The `merge_annotations` function in `writeback.rs` implements the slot invariant correctly. The risk is edge cases:
- CalDAV DESCRIPTION is None + TW has annotations: annotations are preserved.
- CalDAV DESCRIPTION matches slot 0: no-op.
- CalDAV DESCRIPTION differs from slot 0: slot 0 replaced, slots 1+ preserved.

**Prevention:**
- Test the combinatorial cases exhaustively: empty/non-empty DESCRIPTION x 0/1/many TW annotations.
- Verify that `content_identical` in `lww.rs` compares only slot 0 (the CalDAV-owned slot), not all annotations.

**Phase relevance:** Field mapping verification phase.

**Confidence:** HIGH -- logic is implemented; coverage of edge cases needs verification.

---

### Pitfall 17: Static Binary Linking for Distribution

**What goes wrong:** The project uses `reqwest` with `rustls-tls` feature (not OpenSSL), which is good for static linking. However, if any transitive dependency pulls in a C library (e.g., `libz-sys` for compression), static builds for musl targets will fail unless the C library is also statically linked.

**Prevention:**
- Verify that `cargo build --release --target x86_64-unknown-linux-musl` succeeds.
- Use `ldd target/release/caldawarrior` to verify no dynamic library dependencies.
- If musl builds fail, use the `muslrust` Docker image for compilation.
- The existing Dockerfile uses `rust:1.85-bookworm` (glibc), which is fine for the Docker container but means the binary is not portable to alpine/musl-based systems.

**Phase relevance:** Binary release packaging phase.

**Confidence:** MEDIUM -- `rustls-tls` avoids the OpenSSL pitfall, but transitive deps need verification.

---

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|---|---|---|
| Code audit / field mapping | CATEGORIES escaping (#1), floating datetime (#7), PERCENT-COMPLETE (#10) | Add field-specific unit tests before modifying mapper |
| Sync logic audit | Sync loops (#2), ghost tasks (#5) | Run consecutive-sync integration tests for every field change |
| Relations verification | DEPENDS-ON client compatibility (#4) | Test with real tasks.org + DAVx5 + Radicale |
| CalDAV protocol hardening | XML parsing fragility (#6), ETag quoting (#3) | Replace string XML parser; add ETag normalization |
| tasks.org compatibility | X-APPLE-SORT-ORDER (#9), PERCENT-COMPLETE (#10), DEPENDS-ON (#4) | Test extra_props round-trip with real tasks.org data |
| Docker / packaging | Cache invalidation (#13), rolling image (#14), static linking (#17) | Pin versions, two-stage build |
| Testing hardening | Radicale timing (#15), annotation edge cases (#16) | Use retry-based assertions, exhaustive combinatorial tests |
| RFC compliance | VTIMEZONE stripping (#8), UTF-8 folding (#12), floating time (#7) | Round-trip tests with timezone-aware and multi-byte data |

## Sources

- [RFC 5545 - iCalendar Specification](https://www.rfc-editor.org/rfc/rfc5545) - VTODO, TEXT escaping, line folding, CATEGORIES, RELATED-TO
- [RFC 9253 - Support for iCalendar Relationships](https://datatracker.ietf.org/doc/html/rfc9253) - DEPENDS-ON relationship type definition
- [RFC 4791 - CalDAV](https://www.ietf.org/rfc/rfc4791.txt) - REPORT, calendar-query, ETag usage
- [iCalendar.org - CATEGORIES](https://icalendar.org/iCalendar-RFC-5545/3-8-1-2-categories.html) - Multi-value escaping rules
- [iCalendar.org - Content Lines](https://icalendar.org/iCalendar-RFC-5545/3-1-content-lines.html) - 75-octet folding, UTF-8 multi-byte
- [DAVx5 FAQ - Advanced Task Features](https://www.davx5.com/faq/tasks/advanced-task-features) - Task app compatibility matrix
- [Tasks.org Manual Sort Mode](https://tasks.org/docs/manual_sort_mode/) - X-APPLE-SORT-ORDER usage
- [ownCloud Tasks Issue #137](https://github.com/owncloud/tasks/issues/137) - STATUS:COMPLETED missing bug
- [Evert Pot - Escaping in iCalendar and vCard](https://evertpot.com/escaping-in-vcards-and-icalendar/) - TEXT escaping pitfalls
- [CalConnect Developer Guide](https://devguide.calconnect.org/CalDAV/building-a-caldav-client/) - CalDAV client implementation guide
- [Raniz Blog - Rust MUSL Performance](https://raniz.blog/2025-02-06_rust-musl-malloc/) - musl memory allocator issues
- [clux/muslrust](https://github.com/clux/muslrust) - Docker environment for static Rust binaries
- [Outlook CalDav Synchronizer](https://github.com/aluxnimm/outlookcaldavsynchronizer) - Sync loop prevention patterns, ETag handling
- [CalDAV Sync Process](https://icalendar.org/CalDAV-Access-RFC-4791/8-2-1-3-synchronization-process.html) - URI/ETag-based synchronization
- Codebase analysis: `ical.rs`, `caldav_adapter.rs`, `sync/lww.rs`, `sync/writeback.rs`, `mapper/fields.rs`
