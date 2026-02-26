#!/usr/bin/env bash
# Verify Radicale CalDAV server: VTODO PUT/REPORT round-trip
set -euo pipefail

BASE_URL="http://localhost:5232"
CALENDAR_PATH="/test-user/test-calendar/"
VTODO_PATH="${CALENDAR_PATH}test-task-1.ics"
TASK_UID="test-task-$(date +%s)"

echo "=== Starting Radicale ==="
docker compose up -d
echo "Waiting for Radicale to be ready..."
for i in $(seq 1 20); do
  if curl -sf "${BASE_URL}" >/dev/null 2>&1; then
    echo "Radicale is ready (attempt ${i})"
    break
  fi
  if [ "$i" -eq 20 ]; then
    echo "ERROR: Radicale did not become ready in time"
    docker compose logs radicale
    docker compose down
    exit 1
  fi
  sleep 1
done

echo ""
echo "=== MKCOL: Create principal collection ==="
curl -sv -X MKCOL "${BASE_URL}/test-user/" \
  -H "Content-Type: application/xml; charset=utf-8" \
  --data '<?xml version="1.0" encoding="utf-8"?>
<mkcol xmlns="DAV:">
  <set><prop>
    <resourcetype><collection/></resourcetype>
  </prop></set>
</mkcol>'

echo ""
echo "=== MKCOL: Create calendar ==="
curl -sv -X MKCOL "${BASE_URL}${CALENDAR_PATH}" \
  -H "Content-Type: application/xml; charset=utf-8" \
  --data '<?xml version="1.0" encoding="utf-8"?>
<mkcol xmlns="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <set><prop>
    <resourcetype><collection/><C:calendar/></resourcetype>
    <displayname>Test Calendar</displayname>
  </prop></set>
</mkcol>'

echo ""
echo "=== PUT: Create VTODO ==="
VTODO_UID="${TASK_UID}"
curl -sv -X PUT "${BASE_URL}${VTODO_PATH}" \
  -H "Content-Type: text/calendar; charset=utf-8" \
  --data "BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//caldawarrior//test//EN
BEGIN:VTODO
UID:${VTODO_UID}
SUMMARY:Test Task from caldawarrior
STATUS:NEEDS-ACTION
DTSTAMP:$(date -u +%Y%m%dT%H%M%SZ)
DTSTART:$(date -u +%Y%m%dT%H%M%SZ)
END:VTODO
END:VCALENDAR"

echo ""
echo "=== REPORT: Retrieve VTODO ==="
REPORT_RESPONSE=$(curl -sv -X REPORT "${BASE_URL}${CALENDAR_PATH}" \
  -H "Content-Type: application/xml; charset=utf-8" \
  -H "Depth: 1" \
  --data '<?xml version="1.0" encoding="utf-8"?>
<C:calendar-query xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop>
    <D:getetag/>
    <C:calendar-data/>
  </D:prop>
  <C:filter>
    <C:comp-filter name="VCALENDAR">
      <C:comp-filter name="VTODO"/>
    </C:comp-filter>
  </C:filter>
</C:calendar-query>')

echo ""
echo "=== REPORT response ==="
echo "${REPORT_RESPONSE}"

echo ""
echo "=== Verifying UID in response ==="
if echo "${REPORT_RESPONSE}" | grep -q "${VTODO_UID}"; then
  echo "PASS: UID '${VTODO_UID}' found in REPORT response — VTODO round-trip verified"
else
  echo "FAIL: UID '${VTODO_UID}' not found in REPORT response"
  docker compose down
  exit 1
fi

echo ""
echo "=== Stopping Radicale ==="
docker compose down

echo ""
echo "=== All checks passed ==="
