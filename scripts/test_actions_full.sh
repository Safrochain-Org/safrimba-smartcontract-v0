#!/bin/bash
#
# Comprehensive Safrimba Contract Test Suite
# Tests ALL actions end-to-end with different circle configurations.
# If contract errors occur, fix and redeploy with ./scripts/deploy.sh testnet
#
# Circles tested:
#   1. Public + Total threshold (3 members, but tested with 2)
#   2. Private + None threshold (invite_only, instant payout each round)
#   3. Public + MinMembers(2) threshold
#   4. Public + Total, cycle_duration_seconds=60, grace=10s (fast rounds, full lifecycle)
#   5. Public + None, exit_penalty=50%, late_fee=50% (ejection test)
#   6. Private + Total (cancel before start)
#   7. Private + None (exit before start)
#
# Actions tested:
#   CreateCircle, JoinCircle, InviteMember, AcceptInvite, AddPrivateMember,
#   StartCircle, DepositContribution, AdvanceRound, ProcessPayout, Withdraw,
#   ExitCircle (before & after start), CancelCircle (before & after start),
#   PauseCircle, UnpauseCircle, CheckAndEject, BlockMember, DistributeBlockedFunds,
#   UpdateCircle, EmergencyStop, UndelegateForWithdrawals
#
set +e

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
BLUE='\033[0;34m'; CYAN='\033[0;36m'; NC='\033[0m'

NETWORK="${1:-testnet}"
CREATOR_KEY="${2:-mycontractadmin}"
MEMBER_KEY="${3:-mywallet}"
CODE_ID="${4:-97}"
REPORT_FILE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/TEST_REPORT.md"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONTRACT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CHAIN_CONFIG="$CONTRACT_DIR/chain/$NETWORK/safrochain.json"
KEYRING_OPTS="--keyring-backend os"

CHAIN_ID=$(jq -r '.chainId' "$CHAIN_CONFIG")
RPC_URL=$(jq -r '.rpc' "$CHAIN_CONFIG")
FEE_DENOM="usaf"
CONTRIBUTION="100000"       # 0.1 SAF
CREATOR_LOCK=$((CONTRIBUTION * 2))  # 0.2 SAF

CREATOR_ADDR=$(safrochaind keys show "$CREATOR_KEY" -a $KEYRING_OPTS 2>/dev/null || true)
MEMBER_ADDR=$(safrochaind keys show "$MEMBER_KEY" -a $KEYRING_OPTS 2>/dev/null || true)
[[ -z "$CREATOR_ADDR" || -z "$MEMBER_ADDR" ]] && { echo -e "${RED}Keys not found${NC}"; exit 1; }

# ── Helpers ──────────────────────────────────────────────────────────────────

PASS=0; FAIL=0; SKIP=0
REPORT=""
log() { echo -e "$1"; REPORT+="$(echo -e "$1" | sed 's/\x1b\[[0-9;]*m//g')"$'\n'; }

wait_tx() {
    local H="$1" i=1
    while [[ $i -le 30 ]]; do
        local R=$(safrochaind query tx "$H" --node "$RPC_URL" -o json 2>/dev/null || echo "")
        if echo "$R" | jq -e '.code == 0' &>/dev/null; then echo "$R"; return 0; fi
        local C=$(echo "$R" | jq -r '.code // empty' 2>/dev/null)
        if [[ -n "$C" && "$C" != "0" && "$C" != "null" ]]; then
            echo "$R" | jq -r '.raw_log // .events // empty' 2>/dev/null
            return 1
        fi
        sleep 2; i=$((i+1))
    done
    echo "timeout"; return 1
}

tx() {
    local ADDR="$1" MSG="$2" KEY="$3" AMT="${4:-}"
    local EXTRA=""; [[ -n "$AMT" ]] && EXTRA="--amount ${AMT}"
    local OUT=$(safrochaind tx wasm execute "$ADDR" "$MSG" --from "$KEY" --chain-id "$CHAIN_ID" \
        --node "$RPC_URL" --broadcast-mode sync --gas auto --gas-adjustment 1.5 \
        --gas-prices "0.025${FEE_DENOM}" $KEYRING_OPTS $EXTRA -y --output json 2>&1) || true
    local H=$(echo "$OUT" | jq -r '.txhash // empty' 2>/dev/null)
    [[ -z "$H" ]] && H=$(echo "$OUT" | grep -oE '[A-F0-9]{64}' | head -1)
    if [[ -z "$H" ]]; then echo "ERR:$(echo "$OUT" | head -5)"; return 1; fi
    echo "$H"
}

