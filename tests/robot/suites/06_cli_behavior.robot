*** Settings ***
Resource    ../resources/common.robot
Suite Setup    Suite Setup
Suite Teardown    Suite Teardown
Test Teardown    Test Teardown


*** Keywords ***
Run Caldawarrior Sync With Missing Config
    [Documentation]    Run caldawarrior sync with a config path that does not exist.
    ...    Captures stdout, stderr and exit code into suite variables.
    ${result} =    Run Process    caldawarrior    --config    /tmp/caldawarrior-missing-config.toml
    ...    sync
    ...    env:TASKDATA=${TW_DATA_DIR}    env:TASKRC=${TW_DATA_DIR}/.taskrc
    Set Suite Variable    ${LAST_STDOUT}    ${result.stdout}
    Set Suite Variable    ${LAST_STDERR}    ${result.stderr}
    Set Suite Variable    ${LAST_EXIT_CODE}    ${result.rc}


*** Test Cases ***
Invalid Credentials Produce Auth Error And Exit Code 1
    [Documentation]    S-50: Alice misconfigures caldawarrior with a wrong password. She runs
    ...    sync and expects an auth error on stderr and exit code 1.
    [Tags]    cli-behavior    skip-unimplemented
    [Setup]    Skip If Unimplemented
    ...    Auth error CLI path not covered by blackbox tests yet (see CATALOG.md S-50)
    ${config_content} =    Set Variable
    ...    server_url = "${RADICALE_URL}"\nusername = "${RADICALE_USER}"\npassword = "wrong-password"\n\n[[calendar]]\nproject = "default"\nurl = "${COLLECTION_URL}"\n
    Create File    ${CONFIG_PATH}    ${config_content}
    ${result} =    Run Process    caldawarrior    --config    ${CONFIG_PATH}    sync
    ...    env:TASKDATA=${TW_DATA_DIR}    env:TASKRC=${TW_DATA_DIR}/.taskrc
    ...    stdout=PIPE    stderr=PIPE
    Set Suite Variable    ${LAST_STDOUT}    ${result.stdout}
    Set Suite Variable    ${LAST_STDERR}    ${result.stderr}
    Set Suite Variable    ${LAST_EXIT_CODE}    ${result.rc}
    Remove File    ${CONFIG_PATH}
    Exit Code Should Be    1
    Stderr Should Contain    Authentication failed

World Readable Config File Produces Permission Warning
    [Documentation]    S-51: Alice creates a config file with mode 0644. She runs sync and
    ...    expects a [WARN] about insecure permissions while sync proceeds normally.
    [Tags]    cli-behavior    skip-unimplemented
    [Setup]    Skip If Unimplemented
    ...    Config permission warning CLI path not covered by blackbox tests yet (see CATALOG.md S-51)
    ${config_content} =    Set Variable
    ...    server_url = "${RADICALE_URL}"\nusername = "${RADICALE_USER}"\npassword = "${RADICALE_PASSWORD}"\n\n[[calendar]]\nproject = "default"\nurl = "${COLLECTION_URL}"\n
    Create File    ${CONFIG_PATH}    ${config_content}
    Run Process    chmod    0644    ${CONFIG_PATH}
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Stderr Should Contain    [WARN]
    Stderr Should Contain    0644

TW Recurring Task Is Skipped With Warn Message
    [Documentation]    S-52: Alice has a recurring TW task. She runs sync and expects a
    ...    [WARN] on stderr and no VTODO created.
    [Tags]    cli-behavior    skip-unimplemented
    [Setup]    Skip If Unimplemented
    ...    TW recurring task skip warning not covered by blackbox tests yet (see CATALOG.md S-52)
    ${uuid} =    TW.Add TW Task    Weekly report
    TW.Modify TW Task    ${uuid}    recur=weekly
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Stdout Should Contain    Synced: 0 created, 0 updated in CalDAV; 0 created, 0 updated in TW
    Stderr Should Contain    [WARN]
    Stderr Should Contain    recurring
    ${count} =    CalDAV.Count VTODOs    ${COLLECTION_URL}
    Should Be Equal As Integers    ${count}    0

CalDAV Recurring VTODO Is Skipped With Warn Message
    [Documentation]    S-53: Alice's CalDAV calendar has a VTODO with an RRULE property. She
    ...    runs sync and expects a [WARN] on stderr and no TW task created.
    [Tags]    cli-behavior    skip-unimplemented
    [Setup]    Skip If Unimplemented
    ...    CalDAV recurring VTODO skip warning not covered by blackbox tests yet (see CATALOG.md S-53)
    CalDAV.Put VTODO    ${COLLECTION_URL}    vtodo-recurring-001    Recurring VTODO    NEEDS-ACTION
    Run Caldawarrior Sync
    Exit Code Should Be    0
    Stdout Should Contain    Synced: 0 created, 0 updated in CalDAV; 0 created, 0 updated in TW
    Stderr Should Contain    [WARN]
    Stderr Should Contain    RRULE
    ${count} =    TW.TW Task Count
    Should Be Equal As Integers    ${count}    0

Dry Run Flag Enables Dry Run Mode
    [Documentation]    S-54: Alice wants to preview a sync of 2 TW tasks without writing.
    ...    She runs caldawarrior sync --dry-run and expects [DRY-RUN] prefixed operation lines
    ...    and a Would: summary, with nothing written to CalDAV or TW.
    [Tags]    cli-behavior    dry-run
    ${uuid1} =    TW.Add TW Task    First dry-run task
    ${uuid2} =    TW.Add TW Task    Second dry-run task
    Run Caldawarrior Sync Dry Run
    Exit Code Should Be    0
    Stdout Should Contain    [DRY-RUN] [CREATE]
    Stdout Should Contain    Would: 2 create(s)
    ${count} =    CalDAV.Count VTODOs    ${COLLECTION_URL}
    Should Be Equal As Integers    ${count}    0
    ${task1} =    TW.Get TW Task    ${uuid1}
    ${task2} =    TW.Get TW Task    ${uuid2}
    ${has_caldavuid1} =    Evaluate    'caldavuid' in $task1
    ${has_caldavuid2} =    Evaluate    'caldavuid' in $task2
    Should Not Be True    ${has_caldavuid1}
    Should Not Be True    ${has_caldavuid2}

Missing Config File Produces Fatal Error And Exit Code 1
    [Documentation]    S-55: Alice runs caldawarrior sync with a config path that does not
    ...    exist. She expects an error message on stderr and exit code 1.
    [Tags]    cli-behavior
    Run Caldawarrior Sync With Missing Config
    Exit Code Should Be    1
    Stderr Should Contain    Error:
