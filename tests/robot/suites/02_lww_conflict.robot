*** Settings ***
Resource    ../resources/common.robot
Suite Setup    Suite Setup
Suite Teardown    Suite Teardown
Test Teardown    Test Teardown


*** Test Cases ***
TW Wins LWW Conflict Resolution
    [Documentation]    S-10: After a task is synced to CalDAV, modifying it in TW
    ...    makes TW the more-recently-modified side. The next sync should push the
    ...    update to CalDAV (TW wins LWW), leaving CalDAV reflecting TW's new value.
    [Tags]    lww
    ${uuid} =    TW.Add TW Task    Original description
    Run Caldawarrior Sync
    TW.Modify TW Task    ${uuid}    description=Updated by TW
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Stdout Should Contain    Synced: 0 created, 1 updated in CalDAV; 0 created, 0 updated in TW
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    CalDAV.VTODO Should Have Property    ${COLLECTION_URL}    ${caldav_uid}    SUMMARY    Updated by TW

CalDAV Wins LWW Conflict Resolution
    [Documentation]    S-11: After a task is synced, modifying the VTODO in CalDAV
    ...    makes CalDAV the more-recently-modified side. The next sync should pull the
    ...    update into TW (CalDAV wins LWW), leaving TW reflecting CalDAV's new value.
    [Tags]    lww
    ${uuid} =    TW.Add TW Task    Original TW description
    Run Caldawarrior Sync
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    CalDAV.Modify VTODO Summary    ${COLLECTION_URL}    ${caldav_uid}    Updated by CalDAV
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Stdout Should Contain    Synced: 0 created, 0 updated in CalDAV; 0 created, 1 updated in TW
    TW.TW Task Should Have Field    ${uuid}    description    Updated by CalDAV

Immediate Re-Sync After Conflict Is Stable Point
    [Documentation]    S-12: After a CalDAV-wins sync resolves a conflict, both sides
    ...    are in agreement. An immediate second sync should produce zero writes,
    ...    confirming the system has reached a stable point.
    [Tags]    lww    stable-point
    ${uuid} =    TW.Add TW Task    Task to conflict
    Run Caldawarrior Sync
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    CalDAV.Modify VTODO Summary    ${COLLECTION_URL}    ${caldav_uid}    CalDAV change
    Run Caldawarrior Sync
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Sync Should Produce Zero Writes

ETag Conflict Is Handled Without Error
    [Documentation]    S-13: When both TW and CalDAV are modified between syncs,
    ...    caldawarrior encounters an ETag mismatch on PUT. It must handle the 412
    ...    retry gracefully, let LWW pick the winner, and exit 0 without crashing.
    [Tags]    lww    etag
    ${uuid} =    TW.Add TW Task    Task for etag test
    Run Caldawarrior Sync
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    CalDAV.Modify VTODO Summary    ${COLLECTION_URL}    ${caldav_uid}    CalDAV edit
    TW.Modify TW Task    ${uuid}    description=TW edit
    Run Caldawarrior Sync
    Exit Code Should Be    0

LWW Conflict Dry Run Shows Update Without Writing
    [Documentation]    S-14: After a task is synced and then modified in TW, running
    ...    sync with --dry-run should report the pending CalDAV update (showing [DRY-RUN]
    ...    and [UPDATE]) but must not write anything to CalDAV. The CalDAV VTODO should
    ...    still carry the original summary after the dry run.
    [Tags]    lww    dry-run
    ${uuid} =    TW.Add TW Task    Dry run conflict task
    Run Caldawarrior Sync
    TW.Modify TW Task    ${uuid}    description=TW modified
    Run Caldawarrior Sync Dry Run
    Exit Code Should Be    0
    Stdout Should Contain    [DRY-RUN]
    Stdout Should Contain    [UPDATE]
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    CalDAV.VTODO Should Have Property    ${COLLECTION_URL}    ${caldav_uid}    SUMMARY    Dry run conflict task
