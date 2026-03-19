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

TW Tags Sync To CalDAV CATEGORIES
    [Documentation]    AUDIT-01: TW tags are mapped to VTODO CATEGORIES during
    ...    TW-to-CalDAV sync. Tags on the TW task should appear as CATEGORIES
    ...    values in the CalDAV VTODO, not ignored or copied from stale data.
    [Tags]    field-mapping    audit-01
    ${uuid} =    TW.Add TW Task    Tag mapping test
    TW.Modify TW Task    ${uuid}    +meeting
    TW.Modify TW Task    ${uuid}    +work
    Run Caldawarrior Sync
    Exit Code Should Be    0
    TW.TW Task Should Have Caldavuid    ${uuid}
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    ${raw} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${caldav_uid}
    Should Contain    ${raw}    CATEGORIES
    Should Contain    ${raw}    meeting
    Should Contain    ${raw}    work

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

TW Description Update Syncs To CalDAV SUMMARY
    [Documentation]    S-69: Alice updates a TW task description from "Old title" to "New title".
    ...    After sync the CalDAV VTODO SUMMARY reflects the new description.
    [Tags]    field-mapping
    ${uuid} =    TW.Add TW Task    Old title
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    TW.Modify TW Task    ${uuid}    description=New title
    Run Caldawarrior Sync
    Exit Code Should Be    0
    CalDAV.VTODO Should Have Property    ${COLLECTION_URL}    ${caldav_uid}    SUMMARY    New title

CalDAV VTODO DUE Syncs To TW Due Date
    [Documentation]    S-70: Alice creates a CalDAV VTODO with DUE:20260615T120000Z.
    ...    After sync the TW task has due matching that date.
    [Tags]    field-mapping
    ${uid} =    Set Variable    vtodo-s70-due-caldav-001
    CalDAV.Put VTODO With Fields    ${COLLECTION_URL}    ${uid}    Task with CalDAV due    due=20260615T120000Z
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task By Caldavuid    ${uid}
    Should Contain    ${task}[due]    20260615

TW Due Update Syncs To CalDAV DUE
    [Documentation]    S-71: Alice changes the due date on a TW task from 2026-03-15 to 2026-06-20.
    ...    After sync the CalDAV VTODO DUE property reflects the new date.
    [Tags]    field-mapping
    ${uuid} =    TW.Add TW Task    Due update test    due=2026-03-15
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    TW.Modify TW Task    ${uuid}    due=2026-06-20
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${raw} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${caldav_uid}
    Should Contain    ${raw}    20260620

CalDAV DUE Update Syncs To TW Due
    [Documentation]    S-72: Alice changes the DUE on a CalDAV VTODO. After sync the TW task
    ...    due date reflects the change.
    [Tags]    field-mapping
    ${uuid} =    TW.Add TW Task    CalDAV due update test    due=2026-03-15
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    CalDAV.Modify VTODO Field    ${COLLECTION_URL}    ${caldav_uid}    DUE    20260720T120000Z
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task2} =    TW.Get TW Task    ${uuid}
    Should Contain    ${task2}[due]    20260720

TW Due Clear Removes CalDAV DUE Property
    [Documentation]    S-73: Alice clears the due date on a TW task. After sync the CalDAV
    ...    VTODO should no longer have a DUE property.
    [Tags]    field-mapping
    ${uuid} =    TW.Add TW Task    Due clear test    due=2026-03-15
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    TW.Modify TW Task    ${uuid}    due=
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${raw} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${caldav_uid}
    Should Not Contain    ${raw}    DUE

CalDAV DUE Removal Syncs To TW Due Cleared
    [Documentation]    S-74: Alice removes the DUE property from a CalDAV VTODO. After sync
    ...    the TW task should have no due date.
    [Tags]    field-mapping
    ${uuid} =    TW.Add TW Task    CalDAV due clear test    due=2026-03-15
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    CalDAV.Remove VTODO Property    ${COLLECTION_URL}    ${caldav_uid}    DUE
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task2} =    TW.Get TW Task    ${uuid}
    ${has_due} =    Evaluate    'due' in $task2
    Should Not Be True    ${has_due}