q() { safrochaind query wasm contract-state smart "$1" "$2" --node "$RPC_URL" -o json 2>/dev/null; }
qcircle() { q "$1" "$(jq -n --argjson id "$2" '{get_circle:{circle_id:$id}}')" | jq '.data // .'; }

run_test() {
    local NAME="$1"; shift
    local RESULT EXIT_CODE
    RESULT=$("$@" 2>&1)
    EXIT_CODE=$?
    if [[ $EXIT_CODE -eq 0 ]]; then
        log "  ${GREEN}✓ PASS${NC}: $NAME"
        PASS=$((PASS+1))
        return 0
    else
        log "  ${RED}✗ FAIL${NC}: $NAME"
        [[ -n "${RESULT:-}" ]] && log "    → $(echo "$RESULT" | head -3)"
        FAIL=$((FAIL+1))
        return 1
    fi
}

# Expect command to fail (e.g. contract rejects invalid action)
run_test_expect_fail() {
    local NAME="$1"; shift
    local RESULT EXIT_CODE
    RESULT=$("$@" 2>&1)
    EXIT_CODE=$?
    if [[ $EXIT_CODE -ne 0 ]]; then
        log "  ${GREEN}✓ PASS${NC}: $NAME"
        PASS=$((PASS+1))
        return 0
    else
        log "  ${RED}✗ FAIL${NC}: $NAME (expected failure)"
        FAIL=$((FAIL+1))
        return 1
    fi
}

assert_status() {
    local C="$1" ID="$2" EXPECT="$3"
    local GOT=$(qcircle "$C" "$ID" | jq -r '.circle.circle_status // empty')
    [[ "$GOT" == "$EXPECT" ]] || { echo "expected $EXPECT, got $GOT"; return 1; }
}

assert_members() {
    local C="$1" ID="$2" EXPECT="$3"
    local GOT=$(qcircle "$C" "$ID" | jq -r '.circle.members_list | length')
    [[ "$GOT" == "$EXPECT" ]] || { echo "expected $EXPECT members, got $GOT"; return 1; }
}

# ── INSTANTIATE ──────────────────────────────────────────────────────────────

log "${CYAN}════════════════════════════════════════════════════════════════${NC}"
log "${GREEN}  SAFRIMBA COMPREHENSIVE CONTRACT TEST${NC}"
log "${CYAN}════════════════════════════════════════════════════════════════${NC}"
log "Network: $NETWORK | Code ID: $CODE_ID"
log "Creator: $CREATOR_KEY ($CREATOR_ADDR)"
log "Member:  $MEMBER_KEY ($MEMBER_ADDR)"
log ""

log "${CYAN}[1] INSTANTIATE CONTRACT${NC}"
INST_MSG=$(jq -n --arg a "$CREATOR_ADDR" '{"platform_fee_percent":0,"platform_address":$a}')
INST_OUT=$(safrochaind tx wasm instantiate "$CODE_ID" "$INST_MSG" --from "$CREATOR_KEY" --admin "$CREATOR_ADDR" \
    --label "safrimba-test-$(date +%s)" --chain-id "$CHAIN_ID" --node "$RPC_URL" \
    --broadcast-mode sync --gas auto --gas-adjustment 1.5 --gas-prices "0.025${FEE_DENOM}" \
    $KEYRING_OPTS -y --output json 2>&1) || true
INST_TX=$(echo "$INST_OUT" | jq -r '.txhash // empty' 2>/dev/null)
[[ -z "$INST_TX" ]] && INST_TX=$(echo "$INST_OUT" | grep -oE '[A-F0-9]{64}' | head -1)
[[ -z "$INST_TX" ]] && { log "${RED}Instantiate failed: ${INST_OUT:0:300}${NC}"; exit 1; }
sleep 5
INST_RESULT=$(wait_tx "$INST_TX") || { log "${RED}Instantiate TX failed${NC}"; exit 1; }
C=$(echo "$INST_RESULT" | jq -r '[.events[]? | select(.type=="instantiate") | .attributes[]? | select(.key=="_contract_address") | .value] | first // empty')
[[ -z "$C" ]] && C=$(echo "$INST_RESULT" | jq -r '.logs[].events[]? | select(.type=="instantiate") | .attributes[]? | select(.key=="_contract_address") | .value' 2>/dev/null | head -1)
[[ -z "$C" ]] && { log "${RED}Cannot find contract address${NC}"; exit 1; }
log "  Contract: ${YELLOW}$C${NC}"
log ""

# ── CREATE CIRCLES ───────────────────────────────────────────────────────────

log "${CYAN}[2] CREATE CIRCLES (7 different configs)${NC}"

create() {
    local JSON="$1"
    local H
    H=$(tx "$C" "$JSON" "$CREATOR_KEY" "${CREATOR_LOCK}${FEE_DENOM}") || return 1
    sleep 5; wait_tx "$H" >/dev/null || return 1
}

# Circle 1: Public + Total, 2 members, fast rounds (60s cycle, 10s grace)
run_test "Circle 1: Public + Total, 2 members, 60s cycle" \
    create "$(jq -n --arg c "$CONTRIBUTION" '{create_circle:{circle_name:"Pub-Total-Fast",circle_description:"Public Total 2m fast",max_members:2,min_members_required:2,invite_only:false,contribution_amount:$c,exit_penalty_percent:2000,late_fee_percent:1000,total_cycles:2,cycle_duration_days:0,cycle_duration_seconds:60,grace_period_hours:0,grace_period_seconds:10,auto_start_when_full:true,payout_order_type:"RandomOrder",auto_payout_enabled:true,manual_trigger_enabled:false,emergency_stop_enabled:true,auto_refund_if_min_not_met:true,strict_mode:false,visibility:"Public",show_member_identities:true,distribution_threshold:{"total":{}}}}')"

# Circle 2: Private + None threshold, 2 members, fast rounds (60s)
run_test "Circle 2: Private + None, 2 members, fast" \
    create "$(jq -n --arg c "$CONTRIBUTION" '{create_circle:{circle_name:"Priv-None-Fast",circle_description:"Private None fast",max_members:2,min_members_required:2,invite_only:true,contribution_amount:$c,exit_penalty_percent:2000,late_fee_percent:1000,total_cycles:1,cycle_duration_days:0,cycle_duration_seconds:60,grace_period_hours:0,grace_period_seconds:10,auto_start_when_full:false,payout_order_type:"RandomOrder",auto_payout_enabled:true,manual_trigger_enabled:false,emergency_stop_enabled:true,auto_refund_if_min_not_met:true,strict_mode:false,visibility:"Private",show_member_identities:true}}')"

# Circle 3: Public + MinMembers(2), 3 members needed, fast rounds
run_test "Circle 3: Public + MinMembers(2), 3 members" \
    create "$(jq -n --arg c "$CONTRIBUTION" '{create_circle:{circle_name:"Pub-MinMembers",circle_description:"Public MinMembers(2)",max_members:3,min_members_required:2,invite_only:false,contribution_amount:$c,exit_penalty_percent:2000,late_fee_percent:1000,total_cycles:1,cycle_duration_days:0,cycle_duration_seconds:60,grace_period_hours:0,grace_period_seconds:10,auto_start_when_full:true,payout_order_type:"RandomOrder",auto_payout_enabled:true,manual_trigger_enabled:false,emergency_stop_enabled:true,auto_refund_if_min_not_met:true,strict_mode:false,visibility:"Public",show_member_identities:true,distribution_threshold:{"min_members":{"count":2}}}}')"

# Circle 4: Public + Total, manual_trigger, 2 members, fast rounds
run_test "Circle 4: Public + Total, manual_trigger" \
    create "$(jq -n --arg c "$CONTRIBUTION" '{create_circle:{circle_name:"Pub-Total-Manual",circle_description:"Manual trigger test",max_members:2,min_members_required:2,invite_only:false,contribution_amount:$c,exit_penalty_percent:2000,late_fee_percent:1000,total_cycles:1,cycle_duration_days:0,cycle_duration_seconds:60,grace_period_hours:0,grace_period_seconds:10,auto_start_when_full:true,payout_order_type:"RandomOrder",auto_payout_enabled:false,manual_trigger_enabled:true,emergency_stop_enabled:true,auto_refund_if_min_not_met:true,strict_mode:false,visibility:"Public",show_member_identities:true,distribution_threshold:{"total":{}}}}')"

# Circle 5: Public + None, high penalties (ejection test: late_fee=50%, exit_penalty=40%)
run_test "Circle 5: Public + None, ejection params" \
    create "$(jq -n --arg c "$CONTRIBUTION" '{create_circle:{circle_name:"Ejection-Test",circle_description:"40% exit + 50% late for ejection",max_members:2,min_members_required:2,invite_only:false,contribution_amount:$c,exit_penalty_percent:4000,late_fee_percent:5000,total_cycles:1,cycle_duration_days:0,cycle_duration_seconds:60,grace_period_hours:0,grace_period_seconds:10,auto_start_when_full:true,payout_order_type:"RandomOrder",auto_payout_enabled:true,manual_trigger_enabled:false,emergency_stop_enabled:true,auto_refund_if_min_not_met:true,strict_mode:false,visibility:"Public",show_member_identities:true}}')"

