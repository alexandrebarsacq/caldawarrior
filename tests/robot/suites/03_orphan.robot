*** Settings ***
Resource    ../resources/common.robot
Suite Setup    Suite Setup
Suite Teardown    Suite Teardown
Test Teardown    Test Teardown


*** Test Cases ***
Orphaned Caldavuid Causes TW Task Deletion
    [Documentation]    S-20: Alice deleted a VTODO directly from her CalDAV calendar.
    ...    The corresponding TW task still has caldavuid set. She runs sync. She expects
    ...    the orphaned TW task to be deleted (not re-created in CalDAV).
    [Tags]    orphan    deletion
    ${uuid} =    TW.Add TW Task    Task to orphan
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    CalDAV.Delete VTODO    ${COLLECTION_URL}    ${caldav_uid}
    ${pre_count} =    CalDAV.Count VTODOs    ${COLLECTION_URL}
    Should Be Equal As Integers    ${pre_count}    0
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Sync Should Produce Zero Writes
    ${post_count} =    CalDAV.Count VTODOs    ${COLLECTION_URL}
    Should Be Equal As Integers    ${post_count}    0
    ${tw_count} =    TW.TW Task Count
    Should Be Equal As Integers    ${tw_count}    0

Externally Deleted CalDAV VTODO Causes TW Deletion
    [Documentation]    S-21: Bob synced 2 tasks to CalDAV, then deleted one VTODO
    ...    externally. He expects only 1 TW task to remain after the next sync.
    [Tags]    orphan    deletion
    ${uuid1} =    TW.Add TW Task    Keep this task
    ${uuid2} =    TW.Add TW Task    Delete this from CalDAV
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task2} =    TW.Get TW Task    ${uuid2}
    ${caldav_uid2} =    Set Variable    ${task2}[caldavuid]
    CalDAV.Delete VTODO    ${COLLECTION_URL}    ${caldav_uid2}
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${tw_count} =    TW.TW Task Count
    Should Be Equal As Integers    ${tw_count}    1
    ${caldav_count} =    CalDAV.Count VTODOs    ${COLLECTION_URL}
    Should Be Equal As Integers    ${caldav_count}    1

Re-Sync After Deletion Is Stable Point
    [Documentation]    S-22: After all orphan deletions are processed, an immediate
    ...    re-sync should produce zero writes — the system has reached a stable point.
    [Tags]    orphan    stable-point
    ${uuid} =    TW.Add TW Task    Task for stability test
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${task} =    TW.Get TW Task    ${uuid}
    ${caldav_uid} =    Set Variable    ${task}[caldavuid]
    CalDAV.Delete VTODO    ${COLLECTION_URL}    ${caldav_uid}
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Sync Should Produce Zero Writes

CalDAV Cancelled VTODO Without TW Pair Does Not Create Ghost Task
    [Documentation]    S-23: A CANCELLED VTODO exists on CalDAV but was never synced to TW
    ...    (no TW pair). Running sync should NOT create a TW task for it.
    [Tags]    orphan    deletion
    ${uid} =    Set Variable    vtodo-s23-ghost-cancelled-001
    CalDAV.Put VTODO    ${COLLECTION_URL}    ${uid}    Ghost cancelled task    status=CANCELLED
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${count} =    TW.TW Task Count
    Should Be Equal As Integers    ${count}    0

CalDAV Completed VTODO Without TW Pair Does Not Create Task
    [Documentation]    S-24: A COMPLETED VTODO exists on CalDAV but was never synced to TW.
    ...    Running sync should NOT create a TW task (CalDAV-only terminal entries are skipped).
    [Tags]    orphan    deletion
    ${uid} =    Set Variable    vtodo-s24-ghost-completed-001
    CalDAV.Put VTODO    ${COLLECTION_URL}    ${uid}    Ghost completed task    status=COMPLETED
    Run Caldawarrior Sync
    Exit Code Should Be    0
    ${count} =    TW.TW Task Count
    Should Be Equal As Integers    ${count}    0

CalDAV Completed And TW Completed Both Terminal Zero Writes
    [Documentation]    S-25: Alice has a completed TW task paired with a COMPLETED CalDAV VTODO.
    ...    Running sync should produce zero writes (both terminal, identical).
    [Tags]    orphan    deletion
    ${uuid} =    TW.Add TW Task    Terminal both sides completed
    Run Caldawarrior Sync
    Exit Code Should Be    0
    TW.Complete TW Task    ${uuid}
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Sync Should Produce Zero Writes
