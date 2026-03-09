*** Settings ***
Resource    ../resources/common.robot
Suite Setup    Suite Setup
Suite Teardown    Suite Teardown
Test Teardown    Test Teardown


*** Test Cases ***
TW Depends Syncs To CalDAV Related-To
    [Documentation]    S-40: Alice has two TW tasks where "Buy groceries" depends on
    ...    "Go to the store". She runs caldawarrior sync and finds the VTODO for
    ...    "Buy groceries" has a RELATED-TO;RELTYPE=DEPENDS-ON property pointing to
    ...    the UID of the "Go to the store" VTODO.
    [Tags]    dependencies
    ${uuid_b} =    TW.Add TW Task    Go to the store
    ${uuid_a} =    TW.Add TW Task    Buy groceries
    TW.Modify TW Task    ${uuid_a}    depends=${uuid_b}
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Stdout Should Contain    Synced: 2 created, 0 updated in CalDAV; 0 created, 0 updated in TW
    ${task_a} =    TW.Get TW Task    ${uuid_a}
    ${task_b} =    TW.Get TW Task    ${uuid_b}
    ${caldav_uid_a} =    Set Variable    ${task_a}[caldavuid]
    ${caldav_uid_b} =    Set Variable    ${task_b}[caldavuid]
    ${raw_a} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${caldav_uid_a}
    Should Contain    ${raw_a}    RELATED-TO
    Should Contain    ${raw_a}    DEPENDS-ON
    Should Contain    ${raw_a}    ${caldav_uid_b}

CalDAV Related-To Syncs To TW Depends
    [Documentation]    S-41: Alice has two paired CalDAV VTODOs. She adds a
    ...    RELATED-TO;RELTYPE=DEPENDS-ON to VTODO A externally. She runs sync and
    ...    finds that TW task A now has the depends field pointing to TW task B's UUID.
    [Tags]    dependencies
    ${uuid_b} =    TW.Add TW Task    Dependency target task
    ${uuid_a} =    TW.Add TW Task    Dependent task
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task_a} =    TW.Get TW Task    ${uuid_a}
    ${task_b} =    TW.Get TW Task    ${uuid_b}
    ${caldav_uid_a} =    Set Variable    ${task_a}[caldavuid]
    ${caldav_uid_b} =    Set Variable    ${task_b}[caldavuid]
    CalDAV.Add Vtodo Related To    ${COLLECTION_URL}    ${caldav_uid_a}    ${caldav_uid_b}
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Stdout Should Contain    Synced: 0 created, 0 updated in CalDAV; 0 created, 1 updated in TW
    TW.TW Task Should Depend On    ${uuid_a}    ${uuid_b}

Cyclic Dependency Emits Warning And Skips Tasks
    [Documentation]    S-42: Alice creates a cyclic dependency: task A depends on task B,
    ...    and task B depends on task A. She runs sync and expects caldawarrior to detect
    ...    the cycle, emit a WARN for each task, skip both, and exit with code 0.
    [Tags]    dependencies    skip-unimplemented
    [Setup]    Skip If Unimplemented
    ...    Cyclic dependency detection not covered by CLI blackbox tests (see CATALOG.md S-42)
    ${uuid_a} =    TW.Add TW Task    Task A cyclic
    ${uuid_b} =    TW.Add TW Task    Task B cyclic
    TW.Modify TW Task    ${uuid_a}    depends=${uuid_b}
    TW.Modify TW Task    ${uuid_b}    depends=${uuid_a}
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Stdout Should Contain    Synced: 0 created, 0 updated in CalDAV; 0 created, 0 updated in TW
    Stderr Should Contain    CyclicEntry
    ${count} =    CalDAV.Count VTODOs    ${COLLECTION_URL}
    Should Be Equal As Integers    ${count}    0
