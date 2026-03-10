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

CalDAV SUMMARY-only VTODO Creates TW Task With Matching Description
    [Documentation]    S-64: Alice creates a CalDAV VTODO with only SUMMARY set (no
    ...    DESCRIPTION line). She runs caldawarrior sync and finds a TW task was created
    ...    with description equal to the SUMMARY value.
    [Tags]    field-mapping
    ${uid} =    Set Variable    vtodo-s64-summary-only-001
    CalDAV.Put VTODO    ${COLLECTION_URL}    ${uid}    Buy oat milk
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Stdout Should Contain    Synced: 0 created, 0 updated in CalDAV; 1 created, 0 updated in TW
    ${task} =    TW.Get TW Task By Caldavuid    ${uid}
    Should Be Equal    ${task}[description]    Buy oat milk

CalDAV DESCRIPTION-only VTODO Creates TW Task With Sentinel And Annotation
    [Documentation]    S-65: Alice creates a CalDAV VTODO with DESCRIPTION set but
    ...    no SUMMARY line at all. She runs caldawarrior sync and finds a TW task was
    ...    created with description "(no title)" and an annotation containing the
    ...    DESCRIPTION text.
    [Tags]    field-mapping
    ${uid} =    Set Variable    vtodo-s65-description-only-001
    CalDAV.Put VTODO With Description    ${COLLECTION_URL}    ${uid}    A note about milk
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Stdout Should Contain    Synced: 0 created, 0 updated in CalDAV; 1 created, 0 updated in TW
    ${task} =    TW.Get TW Task By Caldavuid    ${uid}
    Should Be Equal    ${task}[description]    (no title)
    ${tw_uuid} =    Set Variable    ${task}[uuid]
    TW.TW Task Should Have Annotation    ${tw_uuid}    A note about milk

TW Task With Annotation Syncs To CalDAV With SUMMARY And DESCRIPTION
    [Documentation]    S-66: Alice creates a TW task with description "Pick up groceries"
    ...    and adds an annotation "Don't forget the milk". After sync the CalDAV VTODO
    ...    SUMMARY equals the TW description and DESCRIPTION equals the annotation text,
    ...    confirming they are not duplicated into the same field.
    [Tags]    field-mapping
    ${uuid} =    TW.Add TW Task    Pick up groceries
    TW.Add TW Annotation    ${uuid}    Don't forget the milk
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Stdout Should Contain    Synced: 1 created, 0 updated in CalDAV; 0 created, 0 updated in TW
    TW.TW Task Should Have Caldavuid    ${uuid}
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    ${summary} =    CalDAV.Get VTODO Property    ${COLLECTION_URL}    ${caldav_uid}    SUMMARY
    ${description} =    CalDAV.Get VTODO Property    ${COLLECTION_URL}    ${caldav_uid}    DESCRIPTION
    Should Be Equal    ${summary}    Pick up groceries
    Should Be Equal    ${description}    Don't forget the milk
    Should Not Be Equal    ${summary}    ${description}

CalDAV PRIORITY Maps To TW Priority Field
    [Documentation]    S-67: Alice creates three CalDAV VTODOs with PRIORITY 1, 5, and 9.
    ...    After sync she finds TW tasks with priority H, M, and L respectively.
    ...    She also creates a TW task with priority H and confirms it syncs to CalDAV
    ...    with PRIORITY:1.
    [Tags]    field-mapping
    CalDAV.Put VTODO With Priority    ${COLLECTION_URL}    vtodo-s67-pri-high    High priority task    1
    CalDAV.Put VTODO With Priority    ${COLLECTION_URL}    vtodo-s67-pri-med     Medium priority task    5
    CalDAV.Put VTODO With Priority    ${COLLECTION_URL}    vtodo-s67-pri-low     Low priority task    9
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task_h} =    TW.Get TW Task By Caldavuid    vtodo-s67-pri-high
    Should Be Equal    ${task_h}[priority]    H
    ${task_m} =    TW.Get TW Task By Caldavuid    vtodo-s67-pri-med
    Should Be Equal    ${task_m}[priority]    M
    ${task_l} =    TW.Get TW Task By Caldavuid    vtodo-s67-pri-low
    Should Be Equal    ${task_l}[priority]    L
    ${uuid} =    TW.Add TW Task    TW high priority task
    TW.Modify TW Task    ${uuid}    priority=H
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    ${raw} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${caldav_uid}
    Should Contain    ${raw}    PRIORITY:1

CalDAV-Only Task In Project Calendar Sets TW Project
    [Documentation]    S-68: Alice's CalDAV setup has a separate "work" calendar. She
    ...    creates a VTODO in the work calendar and runs sync. The resulting TW task
    ...    has project == "work". Skipped unless MULTI_CALENDAR_ENABLED env var is set.
    [Tags]    field-mapping
    ${enabled} =    Get Environment Variable    MULTI_CALENDAR_ENABLED    ${EMPTY}
    Skip If    '${enabled}' == ''    S-68 skipped: set MULTI_CALENDAR_ENABLED=1 to enable multi-calendar tests
    ${slug} =    Evaluate    str(uuid.uuid4())[:8]    modules=uuid
    ${work_url} =    CalDAV.Create Collection    work-${slug}
    CalDAV.Put VTODO    ${work_url}    vtodo-s68-work-001    Buy work supplies
    ${config_content} =    Set Variable
    ...    server_url = "%{RADICALE_URL}"\nusername = "%{RADICALE_USER}"\npassword = "%{RADICALE_PASSWORD}"\n\n[[calendar]]\nproject = "default"\nurl = "${COLLECTION_URL}"\n\n[[calendar]]\nproject = "work"\nurl = "${work_url}"\n
    Create File    ${CONFIG_PATH}    ${config_content}
    ${result} =    Run Process    caldawarrior    --config    ${CONFIG_PATH}    sync
    ...    env:TASKDATA=${TW_DATA_DIR}    env:TASKRC=${TW_DATA_DIR}/.taskrc
    Remove File    ${CONFIG_PATH}
    Should Be Equal As Integers    ${result.rc}    0
    ${task} =    TW.Get TW Task By Caldavuid    vtodo-s68-work-001
    Should Be Equal    ${task}[project]    work
    CalDAV.Delete Collection    ${work_url}
