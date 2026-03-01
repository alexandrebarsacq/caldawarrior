#!/usr/bin/env bash
# Empirical research: TaskWarrior import and CLI behavior
# Tests 9 behavioral items needed for caldawarrior sync design.
# Uses an isolated TASKDATA directory to avoid touching user's actual TW data.
set -uo pipefail

TMPDIR_ROOT=$(mktemp -d /tmp/caldawarrior-tw-research.XXXXXX)
trap 'rm -rf "$TMPDIR_ROOT"' EXIT

# ── helpers ────────────────────────────────────────────────────────────────────
PASS=0; FAIL=0; NOTES=()

pass() { echo "  PASS: $*"; ((PASS++)); }
fail() { echo "  FAIL: $*"; ((FAIL++)); }
note() { echo "  NOTE: $*"; NOTES+=("$*"); }
section() { echo ""; echo "════════════════════════════════════════"; echo "  ITEM $1: $2"; echo "════════════════════════════════════════"; }

new_tw_env() {
    local dir="$TMPDIR_ROOT/$1"
    mkdir -p "$dir"
    cat >"$dir/.taskrc" <<'EOF'
confirmation=no
uda.caldavuid.type=string
uda.caldavuid.label=CalDAV UID
EOF
    echo "$dir"
}

# Regular TW command — stdout+stderr merged (good for seeing errors)
tw() {
    local dir="$1"; shift
    TASKDATA="$dir" TASKRC="$dir/.taskrc" HOME="$dir" task "$@" 2>&1
}

# JSON export — stderr suppressed so JSON is clean
twj() {
    local dir="$1"; shift
    TASKDATA="$dir" TASKRC="$dir/.taskrc" HOME="$dir" task "$@" 2>/dev/null
}

py() { python3 -c "$@"; }

# ── ITEM 1: Does task import mutate `modified`? ────────────────────────────────
section 1 "Does 'task import' mutate 'modified'?"
D=$(new_tw_env "item1")

tw "$D" add "Research task one" >/dev/null
JSON_BEFORE=$(twj "$D" export | py 'import sys,json; t=json.load(sys.stdin)[0]; print(__import__("json").dumps(t))')
UUID=$(py "import json; print(json.loads('''$JSON_BEFORE''')['uuid'])")
MOD_BEFORE=$(py "import json; print(json.loads('''$JSON_BEFORE''').get('modified','MISSING'))")
echo "  UUID: $UUID"
echo "  modified before import: $MOD_BEFORE"

sleep 1
echo "$JSON_BEFORE" | tw "$D" import - >/dev/null
JSON_AFTER=$(twj "$D" export | py 'import sys,json; t=json.load(sys.stdin)[0]; print(__import__("json").dumps(t))')
MOD_AFTER=$(py "import json; print(json.loads('''$JSON_AFTER''').get('modified','MISSING'))")
echo "  modified after import:  $MOD_AFTER"

if [ "$MOD_BEFORE" = "$MOD_AFTER" ]; then
    pass "import preserves 'modified' — does NOT update it to current time"
else
    fail "import CHANGED 'modified': $MOD_BEFORE → $MOD_AFTER"
fi

# Sub-test: import with newer modified value
NEW_MOD=$(date -u +"%Y%m%dT%H%M%SZ")
JSON_NEWER=$(py "
import json, sys
t = json.loads('$JSON_BEFORE')
t['modified'] = '$NEW_MOD'
t['description'] = 'Updated via import'
print(json.dumps(t))
")
echo "$JSON_NEWER" | tw "$D" import - >/dev/null
JSON_AFTER2=$(twj "$D" export | py 'import sys,json; t=json.load(sys.stdin)[0]; print(__import__("json").dumps(t))')
MOD_AFTER2=$(py "import json; print(json.loads('''$JSON_AFTER2''').get('modified','MISSING'))")
DESC_AFTER2=$(py "import json; print(json.loads('''$JSON_AFTER2''').get('description','MISSING'))")
echo "  After import with newer modified: modified=$MOD_AFTER2, description=$DESC_AFTER2"
if [ "$MOD_AFTER2" = "$NEW_MOD" ]; then
    note "import with explicit modified: TW accepts/preserves provided timestamp"
else
    note "import with explicit modified: TW overrode to $MOD_AFTER2"
fi

# ── ITEM 2: task import on deleted UUID ───────────────────────────────────────
section 2 "task import on deleted UUID"
D=$(new_tw_env "item2")

tw "$D" add "Task to be deleted" >/dev/null
UUID=$(twj "$D" export | py 'import sys,json; print(json.load(sys.stdin)[0]["uuid"])')
echo "  UUID: $UUID"

tw "$D" "$UUID" delete 2>/dev/null || true

# Export the deleted task JSON
DELETED_JSON_RAW=$(twj "$D" export all)
DELETED_JSON=$(py "
import json, sys
tasks = json.loads('$DELETED_JSON_RAW'.replace('\n',''))
" 2>/dev/null || twj "$D" export all | py "import sys,json; tasks=json.load(sys.stdin); t=next((t for t in tasks if t.get('uuid')=='$UUID'),None); print(json.dumps(t) if t else '{}')")

DELETED_JSON=$(twj "$D" export all | py "import sys,json; tasks=json.load(sys.stdin); t=next((t for t in tasks if t.get('uuid')=='$UUID'),None); print(json.dumps(t) if t else '{}')")
echo "  Deleted task status: $(py "import json; print(json.loads('''$DELETED_JSON''').get('status','MISSING'))")"

# Import same UUID with status:pending
PENDING_JSON=$(py "
import json
t = json.loads('''$DELETED_JSON''')
t['status'] = 'pending'
t['description'] = 'Re-imported as pending'
t.pop('end', None)
print(json.dumps(t))
")
IMPORT_RESULT=$(echo "$PENDING_JSON" | tw "$D" import - 2>&1)
echo "  Import result: $IMPORT_RESULT"

FINAL_STATUS=$(twj "$D" export all | py "import sys,json; tasks=json.load(sys.stdin); t=next((t for t in tasks if t.get('uuid')=='$UUID'),None); print(t['status'] if t else 'NOT FOUND')")
FINAL_DESC=$(twj "$D" export all | py "import sys,json; tasks=json.load(sys.stdin); t=next((t for t in tasks if t.get('uuid')=='$UUID'),None); print(t.get('description','?') if t else 'NOT FOUND')")
echo "  Final status: $FINAL_STATUS, description: $FINAL_DESC"

case "$FINAL_STATUS" in
    pending)
        pass "import on deleted UUID: task RESURRECTED as pending"
        note "Deleted tasks can be resurrected via import — sync must decide policy" ;;
    deleted)
        pass "import on deleted UUID: task stays deleted (import rejected)"
        note "CRITICAL: import cannot resurrect deleted tasks — sync must not try" ;;
    *)
        note "import on deleted UUID result: status=$FINAL_STATUS" ;;
esac

# ── ITEM 3: task import on pending UUID — duplicate or update? ────────────────
section 3 "task import on pending UUID — duplicate or update?"
D=$(new_tw_env "item3")

tw "$D" add "Original description" >/dev/null
ORIG_JSON=$(twj "$D" export | py 'import sys,json; t=json.load(sys.stdin)[0]; print(__import__("json").dumps(t))')
UUID=$(py "import json; print(json.loads('''$ORIG_JSON''')['uuid'])")
echo "  UUID: $UUID"

NEW_JSON=$(py "import json; t=json.loads('''$ORIG_JSON'''); t['description']='Updated description'; print(json.dumps(t))")
echo "$NEW_JSON" | tw "$D" import - >/dev/null

ALL_JSON=$(twj "$D" export)
COUNT=$(py "import json; print(len(json.loads('''$ALL_JSON''')))" 2>/dev/null || echo "?")
FINAL_DESC=$(twj "$D" export | py "import sys,json; tasks=json.load(sys.stdin); t=next((t for t in tasks if t.get('uuid')=='$UUID'),None); print(t['description'] if t else 'NOT FOUND')")
echo "  Task count after import: $COUNT"
echo "  Description: $FINAL_DESC"

if [ "$COUNT" -eq 1 ] && [ "$FINAL_DESC" = "Updated description" ]; then
    pass "import on pending UUID: UPDATES existing task (no duplicate)"
