*** Settings ***
Resource    ../resources/common.robot
Suite Setup    Suite Setup
Suite Teardown    Suite Teardown
Test Teardown    Test Teardown


*** Test Cases ***
CalDAV Completed Status Syncs To TW Completed
    [Documentation]    S-30: Alice marks a VTODO as COMPLETED in her CalDAV client. She runs
    ...    caldawarrior sync and expects the TW task to be marked as completed in TW, matching
    ...    what she did in CalDAV.
    [Tags]    status-mapping
    ${uuid} =    TW.Add TW Task    Task to complete in CalDAV
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    CalDAV.Modify VTODO Status    ${COLLECTION_URL}    ${caldav_uid}    COMPLETED
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Stdout Should Contain    Synced: 0 created, 0 updated in CalDAV; 0 created, 1 updated in TW
    TW.TW Task Should Have Status    ${uuid}    completed

TW Completed Status Syncs To CalDAV Completed
    [Documentation]    S-31: Alice marks a TW task as done. She runs caldawarrior sync and
    ...    expects the corresponding CalDAV VTODO STATUS to be updated to COMPLETED.
    [Tags]    status-mapping
    ${uuid} =    TW.Add TW Task    Task to complete in TW
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    TW.Complete TW Task    ${uuid}
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Stdout Should Contain    Synced: 0 created, 1 updated in CalDAV; 0 created, 0 updated in TW
    CalDAV.VTODO Should Have Property    ${COLLECTION_URL}    ${caldav_uid}    STATUS    COMPLETED

Pending TW Task Stays Pending With Needs-Action VTODO
    [Documentation]    S-32: Alice has a paired TW task (pending) and matching CalDAV VTODO
    ...    (STATUS:NEEDS-ACTION). Neither modified since last sync. She runs sync and expects
    ...    no changes — both sides remain as-is with a zero-write summary.
    [Tags]    status-mapping    stable-point
    ${uuid} =    TW.Add TW Task    Stable pending task
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Sync Should Produce Zero Writes
    CalDAV.VTODO Should Have Property    ${COLLECTION_URL}    ${caldav_uid}    STATUS    NEEDS-ACTION
    TW.TW Task Should Have Status    ${uuid}    pending

Completed Task Within Cutoff Is Synced Beyond Is Not
    [Documentation]    S-33: Alice has two completed TW tasks: one completed recently (within
    ...    the default 90-day cutoff) and one completed 200 days ago (beyond cutoff). She runs
    ...    sync and expects only the recent task to appear in CalDAV.
    [Tags]    status-mapping    cutoff
    ${uuid1} =    TW.Add TW Task    Recent completed task
    ${uuid2} =    TW.Add TW Task    Old completed task
    TW.Complete TW Task    ${uuid1}
    TW.Complete TW Task    ${uuid2}
    TW.Modify TW Task    ${uuid2}    end=2025-08-13
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Stdout Should Contain    Synced: 1 created, 0 updated in CalDAV; 0 created, 0 updated in TW
    ${count} =    CalDAV.Count VTODOs    ${COLLECTION_URL}
    Should Be Equal As Integers    ${count}    1
    ${task1} =    TW.Get TW Task    ${uuid1}
    Should Not Be Empty    ${task1}[caldavuid]
    ${task2} =    TW.Get TW Task    ${uuid2}
    ${has_caldavuid} =    Evaluate    'caldavuid' in $task2
    Should Not Be True    ${has_caldavuid}
