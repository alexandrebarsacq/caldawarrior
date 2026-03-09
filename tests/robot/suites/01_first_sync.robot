*** Settings ***
Resource    ../resources/common.robot
Suite Setup    Suite Setup
Suite Teardown    Suite Teardown
Test Teardown    Test Teardown


*** Test Cases ***
First Sync Creates TW Task In CalDAV
    [Documentation]    S-01: Alice has two pending TW tasks that have never been synced.
    ...    She runs caldawarrior sync for the first time and expects both tasks to appear
    ...    as VTODOs in her CalDAV calendar.
    [Tags]    first-sync
    ${uuid1} =    TW.Add TW Task    Buy apples
    ${uuid2} =    TW.Add TW Task    Buy oranges
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Stdout Should Contain    Synced: 2 created, 0 updated in CalDAV; 0 created, 0 updated in TW
    ${count} =    CalDAV.Count VTODOs    ${COLLECTION_URL}
    Should Be Equal As Integers    ${count}    2

First Sync Sets Caldavuid UDA On TW Task
    [Documentation]    S-02: After Alice syncs a TW task for the first time, the task
    ...    receives a caldavuid UDA so caldawarrior can track it on future syncs.
    [Tags]    first-sync
    ${uuid} =    TW.Add TW Task    Write report
    Run Caldawarrior Sync
    Exit Code Should Be    0
    TW.TW Task Should Have Caldavuid    ${uuid}

First Sync Dry Run Does Not Write VTODOs
    [Documentation]    S-03: Dave wants to preview what sync would do before committing.
    ...    With --dry-run, no VTODOs should be created and the TW task should not get
    ...    a caldavuid.
    [Tags]    first-sync
    ${uuid} =    TW.Add TW Task    Plan meeting
    Run Caldawarrior Sync Dry Run
    Exit Code Should Be    0
    Stdout Should Contain    [DRY-RUN]
    ${count} =    CalDAV.Count VTODOs    ${COLLECTION_URL}
    Should Be Equal As Integers    ${count}    0

First Sync Routes Projectless Task To Default Calendar
    [Documentation]    S-04: A TW task with no project should route to the "default"
    ...    calendar entry. The task should still sync correctly.
    [Tags]    first-sync
    ${uuid} =    TW.Add TW Task    Projectless task
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${count} =    CalDAV.Count VTODOs    ${COLLECTION_URL}
    Should Be Equal As Integers    ${count}    1
    Stdout Should Contain    Synced: 1 created, 0 updated in CalDAV; 0 created, 0 updated in TW

Five CalDAV VTODOs Created In TW On First Sync
    [Documentation]    S-05: Bob has 3 VTODOs already in his CalDAV calendar. He runs
    ...    caldawarrior sync and expects all 3 to appear as TW tasks.
    [Tags]    first-sync    bulk
    CalDAV.Put VTODO    ${COLLECTION_URL}    vtodo-001    Task Alpha
    CalDAV.Put VTODO    ${COLLECTION_URL}    vtodo-002    Task Beta
    CalDAV.Put VTODO    ${COLLECTION_URL}    vtodo-003    Task Gamma
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Stdout Should Contain    Synced: 0 created, 0 updated in CalDAV; 3 created, 0 updated in TW
    ${count} =    TW.TW Task Count
    Should Be Equal As Integers    ${count}    3