elif [[ "$COUNT" -gt 1 ]]; then
    fail "import on pending UUID: DUPLICATE created (count=$COUNT)"
else
    note "import on pending UUID: count=$COUNT, desc=$FINAL_DESC"
fi

# ── ITEM 4: fresh UUID4 import ────────────────────────────────────────────────
section 4 "Fresh UUID4 import"
D=$(new_tw_env "item4")

FRESH_UUID=$(python3 -c 'import uuid; print(str(uuid.uuid4()))')
NOW=$(date -u +"%Y%m%dT%H%M%SZ")
FRESH_JSON=$(python3 -c "
import json
print(json.dumps({
    'uuid': '$FRESH_UUID',
    'description': 'Imported from CalDAV',
    'status': 'pending',
    'entry': '$NOW',
    'modified': '$NOW'
}))
")
echo "  Importing UUID: $FRESH_UUID"
IMPORT_RESULT=$(echo "$FRESH_JSON" | tw "$D" import - 2>&1)
echo "  Import result: $IMPORT_RESULT"

FOUND_STATUS=$(twj "$D" export all | python3 -c "
import sys,json
tasks=json.load(sys.stdin)
t=next((t for t in tasks if t.get('uuid')=='$FRESH_UUID'),None)
print('FOUND status=' + t['status'] if t else 'NOT FOUND')
")
echo "  Result: $FOUND_STATUS"

if echo "$FOUND_STATUS" | grep -q "^FOUND"; then
    pass "fresh UUID4 import: task created successfully"
else
    fail "fresh UUID4 import: task not found after import"
fi

# ── ITEM 5: status.not:purged validity ────────────────────────────────────────
section 5 "status.not:purged filter validity"
D=$(new_tw_env "item5")

tw "$D" add "Test task" >/dev/null

RESULT=$(twj "$D" status.not:purged export 2>/dev/null); EXIT1=$?
STDERR1=$(TASKDATA="$D" TASKRC="$D/.taskrc" HOME="$D" task status.not:purged export 2>&1 1>/dev/null)
echo "  'task status.not:purged export' exit=$EXIT1"
echo "  stderr: ${STDERR1:0:200}"
if echo "$RESULT" | python3 -c 'import sys,json; json.load(sys.stdin)' 2>/dev/null; then
    COUNT1=$(echo "$RESULT" | python3 -c 'import sys,json; print(len(json.load(sys.stdin)))')
    pass "status.not:purged is VALID — returned $COUNT1 task(s)"
elif echo "$STDERR1" | grep -qi "unknown\|invalid\|unrecognized\|error"; then
    pass "status.not:purged is INVALID filter (TW rejects it)"
    note "Error: ${STDERR1:0:150}"
else
    note "status.not:purged exit=$EXIT1, stderr=${STDERR1:0:100}"
fi

RESULT2=$(twj "$D" status:purged export 2>/dev/null); EXIT2=$?
STDERR2=$(TASKDATA="$D" TASKRC="$D/.taskrc" HOME="$D" task status:purged export 2>&1 1>/dev/null)
echo "  'task status:purged export' exit=$EXIT2, stderr=${STDERR2:0:100}"
if echo "$RESULT2" | python3 -c 'import sys,json; json.load(sys.stdin)' 2>/dev/null; then
    note "status:purged is valid (positive form works)"
else
    note "status:purged rejected: ${STDERR2:0:100}"
fi

# ── ITEM 6: Expired wait date in export ──────────────────────────────────────
section 6 "Expired wait date in export"
D=$(new_tw_env "item6")

tw "$D" add "Waiting task" wait:yesterday >/dev/null
# Export all including waiting
ALL=$(twj "$D" export all)
TASK=$(echo "$ALL" | python3 -c "import sys,json; tasks=json.load(sys.stdin); t=next((t for t in tasks if 'Waiting' in t.get('description','')),None); print(__import__('json').dumps(t) if t else '{}')")
STATUS=$(echo "$TASK" | python3 -c 'import sys,json; print(json.load(sys.stdin).get("status","MISSING"))' 2>/dev/null)
WAIT=$(echo "$TASK" | python3 -c 'import sys,json; print(json.load(sys.stdin).get("wait","ABSENT"))' 2>/dev/null)
echo "  status: $STATUS"
echo "  wait:   $WAIT"

case "$STATUS" in
    pending)
        pass "expired wait: status=pending (wait is past, task is now active)"
        note "Expired wait task appears as pending in export; wait field: $WAIT" ;;
    waiting)
        pass "expired wait: status=waiting (TW keeps waiting status even past wait date)"
        note "Expired wait task stays as waiting; sync must check wait vs now" ;;
    *)
        note "Unexpected status for expired wait task: $STATUS (wait=$WAIT)" ;;
esac

# ── ITEM 7: caldavuid: trailing colon clears UDA ─────────────────────────────
section 7 "caldavuid: trailing colon clears UDA"
D=$(new_tw_env "item7")

tw "$D" add "Task with UDA" >/dev/null
UUID=$(twj "$D" export | python3 -c 'import sys,json; print(json.load(sys.stdin)[0]["uuid"])')

# Set the UDA
tw "$D" "$UUID" modify "caldavuid:test-uid-123" >/dev/null 2>&1
UID_SET=$(twj "$D" export | python3 -c 'import sys,json; print(json.load(sys.stdin)[0].get("caldavuid","ABSENT"))')
echo "  caldavuid after set: $UID_SET"

# Clear with trailing colon
CLEAR_OUT=$(tw "$D" "$UUID" modify "caldavuid:" 2>&1); EC=$?
echo "  'modify caldavuid:' exit=$EC: $CLEAR_OUT"

UID_AFTER=$(twj "$D" export | python3 -c 'import sys,json; print(json.load(sys.stdin)[0].get("caldavuid","FIELD_ABSENT"))')
echo "  caldavuid after clear: '$UID_AFTER'"

if [ "$UID_AFTER" = "FIELD_ABSENT" ]; then
    pass "caldavuid: trailing colon REMOVES field from exported JSON"
elif [ -z "$UID_AFTER" ]; then
    pass "caldavuid: trailing colon sets to empty string (field present but empty)"
    note "Empty string ≠ absent; check how TW filters/exports empty UDAs"
elif [ $EC -ne 0 ]; then
    fail "caldavuid: trailing colon REJECTED (exit=$EC): $CLEAR_OUT"
else
    fail "caldavuid: trailing colon did NOT clear; value still: '$UID_AFTER'"
fi

# Sub-test: import omitting the UDA field
tw "$D" "$UUID" modify "caldavuid:re-set-uid" >/dev/null 2>&1
CURR_JSON=$(twj "$D" export | python3 -c 'import sys,json; print(__import__("json").dumps(json.load(sys.stdin)[0]))')
JSON_OMIT=$(echo "$CURR_JSON" | python3 -c 'import sys,json; t=json.load(sys.stdin); t.pop("caldavuid",None); print(json.dumps(t))')
echo "$JSON_OMIT" | tw "$D" import - >/dev/null
UID_OMIT=$(twj "$D" export | python3 -c 'import sys,json; print(json.load(sys.stdin)[0].get("caldavuid","FIELD_ABSENT"))')
echo "  caldavuid after import-without-field: '$UID_OMIT'"
if [ "$UID_OMIT" = "FIELD_ABSENT" ]; then
    note "import omitting UDA: field is cleared (omit = clear)"
elif [ "$UID_OMIT" = "re-set-uid" ]; then
    note "import omitting UDA: field is PRESERVED (omit ≠ clear, TW merges)"
else
    note "import omitting UDA result: '$UID_OMIT'"
fi

# ── ITEM 8: task delete idempotency ──────────────────────────────────────────
section 8 "task delete idempotency"
D=$(new_tw_env "item8")

tw "$D" add "Task to delete twice" >/dev/null
UUID=$(twj "$D" export | python3 -c 'import sys,json; print(json.load(sys.stdin)[0]["uuid"])')
echo "  UUID: $UUID"

DEL1=$(tw "$D" "$UUID" delete 2>&1); EC1=$?
echo "  First delete (exit=$EC1): ${DEL1:0:80}"
DEL2=$(tw "$D" "$UUID" delete 2>&1); EC2=$?
echo "  Second delete (exit=$EC2): ${DEL2:0:80}"