TW Scheduled Date Syncs To CalDAV DTSTART
    [Documentation]    S-75: Alice creates a TW task with scheduled:2026-04-01. After sync
    ...    the CalDAV VTODO has a DTSTART property matching that date.
    [Tags]    field-mapping
    ${uuid} =    TW.Add TW Task    Scheduled task
    TW.Modify TW Task    ${uuid}    scheduled=2026-04-01
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    ${raw} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${caldav_uid}
    Should Contain    ${raw}    DTSTART
    Should Contain    ${raw}    20260401

CalDAV DTSTART Syncs To TW Scheduled Date
    [Documentation]    S-76: Alice creates a CalDAV VTODO with DTSTART:20260501T090000Z.
    ...    After sync the TW task has a scheduled date matching that datetime.
    [Tags]    field-mapping
    ${uid} =    Set Variable    vtodo-s76-dtstart-001
    CalDAV.Put VTODO With Fields    ${COLLECTION_URL}    ${uid}    DTSTART test task    dtstart=20260501T090000Z
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task By Caldavuid    ${uid}
    Should Contain    ${task}[scheduled]    20260501

TW Scheduled Clear Removes CalDAV DTSTART
    [Documentation]    S-77: Alice clears the scheduled date on a TW task. After sync the
    ...    CalDAV VTODO should no longer have a DTSTART property.
    [Tags]    field-mapping
    ${uuid} =    TW.Add TW Task    Scheduled clear test
    TW.Modify TW Task    ${uuid}    scheduled=2026-04-01
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    TW.Modify TW Task    ${uuid}    scheduled=
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${raw} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${caldav_uid}
    Should Not Contain    ${raw}    DTSTART

TW Priority Syncs To CalDAV PRIORITY
    [Documentation]    S-78: Alice creates a TW task with priority M. After sync the CalDAV
    ...    VTODO has PRIORITY:5 (iCal medium).
    [Tags]    field-mapping
    ${uuid} =    TW.Add TW Task    Priority create test
    TW.Modify TW Task    ${uuid}    priority=M
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    ${raw} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${caldav_uid}
    Should Contain    ${raw}    PRIORITY:5

TW Priority Update Syncs To CalDAV PRIORITY
    [Documentation]    S-79: Alice changes a TW task priority from M to H. After sync the
    ...    CalDAV VTODO PRIORITY changes from 5 to 1.
    [Tags]    field-mapping
    ${uuid} =    TW.Add TW Task    Priority update test
    TW.Modify TW Task    ${uuid}    priority=M
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    TW.Modify TW Task    ${uuid}    priority=H
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${raw} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${caldav_uid}
    Should Contain    ${raw}    PRIORITY:1

TW Priority Clear Removes CalDAV PRIORITY
    [Documentation]    S-80: Alice clears the priority on a TW task. After sync the CalDAV
    ...    VTODO should no longer have a PRIORITY property.
    [Tags]    field-mapping
    ${uuid} =    TW.Add TW Task    Priority clear test
    TW.Modify TW Task    ${uuid}    priority=H
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    TW.Modify TW Task    ${uuid}    priority=
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${raw} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${caldav_uid}
    Should Not Contain    ${raw}    PRIORITY

CalDAV CATEGORIES Syncs To TW Tags
    [Documentation]    S-81: Alice creates a CalDAV VTODO with CATEGORIES:home,errands.
    ...    After sync the TW task has tags home and errands.
    [Tags]    field-mapping
    ${uid} =    Set Variable    vtodo-s81-categories-001
    CalDAV.Put VTODO With Fields    ${COLLECTION_URL}    ${uid}    Categories test    categories=home,errands
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task By Caldavuid    ${uid}
    ${tags} =    Evaluate    $task.get('tags', [])
    Should Contain    ${tags}    home
    Should Contain    ${tags}    errands

TW Tags Update Syncs To CalDAV CATEGORIES
    [Documentation]    S-82: Alice adds a new tag to a TW task (already has +work, adds +urgent).
    ...    After sync the CalDAV VTODO CATEGORIES includes both tags.
    [Tags]    field-mapping
    ${uuid} =    TW.Add TW Task    Tag update test
    TW.Modify TW Task    ${uuid}    +work
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    TW.Modify TW Task    ${uuid}    +urgent
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${raw} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${caldav_uid}
    Should Contain    ${raw}    CATEGORIES
    Should Contain    ${raw}    work
    Should Contain    ${raw}    urgent

