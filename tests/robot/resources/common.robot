# skip-unimplemented tag semantics:
# Tests tagged 'skip-unimplemented' call the Skip keyword in their setup.
# They appear as SKIP (not FAIL) in the HTML report.
# CI should count SKIP separately and not fail the build on SKIP.
# Each skipped test has [Documentation] referencing the missing feature.

*** Settings ***
Library    CalDAVLibrary    WITH NAME    CalDAV
Library    TaskWarriorLibrary    WITH NAME    TW
Library    OperatingSystem
Library    Process
Library    String


*** Variables ***
${ZERO_WRITES_PATTERN}    Synced: 0 created, 0 updated in CalDAV; 0 created, 0 updated in TW
${LAST_STDOUT}            ${EMPTY}
${LAST_STDERR}            ${EMPTY}
${LAST_EXIT_CODE}         ${-1}
${COLLECTION_URL}         ${EMPTY}
${TW_DATA_DIR}            ${EMPTY}
${CONFIG_PATH}            ${EMPTY}


*** Keywords ***
Suite Setup
    ${slug} =    Evaluate    str(uuid.uuid4())[:8]    modules=uuid
    ${COLLECTION_URL} =    CalDAV.Create Collection    ${slug}
    Set Suite Variable    ${COLLECTION_URL}
    TW.Set TW Data Dir    /tmp/tw-${slug}
    Set Suite Variable    ${TW_DATA_DIR}    /tmp/tw-${slug}
    Set Suite Variable    ${CONFIG_PATH}    /tmp/cw-config-${slug}.toml
    Set Suite Variable    ${LAST_STDOUT}    ${EMPTY}
    Set Suite Variable    ${LAST_STDERR}    ${EMPTY}
    Set Suite Variable    ${LAST_EXIT_CODE}    ${-1}

Suite Teardown
    CalDAV.Delete Collection    ${COLLECTION_URL}
    TW.Clear TW Data
    Run Keyword And Ignore Error    Remove File    ${CONFIG_PATH}
    Run Keyword And Ignore Error    Remove Directory    ${TW_DATA_DIR}    recursive=True

Test Teardown
    TW.Clear TW Data
    CalDAV.Clear VTODOs    ${COLLECTION_URL}

Run Caldawarrior Sync
    ${config_content} =    Set Variable
    ...    server_url = "%{RADICALE_URL}"\nusername = "%{RADICALE_USER}"\npassword = "%{RADICALE_PASSWORD}"\n\n[[calendar]]\nproject = "default"\nurl = "${COLLECTION_URL}"\n
    Create File    ${CONFIG_PATH}    ${config_content}
    ${result} =    Run Process    caldawarrior    --config    ${CONFIG_PATH}    sync
    ...    env:TASKDATA=${TW_DATA_DIR}    env:TASKRC=${TW_DATA_DIR}/.taskrc
    Set Suite Variable    ${LAST_STDOUT}    ${result.stdout}
    Set Suite Variable    ${LAST_STDERR}    ${result.stderr}
    Set Suite Variable    ${LAST_EXIT_CODE}    ${result.rc}
    Remove File    ${CONFIG_PATH}

Run Caldawarrior Sync Dry Run
    ${config_content} =    Set Variable
    ...    server_url = "%{RADICALE_URL}"\nusername = "%{RADICALE_USER}"\npassword = "%{RADICALE_PASSWORD}"\n\n[[calendar]]\nproject = "default"\nurl = "${COLLECTION_URL}"\n
    Create File    ${CONFIG_PATH}    ${config_content}
    ${result} =    Run Process    caldawarrior    --config    ${CONFIG_PATH}    sync    --dry-run
    ...    env:TASKDATA=${TW_DATA_DIR}    env:TASKRC=${TW_DATA_DIR}/.taskrc
    Set Suite Variable    ${LAST_STDOUT}    ${result.stdout}
    Set Suite Variable    ${LAST_STDERR}    ${result.stderr}
    Set Suite Variable    ${LAST_EXIT_CODE}    ${result.rc}
    Remove File    ${CONFIG_PATH}

Stdout Should Contain
    [Arguments]    ${expected}
    Should Contain    ${LAST_STDOUT}    ${expected}

Stdout Should Match
    [Arguments]    ${pattern}
    Should Match    ${LAST_STDOUT}    ${pattern}

Stderr Should Contain
    [Arguments]    ${expected}
    Should Contain    ${LAST_STDERR}    ${expected}

Exit Code Should Be
    [Arguments]    ${expected_code}
    Should Be Equal As Integers    ${LAST_EXIT_CODE}    ${expected_code}

Sync Should Produce Zero Writes
    Stdout Should Contain    ${ZERO_WRITES_PATTERN}

Skip If Unimplemented
    [Arguments]    ${reason}
    Skip    ${reason}