if [ $EC2 -eq 0 ]; then
    pass "task delete is idempotent — second delete exits 0"
elif echo "$DEL2" | grep -qi "is not deletable"; then
    # TW 3.x known message for deleting an already-deleted task
    pass "task delete: second delete exits $EC2 with 'is not deletable' (TW 3.x, predictable)"
    note "Second delete not idempotent (exit=$EC2) — sync must guard: ${DEL2:0:100}"
elif echo "$DEL2" | grep -qi "no matches\|nothing\|not found\|invalid\|has no pending\|no pending"; then
    pass "task delete: second delete exits $EC2 with predictable message"
    note "Second delete message: ${DEL2:0:150}"
else
    fail "task delete: second delete unexpected exit=$EC2, output=${DEL2:0:150}"
fi

# ── ITEM 9: task modify field: for sync fields + status transitions ───────────
section 9 "task modify field: for sync fields + status transitions"
D=$(new_tw_env "item9")

# 9a: Clear 'due' date
tw "$D" add "Task with due" due:tomorrow >/dev/null
UUID=$(twj "$D" export | python3 -c 'import sys,json; print(json.load(sys.stdin)[0]["uuid"])')
DUE_BEFORE=$(twj "$D" export | python3 -c 'import sys,json; print(json.load(sys.stdin)[0].get("due","ABSENT"))')
echo "  9a: due before clear: $DUE_BEFORE"
tw "$D" "$UUID" modify "due:" >/dev/null 2>&1
DUE_AFTER=$(twj "$D" export all | python3 -c "import sys,json; tasks=json.load(sys.stdin); t=next((t for t in tasks if t.get('uuid')=='$UUID'),None); print(t.get('due','ABSENT') if t else 'NOT FOUND')")
echo "  9a: due after 'modify due:': $DUE_AFTER"
if [ "$DUE_AFTER" = "ABSENT" ]; then pass "9a: 'modify due:' clears due date"
else note "9a: 'modify due:' → due=$DUE_AFTER"; fi

# 9b: Clear 'scheduled'
tw "$D" add "Task with scheduled" scheduled:tomorrow >/dev/null
UUID_S=$(twj "$D" export | python3 -c 'import sys,json; tasks=json.load(sys.stdin); t=next((t for t in tasks if "scheduled" in t.get("description","")),None); print(t["uuid"] if t else "NONE")')
echo "  9b: UUID=$UUID_S"
if [ "$UUID_S" != "NONE" ]; then
    tw "$D" "$UUID_S" modify "scheduled:" >/dev/null 2>&1
    SCHED_AFTER=$(twj "$D" export all | python3 -c "import sys,json; tasks=json.load(sys.stdin); t=next((t for t in tasks if t.get('uuid')=='$UUID_S'),None); print(t.get('scheduled','ABSENT') if t else 'NOT FOUND')")
    echo "  9b: scheduled after clear: $SCHED_AFTER"
    if [ "$SCHED_AFTER" = "ABSENT" ]; then pass "9b: 'modify scheduled:' clears scheduled"
    else note "9b: 'modify scheduled:' → $SCHED_AFTER"; fi
fi

# 9c: pending→completed via 'done'
tw "$D" add "Task to complete" >/dev/null
UUID_C=$(twj "$D" export | python3 -c 'import sys,json; tasks=json.load(sys.stdin); t=next((t for t in tasks if "complete" in t.get("description","")),None); print(t["uuid"] if t else "NONE")')
echo "  9c: UUID=$UUID_C"
if [ "$UUID_C" != "NONE" ]; then
    tw "$D" "$UUID_C" done >/dev/null 2>&1
    STATUS_C=$(twj "$D" export all | python3 -c "import sys,json; tasks=json.load(sys.stdin); t=next((t for t in tasks if t.get('uuid')=='$UUID_C'),None); print(t['status'] if t else 'NOT FOUND')")
    echo "  9c: status after done: $STATUS_C"
    if [ "$STATUS_C" = "completed" ]; then pass "9c: pending→completed via 'task done'"
    else note "9c: done → status=$STATUS_C"; fi
