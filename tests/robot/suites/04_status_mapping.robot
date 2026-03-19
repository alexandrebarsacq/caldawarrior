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

CalDAV Reopen Completed VTODO Syncs To TW Pending
    [Documentation]    S-34: Alice completes a TW task (synced to CalDAV COMPLETED), then
    ...    her colleague reopens the VTODO in CalDAV by setting STATUS to NEEDS-ACTION.
    ...    Alice runs sync and expects the TW task to be back to pending.
    [Tags]    status-mapping
    ${uuid} =    TW.Add TW Task    Task to reopen from CalDAV
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    TW.Complete TW Task    ${uuid}
    Run Caldawarrior Sync
    Exit Code Should Be    0
    CalDAV.VTODO Should Have Property    ${COLLECTION_URL}    ${caldav_uid}    STATUS    COMPLETED
    CalDAV.Modify VTODO Status    ${COLLECTION_URL}    ${caldav_uid}    NEEDS-ACTION
    Run Caldawarrior Sync
    Exit Code Should Be    0
    TW.TW Task Should Have Status    ${uuid}    pending
    CalDAV.VTODO Should Not Have Property    ${COLLECTION_URL}    ${caldav_uid}    COMPLETED

TW Reopen Completed Task Syncs To CalDAV Needs-Action
    [Documentation]    S-35: Alice marks a TW task as done, syncs (CalDAV COMPLETED), then
    ...    modifies the task in TW (making it pending again). Sync should update CalDAV
    ...    to NEEDS-ACTION and remove the COMPLETED timestamp.
    [Tags]    status-mapping
    ${uuid} =    TW.Add TW Task    Task to reopen from TW
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    TW.Complete TW Task    ${uuid}
    Run Caldawarrior Sync
    Exit Code Should Be    0
    CalDAV.VTODO Should Have Property    ${COLLECTION_URL}    ${caldav_uid}    STATUS    COMPLETED
    TW.Modify TW Task    ${uuid}    status=pending
    Run Caldawarrior Sync
    Exit Code Should Be    0
    CalDAV.VTODO Should Have Property    ${COLLECTION_URL}    ${caldav_uid}    STATUS    NEEDS-ACTION
    CalDAV.VTODO Should Not Have Property    ${COLLECTION_URL}    ${caldav_uid}    COMPLETED

TW Delete Syncs To CalDAV Cancelled
    [Documentation]    S-36: Alice deletes a TW task that was previously synced to CalDAV.
    ...    After sync the CalDAV VTODO should have STATUS:CANCELLED.
    [Tags]    status-mapping    deletion
    ${uuid} =    TW.Add TW Task    Task to delete from TW
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    TW.Delete TW Task    ${uuid}
    Run Caldawarrior Sync
    Exit Code Should Be    0
    CalDAV.VTODO Should Have Property    ${COLLECTION_URL}    ${caldav_uid}    STATUS    CANCELLED

CalDAV Cancelled Syncs To TW Deleted
    [Documentation]    S-37: Alice's colleague cancels a CalDAV VTODO (STATUS:CANCELLED).
    ...    Alice runs sync and expects the paired TW task to be deleted.
    ...    This is the fix for the CANCELLED propagation asymmetry.
    [Tags]    status-mapping    deletion
    ${uuid} =    TW.Add TW Task    Task to cancel from CalDAV
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    CalDAV.Modify VTODO Status    ${COLLECTION_URL}    ${caldav_uid}    CANCELLED
    Run Caldawarrior Sync
    Exit Code Should Be    0
    TW.TW Task Should Have Status    ${uuid}    deleted

Both Sides Deleted And Cancelled Produces Zero Writes
    [Documentation]    S-38: Alice has a TW task marked deleted and the paired CalDAV VTODO
    ...    is STATUS:CANCELLED. Running sync should produce zero writes (both terminal).
    [Tags]    status-mapping    deletion
    ${uuid} =    TW.Add TW Task    Task terminal on both sides
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    TW.Delete TW Task    ${uuid}
    Run Caldawarrior Sync
    Exit Code Should Be    0
    CalDAV.VTODO Should Have Property    ${COLLECTION_URL}    ${caldav_uid}    STATUS    CANCELLED
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Sync Should Produce Zero Writes
