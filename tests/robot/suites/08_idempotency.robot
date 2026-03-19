*** Settings ***
Resource    ../resources/common.robot
Suite Setup    Suite Setup
Suite Teardown    Suite Teardown
Test Teardown    Test Teardown


*** Test Cases ***
Idempotent After TW Task Creation
    [Documentation]    S-90: Alice creates a TW task and syncs. Running sync a second time
    ...    produces zero writes -- the task is already paired and nothing changed.
    [Tags]    idempotency
    ${uuid} =    TW.Add TW Task    Idempotent create test
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Sync Should Produce Zero Writes

Idempotent After CalDAV Task Creation
    [Documentation]    S-91: Alice creates a CalDAV VTODO and syncs. Running sync again
    ...    produces zero writes -- the task was pulled to TW and nothing changed.
    [Tags]    idempotency
    ${uid} =    Set Variable    vtodo-s91-idempotent-001
    CalDAV.Put VTODO    ${COLLECTION_URL}    ${uid}    Idempotent CalDAV create
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Sync Should Produce Zero Writes

Idempotent After TW Field Update
    [Documentation]    S-92: Alice updates due date and priority on a TW task and syncs.
    ...    Running sync again produces zero writes -- fields are in sync.
    [Tags]    idempotency
    ${uuid} =    TW.Add TW Task    Idempotent field update
    TW.Modify TW Task    ${uuid}    due=2026-06-15
    TW.Modify TW Task    ${uuid}    priority=H
    TW.Modify TW Task    ${uuid}    +important
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Sync Should Produce Zero Writes

Idempotent After CalDAV Field Update
    [Documentation]    S-93: Alice changes the SUMMARY on a CalDAV VTODO and syncs.
    ...    Running sync again produces zero writes.
    [Tags]    idempotency
    ${uuid} =    TW.Add TW Task    Idempotent CalDAV update
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    CalDAV.Modify VTODO Summary    ${COLLECTION_URL}    ${caldav_uid}    Updated from CalDAV
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Sync Should Produce Zero Writes

Idempotent After TW Complete
    [Documentation]    S-94: Alice completes a TW task and syncs. Running sync again
    ...    produces zero writes -- COMPLETED status and timestamp are in sync.
    [Tags]    idempotency
    ${uuid} =    TW.Add TW Task    Idempotent complete test
    Run Caldawarrior Sync
    Exit Code Should Be    0
    TW.Complete TW Task    ${uuid}
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Sync Should Produce Zero Writes

Idempotent After TW Delete
    [Documentation]    S-95: Alice deletes a TW task and syncs (CalDAV becomes CANCELLED).
    ...    Running sync again produces zero writes.
    [Tags]    idempotency
    ${uuid} =    TW.Add TW Task    Idempotent delete test
    Run Caldawarrior Sync
    Exit Code Should Be    0
    TW.Delete TW Task    ${uuid}
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Sync Should Produce Zero Writes
