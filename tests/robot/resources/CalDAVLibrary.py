import os
import xml.etree.ElementTree as ET
from datetime import datetime, timedelta, timezone

import requests
from icalendar import Calendar, vDatetime


class CalDAVLibrary:
    """Robot Framework keyword library for CalDAV operations against Radicale.

    Reads connection settings from environment variables:
        RADICALE_URL: Base URL of the Radicale server (e.g. http://radicale:5232)
        RADICALE_USER: Username for HTTP Basic Auth
        RADICALE_PASSWORD: Password for HTTP Basic Auth
    """

    ROBOT_LIBRARY_SCOPE = 'SUITE'

    def __init__(self):
        self._base_url = os.environ['RADICALE_URL'].rstrip('/')
        self._user = os.environ['RADICALE_USER']
        self._password = os.environ['RADICALE_PASSWORD']
        self._session = requests.Session()
        self._session.auth = (self._user, self._password)

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _check_response(self, response, acceptable_statuses=None):
        """Raise AssertionError if the response status is not acceptable.

        Args:
            response: The requests.Response object to check.
            acceptable_statuses: Optional iterable of additional acceptable HTTP
                status codes beyond the 2xx range.

        Raises:
            AssertionError: When the response status is not 2xx and not in
                acceptable_statuses.
        """
        if acceptable_statuses is None:
            acceptable_statuses = set()
        if response.status_code in acceptable_statuses:
            return
        if not (200 <= response.status_code < 300):
            raise AssertionError(
                f"HTTP request to {response.url} failed with status "
                f"{response.status_code}: {response.text!r}"
            )

    # ------------------------------------------------------------------
    # Collection management
    # ------------------------------------------------------------------

    def create_collection(self, name):
        """Create a CalDAV collection (calendar) on the server.

        Args:
            name: String used as the path segment for the new collection.

        Returns:
            The full collection URL as a string
            (e.g. http://radicale:5232/user/name/).

        Raises:
            AssertionError: On non-2xx HTTP response.
        """
        url = f"{self._base_url}/{self._user}/{name}/"
        response = self._session.request('MKCALENDAR', url)
        self._check_response(response)
        return url

    def delete_collection(self, collection_url):
        """Delete a CalDAV collection from the server.

        Args:
            collection_url: Full URL of the collection to delete.

        Raises:
            AssertionError: On non-2xx response (404 is treated as acceptable
                because the collection may already be absent).
        """
        response = self._session.delete(collection_url)
        self._check_response(response, acceptable_statuses={404})

    def clear_vtodos(self, collection_url):
        """Delete all VTODO resources inside a collection, leaving the collection itself.

        Sends a PROPFIND Depth:1 to list all .ics hrefs, then issues a DELETE
        for each one.  404 responses are silently ignored (race-safe).

        Args:
            collection_url: Full URL of the CalDAV collection.

        Raises:
            AssertionError: On non-2xx PROPFIND response or non-2xx/404 DELETE.
        """
        propfind_body = (
            '<?xml version="1.0" encoding="utf-8"?>'
            '<D:propfind xmlns:D="DAV:">'
            '<D:prop><D:resourcetype/></D:prop>'
            '</D:propfind>'
        )
        response = self._session.request(
            'PROPFIND',
            collection_url,
            data=propfind_body,
            headers={
                'Depth': '1',
                'Content-Type': 'application/xml; charset=utf-8',
            },
        )
        self._check_response(response)
        root = ET.fromstring(response.text)
        for href_elem in root.iter('{DAV:}href'):
            href = href_elem.text or ''
            if href.endswith('.ics'):
                delete_url = self._base_url + href
                del_resp = self._session.delete(delete_url)
                self._check_response(del_resp, acceptable_statuses={404})

    # ------------------------------------------------------------------
    # VTODO CRUD
    # ------------------------------------------------------------------

    def count_vtodos(self, collection_url):
        """Return the number of VTODO resources inside a collection.

        Sends a PROPFIND with Depth: 1 and counts .ics hrefs in the
        multistatus response.

        Args:
            collection_url: Full URL of the CalDAV collection.

        Returns:
            Integer count of .ics resources in the collection.

        Raises:
            AssertionError: On non-2xx HTTP response.
        """
        propfind_body = (
            '<?xml version="1.0" encoding="utf-8"?>'
            '<D:propfind xmlns:D="DAV:">'
            '<D:prop><D:resourcetype/></D:prop>'
            '</D:propfind>'
        )
        response = self._session.request(
            'PROPFIND',
            collection_url,
            data=propfind_body,
            headers={
                'Depth': '1',
                'Content-Type': 'application/xml; charset=utf-8',
            },
        )
        self._check_response(response)

        root = ET.fromstring(response.text)
        count = 0
        for href_elem in root.iter('{DAV:}href'):
            href = href_elem.text or ''
            if href.endswith('.ics'):
                count += 1
        return count

    def put_vtodo(self, collection_url, uid, summary, status='NEEDS-ACTION'):
        """Create or overwrite a VTODO resource in a collection.

        Builds a minimal iCalendar VTODO string and sends a PUT request.

        Args:
            collection_url: Full URL of the CalDAV collection.
            uid: Unique identifier for the VTODO (used as the filename).
            summary: Human-readable summary/title of the task.
            status: VTODO status string, defaults to NEEDS-ACTION.

        Raises:
            AssertionError: On non-2xx HTTP response.
        """
        now = datetime.utcnow().strftime('%Y%m%dT%H%M%S')
        today = datetime.utcnow().strftime('%Y%m%d')
        ical_text = (
            'BEGIN:VCALENDAR\r\n'
            'VERSION:2.0\r\n'
            'PRODID:-//caldawarrior-tests//EN\r\n'
            'BEGIN:VTODO\r\n'
            f'UID:{uid}\r\n'
            f'SUMMARY:{summary}\r\n'
            f'STATUS:{status}\r\n'
            f'DTSTAMP:{now}Z\r\n'
            f'DTSTART:{today}\r\n'
            'END:VTODO\r\n'
            'END:VCALENDAR\r\n'
        )
        url = f"{collection_url}{uid}.ics"
        response = self._session.put(
            url,
            data=ical_text.encode('utf-8'),
            headers={'Content-Type': 'text/calendar; charset=utf-8'},
        )
        self._check_response(response)

    def put_vtodo_with_priority(self, collection_url, uid, summary, priority, status='NEEDS-ACTION'):
        """Create or overwrite a VTODO resource with a PRIORITY property set.

        Args:
            collection_url: Full URL of the CalDAV collection.
            uid: Unique identifier for the VTODO (used as the filename).
            summary: Human-readable summary/title of the task.
            priority: Integer priority value (1=high, 5=medium, 9=low per RFC 5545).
            status: VTODO status string, defaults to NEEDS-ACTION.

        Raises:
            AssertionError: On non-2xx HTTP response.
        """
        now = datetime.utcnow().strftime('%Y%m%dT%H%M%S')
        today = datetime.utcnow().strftime('%Y%m%d')
        ical_text = (
            'BEGIN:VCALENDAR\r\n'
            'VERSION:2.0\r\n'
            'PRODID:-//caldawarrior-tests//EN\r\n'
            'BEGIN:VTODO\r\n'
            f'UID:{uid}\r\n'
            f'SUMMARY:{summary}\r\n'
            f'PRIORITY:{priority}\r\n'
            f'STATUS:{status}\r\n'
            f'DTSTAMP:{now}Z\r\n'
            f'DTSTART:{today}\r\n'
            'END:VTODO\r\n'
            'END:VCALENDAR\r\n'
        )
        url = f"{collection_url}{uid}.ics"
        response = self._session.put(
            url,
            data=ical_text.encode('utf-8'),
            headers={'Content-Type': 'text/calendar; charset=utf-8'},
        )
        self._check_response(response)

    def put_vtodo_with_description(self, collection_url, uid, description, status='NEEDS-ACTION'):
        """Create or overwrite a VTODO with DESCRIPTION set and no SUMMARY line.

        Builds an iCalendar VTODO string that omits the SUMMARY property
        entirely, allowing tests to verify behaviour when only DESCRIPTION
        is present.

        Args:
            collection_url: Full URL of the CalDAV collection.
            uid: Unique identifier for the VTODO (used as the filename).
            description: The DESCRIPTION property value.
            status: VTODO status string, defaults to NEEDS-ACTION.

        Raises:
            AssertionError: On non-2xx HTTP response.
        """
        now = datetime.utcnow().strftime('%Y%m%dT%H%M%S')
        today = datetime.utcnow().strftime('%Y%m%d')
        ical_text = (
            'BEGIN:VCALENDAR\r\n'
            'VERSION:2.0\r\n'
            'PRODID:-//caldawarrior-tests//EN\r\n'
            'BEGIN:VTODO\r\n'
            f'UID:{uid}\r\n'
            f'DESCRIPTION:{description}\r\n'
            f'STATUS:{status}\r\n'
            f'DTSTAMP:{now}Z\r\n'
            f'DTSTART:{today}\r\n'
            'END:VTODO\r\n'
            'END:VCALENDAR\r\n'
        )
        url = f"{collection_url}{uid}.ics"
        response = self._session.put(
            url,
            data=ical_text.encode('utf-8'),
            headers={'Content-Type': 'text/calendar; charset=utf-8'},
        )
        self._check_response(response)

    def put_vtodo_with_fields(self, collection_url, uid, summary, **kwargs):
        """Create a VTODO with arbitrary iCal properties.

        Supports keyword arguments: due, dtstart, priority, description,
        categories, wait, status.

        Args:
            collection_url: Full URL of the CalDAV collection.
            uid: Unique identifier for the new VTODO.
            summary: SUMMARY property value.
            **kwargs: Additional properties (due, dtstart, priority, description,
                categories, wait, status).

        Raises:
            AssertionError: On any HTTP error.
        """
        status = kwargs.get('status', 'NEEDS-ACTION')
        lines = [
            'BEGIN:VCALENDAR',
            'VERSION:2.0',
            'PRODID:-//caldawarrior-test//EN',
            'BEGIN:VTODO',
            f'UID:{uid}',
            f'DTSTAMP:{datetime.now(tz=timezone.utc).strftime("%Y%m%dT%H%M%SZ")}',
            f'SUMMARY:{summary}',
            f'STATUS:{status}',
        ]
        if 'due' in kwargs:
            lines.append(f'DUE:{kwargs["due"]}')
        if 'dtstart' in kwargs:
            lines.append(f'DTSTART:{kwargs["dtstart"]}')
        if 'priority' in kwargs:
            lines.append(f'PRIORITY:{kwargs["priority"]}')
        if 'description' in kwargs:
            lines.append(f'DESCRIPTION:{kwargs["description"]}')
        if 'categories' in kwargs:
            lines.append(f'CATEGORIES:{kwargs["categories"]}')
        if 'wait' in kwargs:
            lines.append(f'X-TASKWARRIOR-WAIT:{kwargs["wait"]}')
        lines.extend([
            'END:VTODO',
            'END:VCALENDAR',
        ])
        ical_text = '\r\n'.join(lines) + '\r\n'
        url = f"{collection_url}{uid}.ics"
        response = self._session.put(
            url,
            data=ical_text.encode('utf-8'),
            headers={'Content-Type': 'text/calendar; charset=utf-8'},
        )
        self._check_response(response)

    def put_vtodo_raw_ical(self, collection_url, uid, ical_text):
        """PUT raw iCalendar text directly, for compatibility edge-case tests.

        This keyword enables tests with exact iCal content that cannot be
        produced by the structured put_vtodo_with_fields keyword, such as
        VALUE=DATE parameters, TZID parameters, and arbitrary X-properties.

        Args:
            collection_url: Full URL of the CalDAV collection.
            uid: Unique identifier for the VTODO (used as the filename).
            ical_text: Complete iCalendar text to PUT.

        Raises:
            AssertionError: On non-2xx HTTP response.
        """
        url = f"{collection_url}{uid}.ics"
        response = self._session.put(
            url,
            data=ical_text.encode('utf-8'),
            headers={'Content-Type': 'text/calendar; charset=utf-8'},
        )
        self._check_response(response)

    def get_vtodo_raw(self, collection_url, uid):
        """Retrieve the raw iCalendar text of a VTODO resource.

        Args:
            collection_url: Full URL of the CalDAV collection.
            uid: Unique identifier of the VTODO to retrieve.

        Returns:
            Raw iCalendar text as a string.

        Raises:
            AssertionError: On non-2xx HTTP response.
        """
        url = f"{collection_url}{uid}.ics"
        response = self._session.get(url)
        self._check_response(response)
        return response.text

    def delete_vtodo(self, collection_url, uid):
        """Delete a single VTODO resource from a collection.

        Args:
            collection_url: Full URL of the CalDAV collection.
            uid: Unique identifier of the VTODO to delete.

        Raises:
            AssertionError: On non-2xx response (404 is treated as acceptable).
        """
        url = f"{collection_url}{uid}.ics"
        response = self._session.delete(url)
        self._check_response(response, acceptable_statuses={404})

    # ------------------------------------------------------------------
    # VTODO mutation
    # ------------------------------------------------------------------

    def _get_vtodo_component(self, raw_text):
        """Parse raw iCalendar text and return the first VTODO component.

        Args:
            raw_text: Raw iCalendar string.

        Returns:
            The icalendar component object for the VTODO.

        Raises:
            AssertionError: If no VTODO component is found.
        """
        cal = Calendar.from_ical(raw_text)
        for component in cal.walk():
            if component.name == 'VTODO':
                return component
        raise AssertionError('No VTODO component found in iCalendar data')

    def modify_vtodo_summary(self, collection_url, uid, new_summary):
        """Update the SUMMARY property of an existing VTODO.

        Fetches the VTODO, mutates SUMMARY using the icalendar library, and
        writes the result back via PUT.

        Args:
            collection_url: Full URL of the CalDAV collection.
            uid: Unique identifier of the VTODO to update.
            new_summary: New value for the SUMMARY property.

        Raises:
            AssertionError: On any HTTP error.
        """
        raw = self.get_vtodo_raw(collection_url, uid)
        cal = Calendar.from_ical(raw)
        for component in cal.walk():
            if component.name == 'VTODO':
                component['SUMMARY'] = new_summary
                component['LAST-MODIFIED'] = vDatetime(
                    datetime.now(tz=timezone.utc) + timedelta(seconds=2)
                )
                break
        updated = cal.to_ical().decode('utf-8')
        url = f"{collection_url}{uid}.ics"
        response = self._session.put(
            url,
            data=updated.encode('utf-8'),
            headers={'Content-Type': 'text/calendar; charset=utf-8'},
        )
        self._check_response(response)

    def modify_vtodo_status(self, collection_url, uid, new_status):
        """Update the STATUS property of an existing VTODO.

        Fetches the VTODO, mutates STATUS using the icalendar library, and
        writes the result back via PUT.  When new_status is COMPLETED, the
        COMPLETED timestamp property is also set to the current UTC datetime.

        Args:
            collection_url: Full URL of the CalDAV collection.
            uid: Unique identifier of the VTODO to update.
            new_status: New value for the STATUS property (e.g. COMPLETED,
                IN-PROCESS, NEEDS-ACTION, CANCELLED).

        Raises:
            AssertionError: On any HTTP error.
        """
        raw = self.get_vtodo_raw(collection_url, uid)
        cal = Calendar.from_ical(raw)
        for component in cal.walk():
            if component.name == 'VTODO':
                component['STATUS'] = new_status
                component['LAST-MODIFIED'] = vDatetime(
                    datetime.now(tz=timezone.utc) + timedelta(seconds=2)
                )
                if new_status == 'COMPLETED':
                    completed_dt = datetime.now(tz=timezone.utc)
                    component['COMPLETED'] = vDatetime(completed_dt)
                else:
                    if 'COMPLETED' in component:
                        del component['COMPLETED']
                break
        updated = cal.to_ical().decode('utf-8')
        url = f"{collection_url}{uid}.ics"
        response = self._session.put(
            url,
            data=updated.encode('utf-8'),
            headers={'Content-Type': 'text/calendar; charset=utf-8'},
        )
        self._check_response(response)

    def modify_vtodo_field(self, collection_url, uid, property_name, value):
        """Modify an arbitrary VTODO property and bump LAST-MODIFIED.

        For datetime properties (DUE, DTSTART), value should be an iCal
        datetime string like '20260315T120000Z'. For text properties
        (SUMMARY, DESCRIPTION), value is a plain string.

        Args:
            collection_url: Full URL of the CalDAV collection.
            uid: Unique identifier of the VTODO to update.
            property_name: iCal property name (e.g. DUE, DTSTART, PRIORITY, DESCRIPTION).
            value: New value as string.

        Raises:
            AssertionError: On any HTTP error.
        """
        raw = self.get_vtodo_raw(collection_url, uid)
        cal = Calendar.from_ical(raw)
        for component in cal.walk():
            if component.name == 'VTODO':
                datetime_props = {'DUE', 'DTSTART', 'COMPLETED'}
                if property_name.upper() in datetime_props:
                    from icalendar import vDatetime as vDt
                    dt = datetime.strptime(value, '%Y%m%dT%H%M%SZ').replace(tzinfo=timezone.utc)
                    component[property_name] = vDt(dt)
                elif property_name.upper() == 'PRIORITY':
                    component[property_name] = int(value)
                else:
                    component[property_name] = value
                component['LAST-MODIFIED'] = vDatetime(
                    datetime.now(tz=timezone.utc) + timedelta(seconds=2)
                )
                break
        updated = cal.to_ical().decode('utf-8')
        url = f"{collection_url}{uid}.ics"
        response = self._session.put(
            url,
            data=updated.encode('utf-8'),
            headers={'Content-Type': 'text/calendar; charset=utf-8'},
        )
        self._check_response(response)

    def remove_vtodo_property(self, collection_url, uid, property_name):
        """Remove a property from a VTODO and bump LAST-MODIFIED.

        Used for field clear tests: removing DUE, DTSTART, DESCRIPTION,
        CATEGORIES, PRIORITY, or X-TASKWARRIOR-WAIT from a VTODO.

        Args:
            collection_url: Full URL of the CalDAV collection.
            uid: Unique identifier of the VTODO.
            property_name: iCal property name to remove.

        Raises:
            AssertionError: On any HTTP error.
        """
        raw = self.get_vtodo_raw(collection_url, uid)
        cal = Calendar.from_ical(raw)
        for component in cal.walk():
            if component.name == 'VTODO':
                if property_name in component:
                    del component[property_name]
                component['LAST-MODIFIED'] = vDatetime(
                    datetime.now(tz=timezone.utc) + timedelta(seconds=2)
                )
                break
        updated = cal.to_ical().decode('utf-8')
        url = f"{collection_url}{uid}.ics"
        response = self._session.put(
            url,
            data=updated.encode('utf-8'),
            headers={'Content-Type': 'text/calendar; charset=utf-8'},
        )
        self._check_response(response)

    def add_vtodo_related_to(self, collection_url, uid, related_uid):
        """Add a RELATED-TO;RELTYPE=DEPENDS-ON property to an existing VTODO.

        Fetches the VTODO, adds the RELATED-TO property pointing to
        related_uid, updates LAST-MODIFIED to now (so CalDAV wins LWW on the
        next sync), and writes the result back via PUT.

        Args:
            collection_url: Full URL of the CalDAV collection.
            uid: Unique identifier of the VTODO to update.
            related_uid: The UID of the VTODO that uid depends on.

        Raises:
            AssertionError: On any HTTP error.
        """
        from icalendar import vText
        raw = self.get_vtodo_raw(collection_url, uid)
        cal = Calendar.from_ical(raw)
        for component in cal.walk():
            if component.name == 'VTODO':
                prop = vText(related_uid)
                prop.params['RELTYPE'] = 'DEPENDS-ON'
                component.add('RELATED-TO', prop)
                component['LAST-MODIFIED'] = vDatetime(
                    datetime.now(tz=timezone.utc) + timedelta(seconds=2)
                )
                break
        updated = cal.to_ical().decode('utf-8')
        url = f"{collection_url}{uid}.ics"
        response = self._session.put(
            url,
            data=updated.encode('utf-8'),
            headers={'Content-Type': 'text/calendar; charset=utf-8'},
        )
        self._check_response(response)

    # ------------------------------------------------------------------
    # Property accessors
    # ------------------------------------------------------------------

    def get_vtodo_property(self, collection_url, uid, property_name):
        """Return the string value of a named property from a VTODO.

        Args:
            collection_url: Full URL of the CalDAV collection.
            uid: Unique identifier of the VTODO.
            property_name: Property name as a string (e.g. SUMMARY, STATUS,
                RELATED-TO, UID).

        Returns:
            String value of the requested property.

        Raises:
            AssertionError: If the property is not present on the VTODO.
        """
        raw = self.get_vtodo_raw(collection_url, uid)
        component = self._get_vtodo_component(raw)
        value = component.get(property_name)
        if value is None:
            raise AssertionError(
                f"Property {property_name!r} not found on VTODO {uid!r}"
            )
        return str(value)

    # ------------------------------------------------------------------
    # Assertion keywords
    # ------------------------------------------------------------------

    def vtodo_should_exist(self, collection_url, uid):
        """Assert that a VTODO resource exists in the collection.

        Sends a GET request and raises if the response is not 2xx.

        Args:
            collection_url: Full URL of the CalDAV collection.
            uid: Unique identifier of the VTODO to check.

        Raises:
            AssertionError: If the resource does not exist or returns a
                non-2xx status code.
        """
        url = f"{collection_url}{uid}.ics"
        response = self._session.get(url)
        if not (200 <= response.status_code < 300):
            raise AssertionError(
                f"VTODO {uid!r} does not exist in collection {collection_url!r}: "
                f"HTTP {response.status_code}"
            )

    def vtodo_should_have_property(
        self, collection_url, uid, property_name, expected_value
    ):
        """Assert that a VTODO property has the expected value.

        Args:
            collection_url: Full URL of the CalDAV collection.
            uid: Unique identifier of the VTODO.
            property_name: Property name as a string (e.g. SUMMARY, STATUS).
            expected_value: Expected string value of the property.

        Raises:
            AssertionError: If the actual value does not match expected_value,
                showing both the actual and expected values.
        """
        actual = self.get_vtodo_property(collection_url, uid, property_name)
        if actual != expected_value:
            raise AssertionError(
                f"VTODO {uid!r} property {property_name!r} mismatch:\n"
                f"  Expected: {expected_value!r}\n"
                f"  Actual:   {actual!r}"
            )

    def vtodo_should_not_have_property(self, collection_url, uid, property_name):
        """Assert that a VTODO does NOT have the specified property.

        Fetches the VTODO and parses it with the icalendar library. Raises
        AssertionError if the property is found.

        Args:
            collection_url: Full URL of the CalDAV collection.
            uid: Unique identifier of the VTODO.
            property_name: Property name that should be absent (e.g. COMPLETED, CATEGORIES).

        Raises:
            AssertionError: If the property IS found on the VTODO.
        """
        raw = self.get_vtodo_raw(collection_url, uid)
        component = self._get_vtodo_component(raw)
        if property_name in component:
            value = component[property_name]
            raise AssertionError(
                f"VTODO {uid!r} unexpectedly has property {property_name!r} "
                f"with value {str(value)!r}"
            )
