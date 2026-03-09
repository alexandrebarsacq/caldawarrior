*** Settings ***
Resource    ../resources/common.robot
Suite Setup    Suite Setup
Suite Teardown    Suite Teardown
Test Teardown    Test Teardown


*** Test Cases ***
TW Description Syncs To VTODO Summary
    [Documentation]    S-60: Alice creates a TW task with description "Pick up dry cleaning".
    ...    She runs caldawarrior sync and finds the CalDAV VTODO SUMMARY is exactly
    ...    "Pick up dry cleaning".
    [Tags]    field-mapping
    ${uuid} =    TW.Add TW Task    Pick up dry cleaning
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Stdout Should Contain    Synced: 1 created, 0 updated in CalDAV; 0 created, 0 updated in TW
    TW.TW Task Should Have Caldavuid    ${uuid}
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    CalDAV.VTODO Should Have Property    ${COLLECTION_URL}    ${caldav_uid}    SUMMARY    Pick up dry cleaning

TW Due Date Syncs To VTODO DUE Property
    [Documentation]    S-61: Alice creates a TW task with a due date. She runs caldawarrior
    ...    sync and finds the CalDAV VTODO has a DUE property whose date matches the TW due.
    [Tags]    field-mapping
    ${uuid} =    TW.Add TW Task    Task with due date    due=2026-03-15
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Stdout Should Contain    Synced: 1 created, 0 updated in CalDAV; 0 created, 0 updated in TW
    TW.TW Task Should Have Caldavuid    ${uuid}
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    ${raw} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${caldav_uid}
    Should Contain    ${raw}    DUE
    Should Contain    ${raw}    20260315

CalDAV VTODO Summary Syncs To TW Description
    [Documentation]    S-62: Alice edits the SUMMARY of a paired VTODO from "Old description"
    ...    to "New description" in her CalDAV client. She runs sync and finds the TW task
    ...    description updated to "New description" (CalDAV wins LWW).
    [Tags]    field-mapping
    ${uuid} =    TW.Add TW Task    Old description
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    CalDAV.Modify VTODO Summary    ${COLLECTION_URL}    ${caldav_uid}    New description
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Stdout Should Contain    Synced: 0 created, 0 updated in CalDAV; 0 created, 1 updated in TW
    TW.TW Task Should Have Field    ${uuid}    description    New description

Caldavuid UDA Stores CalDAV UID As UUID4 String
    [Documentation]    S-63: After Alice syncs a TW task for the first time, she inspects the
    ...    caldavuid field and finds a UUID4 string (8-4-4-4-12 hex groups) that matches the
    ...    UID of the CalDAV VTODO.
    [Tags]    field-mapping
    ${uuid} =    TW.Add TW Task    UUID4 verification task
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Stdout Should Contain    Synced: 1 created, 0 updated in CalDAV; 0 created, 0 updated in TW
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldavuid} =    Set Variable    ${task}[caldavuid]
    Should Match Regexp    ${caldavuid}
    ...    [0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}
    ${vtodo_uid} =    CalDAV.Get VTODO Property    ${COLLECTION_URL}    ${caldavuid}    UID
    Should Be Equal    ${vtodo_uid}    ${caldavuid}