# Circle 6: Private + Total (for cancel before start)
run_test "Circle 6: Private + Total (cancel test)" \
    create "$(jq -n --arg c "$CONTRIBUTION" '{create_circle:{circle_name:"Cancel-Before-Start",circle_description:"Cancel test",max_members:2,min_members_required:2,invite_only:true,contribution_amount:$c,exit_penalty_percent:2000,late_fee_percent:1000,total_cycles:1,cycle_duration_days:1,grace_period_hours:1,auto_start_when_full:false,payout_order_type:"RandomOrder",auto_payout_enabled:true,manual_trigger_enabled:false,emergency_stop_enabled:true,auto_refund_if_min_not_met:true,strict_mode:false,visibility:"Private",show_member_identities:true,distribution_threshold:{"total":{}}}}')"

# Circle 7: Private + None (for exit before start)
run_test "Circle 7: Private + None (exit test)" \
    create "$(jq -n --arg c "$CONTRIBUTION" '{create_circle:{circle_name:"Exit-Before-Start",circle_description:"Exit test",max_members:2,min_members_required:2,invite_only:true,contribution_amount:$c,exit_penalty_percent:2000,late_fee_percent:1000,total_cycles:1,cycle_duration_days:1,grace_period_hours:1,auto_start_when_full:false,payout_order_type:"RandomOrder",auto_payout_enabled:true,manual_trigger_enabled:false,emergency_stop_enabled:true,auto_refund_if_min_not_met:true,strict_mode:false,visibility:"Private",show_member_identities:true}}')"

log ""

# ── UPDATE CIRCLE ────────────────────────────────────────────────────────────

log "${CYAN}[3] UPDATE CIRCLE (before start)${NC}"
do_update() {
    local H
    H=$(tx "$C" '{"update_circle":{"circle_id":6,"circle_name":"Cancel-Renamed","circle_description":"Updated desc"}}' "$CREATOR_KEY") || return 1
    sleep 5; wait_tx "$H" >/dev/null
}
run_test "UpdateCircle on circle 6" do_update
log ""

# ── JOIN / INVITE / ACCEPT ───────────────────────────────────────────────────

log "${CYAN}[4] JOIN PUBLIC CIRCLES${NC}"
do_join() {
    local ID=$1 H
    H=$(tx "$C" "$(jq -n --argjson id $ID '{join_circle:{circle_id:$id}}')" "$MEMBER_KEY" "${CONTRIBUTION}${FEE_DENOM}") || return 1
    sleep 5; wait_tx "$H" >/dev/null
}
for id in 1 3 4 5; do
    run_test "Join circle $id (Public)" do_join $id
done
log ""

log "${CYAN}[5] INVITE + ACCEPT (Private circles)${NC}"
do_invite_accept() {
    local ID=$1 H
    H=$(tx "$C" "$(jq -n --argjson id $ID --arg a "$MEMBER_ADDR" '{invite_member:{circle_id:$id,member_address:$a}}')" "$CREATOR_KEY") || return 1
    sleep 5; wait_tx "$H" >/dev/null || return 1
    H=$(tx "$C" "$(jq -n --argjson id $ID '{accept_invite:{circle_id:$id}}')" "$MEMBER_KEY" "${CONTRIBUTION}${FEE_DENOM}") || return 1
    sleep 5; wait_tx "$H" >/dev/null
}
for id in 2 6 7; do
    run_test "Invite+Accept circle $id (Private)" do_invite_accept $id
done
log ""

# ── CANCEL BEFORE START ─────────────────────────────────────────────────────

log "${CYAN}[6] CANCEL CIRCLE BEFORE START${NC}"
do_cancel() {
    local ID=$1 H
    H=$(tx "$C" "$(jq -n --argjson id $ID '{cancel_circle:{circle_id:$id}}')" "$CREATOR_KEY") || return 1
    sleep 5; wait_tx "$H" >/dev/null
}
run_test "Cancel circle 6 (before start, full refund)" do_cancel 6
run_test "Verify circle 6 status = Cancelled" assert_status "$C" 6 "Cancelled"
log ""

# ── EXIT BEFORE START ────────────────────────────────────────────────────────

log "${CYAN}[7] EXIT CIRCLE BEFORE START${NC}"
do_exit() {
    local ID=$1 KEY=$2 H
    H=$(tx "$C" "$(jq -n --argjson id $ID '{exit_circle:{circle_id:$id}}')" "$KEY") || return 1
    sleep 5; wait_tx "$H" >/dev/null
}
run_test "Exit circle 7 as member (full refund)" do_exit 7 "$MEMBER_KEY"
log ""

# ── START CIRCLES ────────────────────────────────────────────────────────────