fi

# 9d: pending→deleted via 'modify status:deleted'
tw "$D" add "Task to delete via modify" >/dev/null
UUID_D=$(twj "$D" export | python3 -c 'import sys,json; tasks=json.load(sys.stdin); t=next((t for t in tasks if "delete via" in t.get("description","")),None); print(t["uuid"] if t else "NONE")')
echo "  9d: UUID=$UUID_D"
if [ "$UUID_D" != "NONE" ]; then
    MOD_OUT=$(tw "$D" "$UUID_D" modify "status:deleted" 2>&1); EC_MOD=$?
    STATUS_D=$(twj "$D" export all | python3 -c "import sys,json; tasks=json.load(sys.stdin); t=next((t for t in tasks if t.get('uuid')=='$UUID_D'),None); print(t['status'] if t else 'NOT FOUND')")
    echo "  9d: modify status:deleted exit=$EC_MOD → status=$STATUS_D"
    if [ "$STATUS_D" = "deleted" ]; then pass "9d: pending→deleted via 'modify status:deleted'"
    else note "9d: modify status:deleted result: exit=$EC_MOD output=${MOD_OUT:0:80} status=$STATUS_D"; fi
fi

# 9e: import with status:completed
D2=$(new_tw_env "item9e")
NOW=$(date -u +"%Y%m%dT%H%M%SZ")
COMP_UUID=$(python3 -c 'import uuid; print(str(uuid.uuid4()))')
COMP_JSON=$(python3 -c "import json; print(json.dumps({'uuid':'$COMP_UUID','description':'Pre-completed','status':'completed','entry':'$NOW','modified':'$NOW','end':'$NOW'}))")
COMP_OUT=$(echo "$COMP_JSON" | tw "$D2" import - 2>&1)
echo "  9e: import completed: $COMP_OUT"
COMP_STATUS=$(twj "$D2" export all | python3 -c "import sys,json; tasks=json.load(sys.stdin); t=next((t for t in tasks if t.get('uuid')=='$COMP_UUID'),None); print(t['status'] if t else 'NOT FOUND')")
COMP_END=$(twj "$D2" export all | python3 -c "import sys,json; tasks=json.load(sys.stdin); t=next((t for t in tasks if t.get('uuid')=='$COMP_UUID'),None); print(t.get('end','ABSENT') if t else 'NOT FOUND')")
echo "  9e: status=$COMP_STATUS, end=$COMP_END"
if [ "$COMP_STATUS" = "completed" ]; then pass "9e: import with status:completed works"
else note "9e: import completed → status=$COMP_STATUS"; fi

# 9f: import with status:deleted
D3=$(new_tw_env "item9f")
DEL_UUID=$(python3 -c 'import uuid; print(str(uuid.uuid4()))')
DEL_JSON=$(python3 -c "import json; print(json.dumps({'uuid':'$DEL_UUID','description':'Pre-deleted','status':'deleted','entry':'$NOW','modified':'$NOW','end':'$NOW'}))")
DEL_OUT=$(echo "$DEL_JSON" | tw "$D3" import - 2>&1)
echo "  9f: import deleted: $DEL_OUT"
DEL_STATUS=$(twj "$D3" export all | python3 -c "import sys,json; tasks=json.load(sys.stdin); t=next((t for t in tasks if t.get('uuid')=='$DEL_UUID'),None); print(t['status'] if t else 'NOT FOUND')")
echo "  9f: status=$DEL_STATUS"
if [ "$DEL_STATUS" = "deleted" ]; then pass "9f: import with status:deleted works"
else note "9f: import deleted → status=$DEL_STATUS"; fi

# ── SUMMARY ───────────────────────────────────────────────────────────────────
echo ""
echo "════════════════════════════════════════"
echo "  RESEARCH COMPLETE"
echo "  PASS: $PASS   FAIL: $FAIL"
echo "════════════════════════════════════════"
if [ ${#NOTES[@]} -gt 0 ]; then
    echo ""
    echo "  DESIGN NOTES:"
    for n in "${NOTES[@]}"; do echo "  • $n"; done
fi
echo ""
if [ "$FAIL" -gt 0 ]; then exit 1; fi
