*** Settings ***
Resource    ../resources/common.robot

*** Test Cases ***
Syntax Check Placeholder
    [Documentation]    Dry-run only — verifies library imports resolve without error.
    No Operation