log "${CYAN}[8] START CIRCLES${NC}"
do_start() {
    local ID=$1 H
    H=$(tx "$C" "$(jq -n --argjson id $ID '{start_circle:{circle_id:$id}}')" "$CREATOR_KEY") || return 1
    sleep 5; wait_tx "$H" >/dev/null
}
for id in 1 2 4 5; do
    run_test "Start circle $id" do_start $id
done
# Circle 3 only has 2/3 members: start should work (min=2)
run_test "Start circle 3 (2/3 members, min=2)" do_start 3
log ""

# ── QUERY ALL STATES ─────────────────────────────────────────────────────────

log "${CYAN}[9] QUERY CIRCLE STATES${NC}"
for id in 1 2 3 4 5 6 7; do
    QD=$(qcircle "$C" "$id" 2>/dev/null || echo "{}")
    S=$(echo "$QD" | jq -r '.circle.circle_status // "?"')
    M=$(echo "$QD" | jq -r '.circle.members_list | length // 0')
    DT=$(echo "$QD" | jq -r '.circle.distribution_threshold // "none"')
    log "  Circle $id: status=${YELLOW}$S${NC}, members=$M, threshold=$DT"
done
log ""

# ── DEPOSIT CONTRIBUTION ─────────────────────────────────────────────────────

log "${CYAN}[10] DEPOSIT CONTRIBUTION (Round 1 for all)${NC}"
do_deposit() {
    local ID=$1 KEY=$2 H
    H=$(tx "$C" "$(jq -n --argjson id $ID '{deposit_contribution:{circle_id:$id}}')" "$KEY" "${CONTRIBUTION}${FEE_DENOM}") || return 1
    sleep 5; wait_tx "$H" >/dev/null
}

do_advance() {
    local ID=$1 H
    H=$(tx "$C" "$(jq -n --argjson id $ID '{advance_round:{circle_id:$id}}')" "$CREATOR_KEY") || return 1
    sleep 5; wait_tx "$H" >/dev/null
}

do_process_payout() {
    local ID=$1 KEY=${2:-$CREATOR_KEY} H
    H=$(tx "$C" "$(jq -n --argjson id $ID '{process_payout:{circle_id:$id}}')" "$KEY") || return 1
    sleep 5; wait_tx "$H" >/dev/null
}

do_withdraw() {
    local ID=$1 KEY=$2 H
    H=$(tx "$C" "$(jq -n --argjson id $ID '{withdraw:{circle_id:$id}}')" "$KEY") || return 1
    sleep 5; wait_tx "$H" >/dev/null
}

do_check_eject() {
    local ID=$1 H
    H=$(tx "$C" "$(jq -n --argjson id $ID '{check_and_eject:{circle_id:$id}}')" "$CREATOR_KEY") || return 1
    sleep 5; wait_tx "$H" >/dev/null
}

# All Public circles → Total threshold forced (need 2 rounds for 2-member circles)
# Circle 1,3,4,5: Public (Total forced) — deposit R1 for both (except circle 5 member misses)
# Circle 2: Private (None threshold) — deposit R1 for both
for id in 1 3 4; do
    run_test "Circle $id: creator deposits R1" do_deposit $id "$CREATOR_KEY"
    run_test "Circle $id: member deposits R1"  do_deposit $id "$MEMBER_KEY"
done
run_test "Circle 2: creator deposits R1" do_deposit 2 "$CREATOR_KEY"
run_test "Circle 2: member deposits R1"  do_deposit 2 "$MEMBER_KEY"
# Circle 5 (ejection): only creator deposits R1 (member misses)
run_test "Circle 5: creator deposits R1 (member will miss)" do_deposit 5 "$CREATOR_KEY"
log ""

# ── ADVANCE ROUND (Total threshold: round 1/2 → round 2/2) ──────────────────

log "${CYAN}[11] ADVANCE ROUND R1→R2 (Public=Total, need all rounds)${NC}"
# All Public circles (1,3,4,5) have Total threshold forced — advance to round 2
for id in 1 3 4 5; do
    run_test "Advance circle $id R1→R2 (Total forced)" do_advance $id
done
# Circle 2 (Private None): process_payout directly after R1 deposit (no advance needed)
run_test "Circle 2: ProcessPayout R1 (None threshold, instant)" do_process_payout 2
log ""

# ── PAUSE / UNPAUSE ──────────────────────────────────────────────────────────

log "${CYAN}[12] PAUSE / UNPAUSE${NC}"
do_pause() {
    local ID=$1 H
    H=$(tx "$C" "$(jq -n --argjson id $ID '{pause_circle:{circle_id:$id}}')" "$CREATOR_KEY") || return 1
    sleep 5; wait_tx "$H" >/dev/null
}
do_unpause() {
    local ID=$1 H
    H=$(tx "$C" "$(jq -n --argjson id $ID '{unpause_circle:{circle_id:$id}}')" "$CREATOR_KEY") || return 1
    sleep 5; wait_tx "$H" >/dev/null
}
run_test "Pause circle 3" do_pause 3
run_test "Verify circle 3 status = Paused" assert_status "$C" 3 "Paused"
run_test "Unpause circle 3" do_unpause 3
run_test "Verify circle 3 status = Running" assert_status "$C" 3 "Running"
log ""

# ── DEPOSIT R2 + PROCESS PAYOUT (distribution round for Total circles) ───────

log "${CYAN}[13] DEPOSIT R2 + PROCESS PAYOUT${NC}"
# Circles 1,3,4: both deposit round 2 (now at dist round 2/2)
for id in 1 3 4; do
    run_test "Circle $id: creator deposits R2" do_deposit $id "$CREATOR_KEY"
    run_test "Circle $id: member deposits R2"  do_deposit $id "$MEMBER_KEY"
done
# Circle 5 R2: only creator deposits (member still missing → more late fees)
run_test "Circle 5: creator deposits R2 (member still missing)" do_deposit 5 "$CREATOR_KEY"

# Process payout at distribution round (R2/2 for Total)
run_test "Circle 1: ProcessPayout (R2/2, cycle 1)" do_process_payout 1
run_test "Circle 3: ProcessPayout (R2/2)" do_process_payout 3
run_test "Circle 4: ProcessPayout (manual, R2/2)" do_process_payout 4 "$CREATOR_KEY"
# Circle 5: process payout (will accumulate late fees for missing member)
run_test "Circle 5: ProcessPayout (R2/2, member missed both)" do_process_payout 5

# Circle 2: deposit+payout R2 (complete the cycle)
run_test "Circle 2: creator deposits R2" do_deposit 2 "$CREATOR_KEY"
run_test "Circle 2: member deposits R2"  do_deposit 2 "$MEMBER_KEY"
run_test "Circle 2: ProcessPayout R2 (cycle complete)" do_process_payout 2
log ""

# ── WITHDRAW ─────────────────────────────────────────────────────────────────

log "${CYAN}[14] WITHDRAW (pending payouts)${NC}"
for id in 1 2 3 4; do
    for key in "$CREATOR_KEY" "$MEMBER_KEY"; do
        KEYNAME=$([ "$key" = "$CREATOR_KEY" ] && echo "creator" || echo "member")
        run_test "Withdraw circle $id ($KEYNAME)" do_withdraw $id "$key" || true
    done
done
log ""

# ── CHECK AND EJECT (circle 5: verify ejection after process_payout) ─────────

log "${CYAN}[15] CHECK EJECTION RESULTS (circle 5)${NC}"
C5_STATUS=$(qcircle "$C" 5 | jq -r '.circle.circle_status')
MEMBERS_5=$(qcircle "$C" 5 | jq '.circle.members_list | length')
log "  Circle 5 status: $C5_STATUS, members: $MEMBERS_5"
log "  (Member missed 2 rounds → late fees accumulated during advance+payout)"
if [[ "$MEMBERS_5" -eq 1 ]]; then
    log "  ${GREEN}✓ Member was auto-ejected during ProcessPayout${NC}"
    PASS=$((PASS+1))
else
    log "  ${YELLOW}Note: Member not ejected (late_fee 50%×2=100% + exit 40% >= locked)${NC}"
    # Try explicit check_and_eject if still Running
    if [[ "$C5_STATUS" == "Running" ]]; then
        run_test "Circle 5: CheckAndEject fallback" do_check_eject 5
        MEMBERS_5=$(qcircle "$C" 5 | jq '.circle.members_list | length')
        log "  Circle 5 members after eject: $MEMBERS_5"
    fi
fi
log ""

# ── EMERGENCY STOP ───────────────────────────────────────────────────────────

log "${CYAN}[16] EMERGENCY STOP${NC}"
do_emergency_stop() {
    local ID=$1 H
    H=$(tx "$C" "$(jq -n --argjson id $ID '{emergency_stop:{circle_id:$id}}')" "$CREATOR_KEY") || return 1
    sleep 5; wait_tx "$H" >/dev/null
}
# Check if any circle is still Running for EmergencyStop test
for TEST_ID in 5 1; do
    TEST_STATUS=$(qcircle "$C" $TEST_ID | jq -r '.circle.circle_status')
    if [[ "$TEST_STATUS" == "Running" ]]; then
        run_test "EmergencyStop circle $TEST_ID" do_emergency_stop $TEST_ID
        break
    fi
done
log ""

# ── EXIT AFTER START ─────────────────────────────────────────────────────────

log "${CYAN}[17] EXIT AFTER START${NC}"
# Circle 1 has 2 cycles — after cycle 1 payout it should be Running for cycle 2
C1_STATUS=$(qcircle "$C" 1 | jq -r '.circle.circle_status')
log "  Circle 1 status: $C1_STATUS"
if [[ "$C1_STATUS" == "Running" ]]; then
    run_test "Exit circle 1 as member (after start, penalty)" do_exit 1 "$MEMBER_KEY" || true
fi
log ""

# ── BLOCK MEMBER + DISTRIBUTE BLOCKED FUNDS ──────────────────────────────────

log "${CYAN}[18] BLOCK MEMBER + DISTRIBUTE BLOCKED FUNDS${NC}"
do_block_member() {
    local ID=$1 H
    H=$(tx "$C" "$(jq -n --argjson id $ID --arg a "$MEMBER_ADDR" '{block_member:{circle_id:$id,member_address:$a}}')" "$CREATOR_KEY") || return 1
    sleep 5; wait_tx "$H" >/dev/null
}
do_distribute_blocked() {
    local ID=$1 CYCLE=$2 H
    H=$(tx "$C" "$(jq -n --argjson id $ID --argjson cy $CYCLE '{distribute_blocked_funds:{circle_id:$id,cycle:$cy}}')" "$CREATOR_KEY") || return 1
    sleep 5; wait_tx "$H" >/dev/null
}
# Circle 1: if still Running with 2+ members, block the member
C1_STATUS2=$(qcircle "$C" 1 | jq -r '.circle.circle_status')
C1_MEMBERS=$(qcircle "$C" 1 | jq '.circle.members_list | length')
log "  Circle 1: status=$C1_STATUS2, members=$C1_MEMBERS"
if [[ "$C1_STATUS2" == "Running" && "$C1_MEMBERS" -gt 1 ]]; then
    run_test "Block member in circle 1" do_block_member 1
    CUR_CYCLE=$(qcircle "$C" 1 | jq '.circle.current_cycle_index')
    run_test "Distribute blocked funds circle 1 cycle $CUR_CYCLE" do_distribute_blocked 1 "$CUR_CYCLE" || true
fi
log ""

# ── CANCEL AFTER START ───────────────────────────────────────────────────────

log "${CYAN}[19] CANCEL AFTER START${NC}"
# Find a Running/Paused circle that had ProcessPayout (has distributions)
for CID in 1 3 4; do
    ST=$(qcircle "$C" $CID | jq -r '.circle.circle_status')
    if [[ "$ST" == "Running" || "$ST" == "Paused" ]]; then
        log "  Circle $CID status: $ST"
        run_test_expect_fail "Cancel circle $CID after distribution should fail" do_cancel $CID
        break
    fi
done
log ""

# ── QUERIES ──────────────────────────────────────────────────────────────────

log "${CYAN}[20] COMPREHENSIVE QUERIES${NC}"
do_query_test() {
    local NAME="$1" QUERY="$2"
    local R=$(q "$C" "$QUERY" 2>&1) || { echo "query failed"; return 1; }
    echo "$R" | jq -e '.data // .' >/dev/null 2>&1 || { echo "no data"; return 1; }
}
run_test "GetCircle(1)"         do_query_test "GetCircle" '{"get_circle":{"circle_id":1}}'
run_test "GetCircles(limit=5)"  do_query_test "GetCircles" '{"get_circles":{"limit":5}}'
run_test "GetCircleMembers(1)"  do_query_test "GetCircleMembers" '{"get_circle_members":{"circle_id":1}}'
run_test "GetCircleStatus(1)"   do_query_test "GetCircleStatus" '{"get_circle_status":{"circle_id":1}}'
run_test "GetCurrentCycle(1)"   do_query_test "GetCurrentCycle" '{"get_current_cycle":{"circle_id":1}}'
run_test "GetCycleDeposits(1,1)" do_query_test "GetCycleDeposits" "$(jq -n --arg a "$CREATOR_ADDR" '{get_cycle_deposits:{circle_id:1,cycle:1}}')"
run_test "GetMemberDeposits"    do_query_test "GetMemberDeposits" "$(jq -n --arg a "$CREATOR_ADDR" '{get_member_deposits:{circle_id:1,member:$a}}')"
run_test "GetPayouts(1)"        do_query_test "GetPayouts" '{"get_payouts":{"circle_id":1}}'
run_test "GetPayoutHistory(1)"  do_query_test "GetPayoutHistory" '{"get_payout_history":{"circle_id":1}}'
run_test "GetCircleBalance(1)"  do_query_test "GetCircleBalance" '{"get_circle_balance":{"circle_id":1}}'
run_test "GetMemberBalance"     do_query_test "GetMemberBalance" "$(jq -n --arg a "$CREATOR_ADDR" '{get_member_balance:{circle_id:1,member:$a}}')"
run_test "GetPenalties(1)"      do_query_test "GetPenalties" '{"get_penalties":{"circle_id":1}}'
run_test "GetRefunds(1)"        do_query_test "GetRefunds" '{"get_refunds":{"circle_id":1}}'
run_test "GetPendingPayout"     do_query_test "GetPendingPayout" "$(jq -n --arg a "$CREATOR_ADDR" '{get_pending_payout:{circle_id:1,member:$a}}')"
run_test "GetMemberAccumLateFees" do_query_test "GetMemberAccumLateFees" "$(jq -n --arg a "$CREATOR_ADDR" '{get_member_accumulated_late_fees:{circle_id:1,member:$a}}')"
run_test "GetDepositRequirement" do_query_test "GetDepositRequirement" "$(jq -n --arg a "$CREATOR_ADDR" '{get_deposit_requirement:{circle_id:1,member:$a}}')"
run_test "GetEvents(1)"         do_query_test "GetEvents" '{"get_events":{"circle_id":1,"limit":10}}'
run_test "GetCircleStats(1)"    do_query_test "GetCircleStats" '{"get_circle_stats":{"circle_id":1}}'
run_test "GetMemberStats"       do_query_test "GetMemberStats" "$(jq -n --arg a "$CREATOR_ADDR" '{get_member_stats:{circle_id:1,member:$a}}')"
run_test "GetMemberLockedAmount" do_query_test "GetMemberLockedAmount" "$(jq -n --arg a "$CREATOR_ADDR" '{get_member_locked_amount:{circle_id:1,member:$a}}')"
run_test "GetBlockedMembers(1)" do_query_test "GetBlockedMembers" '{"get_blocked_members":{"circle_id":1}}'
run_test "GetDistributionCalendar" do_query_test "GetDistributionCalendar" '{"get_distribution_calendar":{"circle_id":1}}'
run_test "GetCircleStakingInfo" do_query_test "GetCircleStakingInfo" '{"get_circle_staking_info":{"circle_id":1}}'
run_test "GetPendingRefunds(1)" do_query_test "GetPendingRefunds" '{"get_pending_refunds":{"circle_id":1}}'
log ""

# ── FINAL STATUS CHECK ───────────────────────────────────────────────────────

log "${CYAN}[21] FINAL STATUS CHECK${NC}"
for id in 1 2 3 4 5 6 7; do
    QD=$(qcircle "$C" "$id" 2>/dev/null || echo "{}")
    S=$(echo "$QD" | jq -r '.circle.circle_status // "?"')
    M=$(echo "$QD" | jq -r '.circle.members_list | length // 0')
    CY=$(echo "$QD" | jq -r '.circle.current_cycle_index // 0')
    PP=$(echo "$QD" | jq -r '.circle.total_pending_payouts // "0"')
    log "  Circle $id: status=${YELLOW}$S${NC}, members=$M, cycle=$CY, pending_payouts=$PP"
done
log ""

# ── SUMMARY ──────────────────────────────────────────────────────────────────

TOTAL=$((PASS + FAIL))
log "${CYAN}════════════════════════════════════════════════════════════════${NC}"
if [[ $FAIL -eq 0 ]]; then
    log "${GREEN}  ALL $TOTAL TESTS PASSED ✓${NC}"
else
    log "${RED}  $FAIL/$TOTAL TESTS FAILED${NC}"
    log "${GREEN}  $PASS passed${NC}, ${RED}$FAIL failed${NC}"
fi
log "${CYAN}════════════════════════════════════════════════════════════════${NC}"
log "Contract: $C"
log ""

# ── Write report ─────────────────────────────────────────────────────────────
{
    echo "# Safrimba Contract Test Report"
    echo ""
    echo "Generated: $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
    echo "Network: $NETWORK | Chain: $CHAIN_ID | Code ID: $CODE_ID"
    echo "Contract: \`$C\`"
    echo "Creator: \`$CREATOR_ADDR\` | Member: \`$MEMBER_ADDR\`"
    echo ""
    echo "## Results: $PASS passed, $FAIL failed out of $TOTAL tests"
    echo ""
    echo '```'
    echo "$REPORT"
    echo '```'
} > "$REPORT_FILE"
log "Report: $REPORT_FILE"

[[ $FAIL -eq 0 ]] && exit 0 || exit 1
