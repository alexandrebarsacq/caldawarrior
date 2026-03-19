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

Cyclic Tasks Synced Without Related-To
    [Documentation]    S-42: Alice creates a 2-node cyclic dependency: task A depends on
    ...    task B, and task B depends on task A. She runs sync. Both tasks sync to CalDAV
    ...    with SUMMARY and STATUS, but neither has RELATED-TO properties. Stderr contains
    ...    CyclicEntry warnings for both tasks. Exit code is 0.
    [Tags]    dependencies
    ${uuid_a} =    TW.Add TW Task    Task A cyclic
    ${uuid_b} =    TW.Add TW Task    Task B cyclic
    TW.Modify TW Task    ${uuid_a}    depends=${uuid_b}
    TW.Force TW Dependency    ${uuid_b}    ${uuid_a}
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Stderr Should Contain    CyclicEntry
    # Both tasks should be synced to CalDAV (not skipped)
    ${count} =    CalDAV.Count VTODOs    ${COLLECTION_URL}
    Should Be Equal As Integers    ${count}    2
    # Verify VTODOs have SUMMARY but no RELATED-TO
    ${task_a} =    TW.Get TW Task    ${uuid_a}
    ${task_b} =    TW.Get TW Task    ${uuid_b}
    ${caldav_uid_a} =    Set Variable    ${task_a}[caldavuid]
    ${caldav_uid_b} =    Set Variable    ${task_b}[caldavuid]
    ${raw_a} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${caldav_uid_a}
    ${raw_b} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${caldav_uid_b}
    Should Contain    ${raw_a}    SUMMARY:Task A cyclic
    Should Contain    ${raw_b}    SUMMARY:Task B cyclic
    Should Not Contain    ${raw_a}    RELATED-TO
    Should Not Contain    ${raw_b}    RELATED-TO

Three-Node Cyclic Dependency Synced Without Related-To
    [Documentation]    S-43: Alice creates three tasks in a cycle: A depends B,
    ...    B depends C, C depends A. All three sync to CalDAV with their fields
    ...    but without any RELATED-TO properties. Stderr has 3 CyclicEntry warnings.
    [Tags]    dependencies
    ${uuid_a} =    TW.Add TW Task    Cycle node A
    ${uuid_b} =    TW.Add TW Task    Cycle node B
    ${uuid_c} =    TW.Add TW Task    Cycle node C
    TW.Modify TW Task    ${uuid_a}    depends=${uuid_b}
    TW.Modify TW Task    ${uuid_b}    depends=${uuid_c}
    TW.Force TW Dependency    ${uuid_c}    ${uuid_a}
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Stderr Should Contain    CyclicEntry
    ${count} =    CalDAV.Count VTODOs    ${COLLECTION_URL}
    Should Be Equal As Integers    ${count}    3
    ${task_a} =    TW.Get TW Task    ${uuid_a}
    ${task_b} =    TW.Get TW Task    ${uuid_b}
    ${task_c} =    TW.Get TW Task    ${uuid_c}
    ${raw_a} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${task_a}[caldavuid]
    ${raw_b} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${task_b}[caldavuid]
    ${raw_c} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${task_c}[caldavuid]
    Should Not Contain    ${raw_a}    RELATED-TO
    Should Not Contain    ${raw_b}    RELATED-TO
    Should Not Contain    ${raw_c}    RELATED-TO
    Should Contain    ${raw_a}    SUMMARY:Cycle node A
    Should Contain    ${raw_b}    SUMMARY:Cycle node B
    Should Contain    ${raw_c}    SUMMARY:Cycle node C

TW Blocks Field Reflects Inverse Dependency After Sync
    [Documentation]    S-44: Alice sets task A depends on task B. After sync,
    ...    only A's VTODO has RELATED-TO (B's does not). In TW, B's export
    ...    shows A in its computed blocks field.
    [Tags]    dependencies
    ${uuid_b} =    TW.Add TW Task    Blocking task
    ${uuid_a} =    TW.Add TW Task    Dependent task
    TW.Modify TW Task    ${uuid_a}    depends=${uuid_b}
    Run Caldawarrior Sync
    Exit Code Should Be    0
    # Verify TW depends/blocks
    TW.TW Task Should Depend On    ${uuid_a}    ${uuid_b}
    TW.TW Task Should Have Blocks    ${uuid_b}    ${uuid_a}
    # Verify CalDAV: only A's VTODO has RELATED-TO
    ${task_a} =    TW.Get TW Task    ${uuid_a}
    ${task_b} =    TW.Get TW Task    ${uuid_b}
    ${raw_a} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${task_a}[caldavuid]
    ${raw_b} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${task_b}[caldavuid]
    Should Contain    ${raw_a}    RELATED-TO
    Should Contain    ${raw_a}    DEPENDS-ON
    Should Not Contain    ${raw_b}    RELATED-TO

Removing TW Dependency Clears CalDAV Related-To
    [Documentation]    S-45: Alice has task A depending on task B (synced).
    ...    She removes the dependency and re-syncs. The VTODO for A no longer
    ...    has a RELATED-TO property.
    [Tags]    dependencies
    ${uuid_b} =    TW.Add TW Task    Was blocking
    ${uuid_a} =    TW.Add TW Task    Was dependent
    TW.Modify TW Task    ${uuid_a}    depends=${uuid_b}
    Run Caldawarrior Sync
    Exit Code Should Be    0
    # Verify RELATED-TO exists initially
    ${task_a} =    TW.Get TW Task    ${uuid_a}
    ${caldav_uid_a} =    Set Variable    ${task_a}[caldavuid]
    ${raw_a_before} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${caldav_uid_a}
    Should Contain    ${raw_a_before}    RELATED-TO
    # Remove the dependency in TW
    TW.Modify TW Task    ${uuid_a}    depends=
    Run Caldawarrior Sync
    Exit Code Should Be    0
    # Verify RELATED-TO is gone
    ${raw_a_after} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${caldav_uid_a}
    Should Not Contain    ${raw_a_after}    RELATED-TO