TW Tags Cleared Removes CalDAV CATEGORIES Property
    [Documentation]    S-83: Alice removes all tags from a TW task. After sync the CalDAV
    ...    VTODO should NOT have a CATEGORIES property (removed entirely, not empty).
    [Tags]    field-mapping
    ${uuid} =    TW.Add TW Task    Tags clear test
    TW.Modify TW Task    ${uuid}    +meeting
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    TW.Modify TW Task    ${uuid}    -meeting
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${raw} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${caldav_uid}
    Should Not Contain    ${raw}    CATEGORIES

TW Annotation Update Syncs To CalDAV DESCRIPTION
    [Documentation]    S-84: Alice adds a second annotation to a TW task (the first annotation
    ...    maps to DESCRIPTION). After sync the CalDAV VTODO DESCRIPTION reflects
    ...    the first annotation text.
    [Tags]    field-mapping
    ${uuid} =    TW.Add TW Task    Annotation update test
    TW.Add TW Annotation    ${uuid}    First note
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    ${desc} =    CalDAV.Get VTODO Property    ${COLLECTION_URL}    ${caldav_uid}    DESCRIPTION
    Should Be Equal    ${desc}    First note

CalDAV DESCRIPTION Update Syncs To TW Annotation
    [Documentation]    S-85: Alice edits the DESCRIPTION of a CalDAV VTODO. After sync the
    ...    TW task annotation text matches the updated DESCRIPTION.
    [Tags]    field-mapping
    ${uuid} =    TW.Add TW Task    CalDAV desc update test
    TW.Add TW Annotation    ${uuid}    Old note
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    CalDAV.Modify VTODO Field    ${COLLECTION_URL}    ${caldav_uid}    DESCRIPTION    New note
    Run Caldawarrior Sync
    Exit Code Should Be    0
    TW.TW Task Should Have Annotation    ${uuid}    New note

TW Task Without Annotations Has No CalDAV DESCRIPTION
    [Documentation]    S-86: Alice creates a TW task with no annotations. After sync the
    ...    CalDAV VTODO should not have a DESCRIPTION property.
    [Tags]    field-mapping
    ${uuid} =    TW.Add TW Task    No annotation task
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    ${raw} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${caldav_uid}
    Should Not Contain    ${raw}    DESCRIPTION

TW Wait Date Syncs To CalDAV X-TASKWARRIOR-WAIT
    [Documentation]    S-87: Alice creates a TW task with wait:2026-12-01. After sync the
    ...    CalDAV VTODO has an X-TASKWARRIOR-WAIT property with the matching date.
    [Tags]    field-mapping
    ${uuid} =    TW.Add TW Task    Wait test task
    TW.Modify TW Task    ${uuid}    wait=2026-12-01
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    ${raw} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${caldav_uid}
    Should Contain    ${raw}    X-TASKWARRIOR-WAIT
    Should Contain    ${raw}    20261201

TW Wait Clear Removes CalDAV X-TASKWARRIOR-WAIT
    [Documentation]    S-88: Alice clears the wait date on a TW task. After sync the CalDAV
    ...    VTODO should no longer have an X-TASKWARRIOR-WAIT property.
    [Tags]    field-mapping
    ${uuid} =    TW.Add TW Task    Wait clear test
    TW.Modify TW Task    ${uuid}    wait=2026-12-01
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    TW.Modify TW Task    ${uuid}    wait=
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${raw} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${caldav_uid}
    Should Not Contain    ${raw}    X-TASKWARRIOR-WAIT

Completing Task Sets COMPLETED Timestamp And Reopening Clears It
    [Documentation]    S-89: Alice completes a TW task and verifies the CalDAV VTODO gains
    ...    a COMPLETED timestamp. She then reopens the task (status=pending) and verifies
    ...    the COMPLETED property is removed from the CalDAV VTODO.
    [Tags]    field-mapping    status-mapping
    ${uuid} =    TW.Add TW Task    COMPLETED timestamp test
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    TW.Complete TW Task    ${uuid}
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${raw} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${caldav_uid}
    Should Contain    ${raw}    COMPLETED
    TW.Modify TW Task    ${uuid}    status=pending
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${raw2} =    CalDAV.Get VTODO Raw    ${COLLECTION_URL}    ${caldav_uid}
    Should Not Contain    ${raw2}    COMPLETED:
