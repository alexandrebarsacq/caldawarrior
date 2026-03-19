# tasks.org / DAVx5 Compatibility with DEPENDS-ON Relations

## Summary

caldawarrior maps TaskWarrior `depends` to the CalDAV `RELATED-TO;RELTYPE=DEPENDS-ON`
property (defined in [RFC 9253](https://datatracker.ietf.org/doc/html/rfc9253), published 2022).
This property **survives round-trips through Radicale** (file-based storage preserves
all iCalendar properties verbatim) but is **not rendered as a dependency in tasks.org**.

## RELATED-TO RELTYPE Support Matrix

| Component | PARENT (RFC 5545) | CHILD (RFC 5545) | DEPENDS-ON (RFC 9253) |
|-----------|-------------------|-------------------|-----------------------|
| Radicale (server) | Preserved | Preserved | Preserved |
| DAVx5 (sync proxy) | Pass-through | Pass-through | Pass-through |
| tasks.org (client) | Rendered as subtask hierarchy | Rendered | Not rendered (invisible) |
| jtx Board (client) | Rendered | Rendered | Unconfirmed |

## Detailed Findings

### Radicale

Radicale uses file-based storage, writing each VTODO as a `.ics` file verbatim.
All iCalendar properties, including `RELATED-TO;RELTYPE=DEPENDS-ON`, are preserved
exactly as written. **Confidence: HIGH** (architecture guarantees preservation).

### DAVx5

DAVx5 acts as a transparent CalDAV sync proxy between the server and Android task
apps. It does not interpret VTODO semantics — it passes through raw VCALENDAR data
to the task provider (tasks.org, jtx Board, OpenTasks). RELATED-TO properties of
any RELTYPE should survive DAVx5 sync. **Confidence: MEDIUM** (based on
[DAVx5 documentation](https://manual.davx5.com/tasks_notes.html)).

### tasks.org

tasks.org implements CalDAV sync for VTODOs but only uses `RELATED-TO;RELTYPE=PARENT`
for its subtask hierarchy feature. Key observations:

- **Does NOT use RELTYPE=DEPENDS-ON.** DEPENDS-ON is defined in RFC 9253 (2022), a
  relatively recent extension. tasks.org implements parent-child relationships only.
- **Subtask sync has known bugs.** See [tasks/tasks#3023](https://github.com/tasks/tasks/issues/3023):
  tasks initially import correctly but PUT requests can break the PARENT hierarchy.
- **Behavior with unknown RELTYPE values is undocumented.** tasks.org likely preserves
  the raw iCalendar property (since it stores the full iCal text) but does not render
  DEPENDS-ON relationships in the UI.

**Confidence: MEDIUM** (based on issue tracker and documentation analysis, not
source code audit of [tasks/tasks](https://github.com/tasks/tasks)).

### jtx Board

jtx Board explicitly supports RELATED-TO for cross-linking tasks, notes, and journals.
It is the most standards-compliant of the Android task apps. Support for
RELTYPE=DEPENDS-ON specifically has not been confirmed. **Confidence: LOW**.

## Practical Impact

For caldawarrior users:

1. **Dependencies work reliably between TaskWarrior instances** syncing through
   any CalDAV server that preserves iCalendar properties (Radicale, Nextcloud, Baikal).
2. **Dependencies are invisible in tasks.org.** The RELATED-TO;RELTYPE=DEPENDS-ON
   property is stored on the server and survives sync, but tasks.org does not display
   it. This is expected behavior, not a bug — DEPENDS-ON is a newer RFC extension.
3. **Dependencies are not corrupted by tasks.org.** Since tasks.org preserves unknown
   properties, using tasks.org alongside caldawarrior should not strip DEPENDS-ON
   relations (though this is based on expected behavior, not verified with a physical device).

## References

- [RFC 9253 - iCalendar Relationships](https://datatracker.ietf.org/doc/html/rfc9253)
  — Defines DEPENDS-ON, REFID, and LINK RELTYPE values
- [RFC 5545 - iCalendar](https://datatracker.ietf.org/doc/rfc5545/)
  — Original RELATED-TO with PARENT/CHILD/SIBLING
- [tasks/tasks#3023](https://github.com/tasks/tasks/issues/3023)
  — RELATED-TO handling issues in tasks.org CalDAV sync
- [DAVx5 Tasks & Notes documentation](https://manual.davx5.com/tasks_notes.html)
  — Transparent VTODO sync behavior

---
*Document created: Phase 2 (Relation Verification)*
*Research confidence: MEDIUM overall*
*Last updated: 2026-03-19*
