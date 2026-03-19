*** Settings ***
Resource    ../resources/common.robot
Suite Setup    Suite Setup
Suite Teardown    Suite Teardown
Test Teardown    Test Teardown


*** Test Cases ***
S-96: DATE-Only DUE Round-Trip Through Radicale
    [Documentation]    S-96: A VTODO with DUE;VALUE=DATE:YYYYMMDD survives a sync
    ...    cycle without gaining a time component.
    [Tags]    compatibility    date-only
    # Step 1: PUT a VTODO with DATE-only DUE directly to Radicale
    ${ical} =    Catenate    SEPARATOR=\r\n
    ...    BEGIN:VCALENDAR
    ...    VERSION:2.0
    ...    PRODID:-//test//EN
    ...    BEGIN:VTODO
    ...    UID:date-only-due-001
    ...    SUMMARY:Date only task
    ...    DUE;VALUE=DATE:20260315
    ...    STATUS:NEEDS-ACTION
    ...    DTSTAMP:20260319T100000Z
    ...    END:VTODO
    ...    END:VCALENDAR
    ...    ${EMPTY}
    CalDAV.Put VTODO Raw ICal    ${COLLECTION_URL}    date-only-due-001    ${ical}
    # Step 2: Sync CalDAV -> TW (creates TW task from CalDAV VTODO)
    Run Caldawarrior Sync
    Exit Code Should Be    0
    # Step 3: Sync again (TW -> CalDAV writeback, should preserve DATE-only)
    Run Caldawarrior Sync
    Exit Code Should Be    0
    # Step 4: Fetch raw iCal and verify DUE is still DATE-only
    ${raw} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    date-only-due-001
    Should Contain    ${raw}    DUE;VALUE=DATE:20260315
    Should Not Contain    ${raw}    DUE:20260315T

S-97: DATE-Only DTSTART Round-Trip Through Radicale
    [Documentation]    S-97: A VTODO with DTSTART;VALUE=DATE:YYYYMMDD preserves
    ...    date-only format after a sync round-trip.
    [Tags]    compatibility    date-only
    ${ical} =    Catenate    SEPARATOR=\r\n
    ...    BEGIN:VCALENDAR
    ...    VERSION:2.0
    ...    PRODID:-//test//EN
    ...    BEGIN:VTODO
    ...    UID:date-only-dtstart-001
    ...    SUMMARY:Start date only
    ...    DTSTART;VALUE=DATE:20260401
    ...    STATUS:NEEDS-ACTION
    ...    DTSTAMP:20260319T100000Z
    ...    END:VTODO
    ...    END:VCALENDAR
    ...    ${EMPTY}
    CalDAV.Put VTODO Raw ICal    ${COLLECTION_URL}    date-only-dtstart-001    ${ical}
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${raw} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    date-only-dtstart-001
    Should Contain    ${raw}    DTSTART;VALUE=DATE:20260401
    Should Not Contain    ${raw}    DTSTART:20260401T

S-98: TW-Originated Task Uses DATE-TIME In CalDAV
    [Documentation]    S-98: Tasks created in TW always write DATE-TIME to CalDAV
    ...    (not DATE-only), because TW stores full timestamps internally.
    [Tags]    compatibility    date-only
    ${uuid} =    TW.Add TW Task    TW originated task
    TW.Modify TW Task    ${uuid}    due:2026-03-20T12:00:00
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    ${raw} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${caldav_uid}
    # TW-originated tasks always emit DATE-TIME (with time + Z suffix)
    Should Match Regexp    ${raw}    DUE:\\d{8}T\\d{6}Z
    Should Not Contain    ${raw}    VALUE=DATE

S-99: X-Properties Survive Round-Trip Through Radicale
    [Documentation]    S-99: Non-standard X-properties from other clients survive
    ...    a caldawarrior sync cycle unchanged.
    [Tags]    compatibility    x-property
    # PUT VTODO with three X-properties that other clients would write
    ${ical} =    Catenate    SEPARATOR=\r\n
    ...    BEGIN:VCALENDAR
    ...    VERSION:2.0
    ...    PRODID:-//test//EN
    ...    BEGIN:VTODO
    ...    UID:xprop-001
    ...    SUMMARY:Task with X-props
    ...    STATUS:NEEDS-ACTION
    ...    DTSTAMP:20260319T100000Z
    ...    X-APPLE-SORT-ORDER:42
    ...    X-OC-HIDESUBTASKS:1
    ...    X-CUSTOM-FOO:bar-baz
    ...    END:VTODO
    ...    END:VCALENDAR
    ...    ${EMPTY}
    CalDAV.Put VTODO Raw ICal    ${COLLECTION_URL}    xprop-001    ${ical}
    # Sync twice: CalDAV->TW then TW->CalDAV writeback
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Run Caldawarrior Sync
    Exit Code Should Be    0
    # Verify all X-properties survived
    ${raw} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    xprop-001
    Should Contain    ${raw}    X-APPLE-SORT-ORDER:42
    Should Contain    ${raw}    X-OC-HIDESUBTASKS:1
    Should Contain    ${raw}    X-CUSTOM-FOO:bar-baz

S-100: X-TASKWARRIOR-WAIT Coexists With Other X-Properties
    [Documentation]    S-100: caldawarrior's X-TASKWARRIOR-WAIT management does
    ...    not disturb other clients' X-properties during writeback.
    [Tags]    compatibility    x-property
    # PUT VTODO with X-properties AND X-TASKWARRIOR-WAIT
    ${ical} =    Catenate    SEPARATOR=\r\n
    ...    BEGIN:VCALENDAR
    ...    VERSION:2.0
    ...    PRODID:-//test//EN
    ...    BEGIN:VTODO
    ...    UID:xprop-coexist-001
    ...    SUMMARY:Coexist task
    ...    STATUS:NEEDS-ACTION
    ...    DTSTAMP:20260319T100000Z
    ...    X-APPLE-SORT-ORDER:7
    ...    X-TASKWARRIOR-WAIT:20260401T000000Z
    ...    X-OC-HIDESUBTASKS:0
    ...    END:VTODO
    ...    END:VCALENDAR
    ...    ${EMPTY}
    CalDAV.Put VTODO Raw ICal    ${COLLECTION_URL}    xprop-coexist-001    ${ical}
    # Sync: caldawarrior reads VTODO, imports wait into TW
    Run Caldawarrior Sync
    Exit Code Should Be    0
    # Modify TW task description to force writeback on next sync
    ${tw_task} =    TW.Get TW Task By Caldavuid    xprop-coexist-001
    TW.Modify TW Task    ${tw_task}[uuid]    description:Updated coexist task
    Run Caldawarrior Sync
    Exit Code Should Be    0
    # Verify: other X-properties survive caldawarrior's X-TASKWARRIOR-WAIT management
    ${raw} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    xprop-coexist-001
    Should Contain    ${raw}    X-APPLE-SORT-ORDER:7
    Should Contain    ${raw}    X-OC-HIDESUBTASKS:0
