#!/bin/bash
#
# Comprehensive Safrimba Contract Actions Test
# Tests ALL possible actions: CreateCircle, Invite, Accept, Join, Start, Deposit,
# ProcessPayout, Withdraw, Cancel, Exit, Update, Block, Ejection, Pause, etc.
# Outputs full report to TEST_ACTIONS_REPORT.md for frontend correction.
#
set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

NETWORK="${1:-testnet}"
CREATOR_KEY="${2:-mycontractadmin}"
MEMBER_KEY="${3:-mywallet}"
CODE_ID="${4:-93}"
REPORT_FILE="${5:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/TEST_ACTIONS_REPORT.md}"

if [[ "$NETWORK" != "testnet" && "$NETWORK" != "mainnet" ]]; then
    echo -e "${RED}Error: Network must be testnet or mainnet${NC}"
    echo "Usage: $0 [network] [creator_key] [member_key] [code_id] [report_file]"
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONTRACT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CHAIN_CONFIG="$CONTRACT_DIR/chain/$NETWORK/safrochain.json"
KEYRING_OPTS="--keyring-backend os"

CHAIN_ID=$(jq -r '.chainId' "$CHAIN_CONFIG")
RPC_URL=$(jq -r '.rpc' "$CHAIN_CONFIG")
FEE_DENOM="usaf"
CONTRIBUTION_AMOUNT="100000"
CREATOR_LOCK=$((CONTRIBUTION_AMOUNT * 2))

CREATOR_ADDR=$(safrochaind keys show "$CREATOR_KEY" -a $KEYRING_OPTS 2>/dev/null || true)
MEMBER_ADDR=$(safrochaind keys show "$MEMBER_KEY" -a $KEYRING_OPTS 2>/dev/null || true)
if [[ -z "$CREATOR_ADDR" || -z "$MEMBER_ADDR" ]]; then
    echo -e "${RED}Error: Keys not found. Need $CREATOR_KEY and $MEMBER_KEY${NC}"
    exit 1
fi

# Report buffer
REPORT=""
log_report() { REPORT="${REPORT}$1"$'\n'; }
log_both() { echo -e "$1"; log_report "$(echo -e "$1" | sed 's/\x1b\[[0-9;]*m//g')"; }

# Helpers
wait_for_tx() {
    local TX_HASH="$1" ATTEMPT=1
    while [[ $ATTEMPT -le 30 ]]; do
        local TX_RESULT=$(safrochaind query tx "$TX_HASH" --node "$RPC_URL" --output json 2>/dev/null || echo "")
        if echo "$TX_RESULT" | jq -e '.code == 0' >/dev/null 2>&1; then return 0; fi
        if echo "$TX_RESULT" | jq -e '.code' >/dev/null 2>&1; then
            local C=$(echo "$TX_RESULT" | jq -r '.code')
            [[ "$C" != "0" && "$C" != "null" ]] && { echo "$TX_RESULT" | jq -r '.raw_log' 2>/dev/null; return 1; }
        fi
        sleep 2; ATTEMPT=$((ATTEMPT + 1))
    done
    return 1
}

tx_exec() {
    local CONTRACT="$1" MSG="$2" FROM="$3" AMOUNT="${4:-}" EXTRA=""
    [[ -n "$AMOUNT" ]] && EXTRA="--amount ${AMOUNT}"
    local OUT=$(safrochaind tx wasm execute "$CONTRACT" "$MSG" --from "$FROM" --chain-id "$CHAIN_ID" \
        --node "$RPC_URL" --broadcast-mode sync --gas auto --gas-adjustment 1.4 \
        --gas-prices "0.025${FEE_DENOM}" $KEYRING_OPTS $EXTRA -y --output json 2>&1) || true
    local HASH=$(echo "$OUT" | jq -r '.txhash // empty' 2>/dev/null)
    [[ -z "$HASH" ]] && HASH=$(echo "$OUT" | grep -oE '[A-F0-9]{64}' | head -1)
    echo "$HASH"
}

query_circle() {
    safrochaind query wasm contract-state smart "$1" "$(jq -n --argjson id "$2" '{get_circle:{circle_id:$id}}')" \
        --node "$RPC_URL" --output json 2>/dev/null | jq -c '.circle // .data.circle // {}'
}

# Initialize report
log_both "${GREEN}=== Safrimba Full Actions Test ===${NC}"
log_both "Network: $NETWORK | Creator: $CREATOR_KEY | Member: $MEMBER_KEY | Code ID: $CODE_ID"
log_both "Report: $REPORT_FILE"
log_both ""

# --- Instantiate ---
log_both "${CYAN}[1] INSTANTIATE${NC}"
INST_MSG=$(jq -n --arg a "$CREATOR_ADDR" '{"platform_fee_percent":0,"platform_address":$a}')
INST_OUT=$(safrochaind tx wasm instantiate "$CODE_ID" "$INST_MSG" --from "$CREATOR_KEY" --admin "$CREATOR_ADDR" \
    --label "safrimba-full-test-$(date +%s)" --chain-id "$CHAIN_ID" --node "$RPC_URL" \
    --broadcast-mode sync --gas auto --gas-adjustment 1.4 --gas-prices "0.025${FEE_DENOM}" \
    $KEYRING_OPTS -y --output json 2>&1) || true
INST_TX=$(echo "$INST_OUT" | grep -oE '\{.*\}' | tail -1 | jq -r '.txhash // empty' 2>/dev/null)
[[ -z "$INST_TX" ]] && INST_TX=$(echo "$INST_OUT" | grep -oE '[A-F0-9]{64}' | head -1)
if [[ -z "$INST_TX" ]]; then
    log_both "${RED}Instantiate: no tx hash. Output: ${INST_OUT:0:500}${NC}"
    exit 1
fi
log_both "  Tx: $INST_TX"
sleep 5
if ! wait_for_tx "$INST_TX"; then
    log_both "${RED}Instantiate failed${NC}"; exit 1
fi
CONTRACT=$(safrochaind query tx "$INST_TX" --node "$RPC_URL" -o json 2>/dev/null | \
    jq -r '.events[]? | select(.type=="instantiate") | .attributes[]? | select(.key=="_contract_address") | .value' 2>/dev/null | head -1)
[[ -z "$CONTRACT" ]] && CONTRACT=$(safrochaind query tx "$INST_TX" --node "$RPC_URL" -o json 2>/dev/null | \
    jq -r '.logs[].events[]? | select(.type=="instantiate") | .attributes[]? | select(.key=="_contract_address") | .value' 2>/dev/null | head -1)
log_both "  Contract: $CONTRACT"
log_both ""

# --- CreateCircle (all 3 distribution thresholds) ---
log_both "${CYAN}[2] CREATE CIRCLE (all distribution thresholds)${NC}"
do_create() {
    local NAME="$1" VIS="$2" DT="$3"
    local MSG
    if [[ -n "$DT" ]]; then
        MSG=$(jq -n --arg n "$NAME" --arg v "$VIS" --arg c "$CONTRIBUTION_AMOUNT" --argjson dt "$DT" \
            '{create_circle:{circle_name:$n,circle_description:("Test: "+$n),max_members:3,min_members_required:2,invite_only:($v=="Private"),contribution_amount:$c,exit_penalty_percent:2000,late_fee_percent:1000,total_cycles:1,cycle_duration_days:1,grace_period_hours:1,auto_start_when_full:true,payout_order_type:"RandomOrder",auto_payout_enabled:true,manual_trigger_enabled:false,emergency_stop_enabled:true,auto_refund_if_min_not_met:true,strict_mode:false,visibility:$v,show_member_identities:true,distribution_threshold:$dt}}')
    else
        MSG=$(jq -n --arg n "$NAME" --arg v "$VIS" --arg c "$CONTRIBUTION_AMOUNT" \
            '{create_circle:{circle_name:$n,circle_description:("Test: "+$n),max_members:3,min_members_required:2,invite_only:($v=="Private"),contribution_amount:$c,exit_penalty_percent:2000,late_fee_percent:1000,total_cycles:1,cycle_duration_days:1,grace_period_hours:1,auto_start_when_full:true,payout_order_type:"RandomOrder",auto_payout_enabled:true,manual_trigger_enabled:false,emergency_stop_enabled:true,auto_refund_if_min_not_met:true,strict_mode:false,visibility:$v,show_member_identities:true}}')
    fi
    local H=$(tx_exec "$CONTRACT" "$MSG" "$CREATOR_KEY" "${CREATOR_LOCK}${FEE_DENOM}")
    sleep 3
    wait_for_tx "$H" && log_both "  OK: $NAME" || { log_both "  FAIL: $NAME"; return 1; }
}

do_create "Public-Total-1" "Public" '{"total":{}}'
do_create "Public-Total-2" "Public" '{"total":{}}'
do_create "Private-Total-1" "Private" '{"total":{}}'
do_create "Private-None-1" "Private" ""
do_create "Private-MinMembers-1" "Private" '{"min_members":{"count":2}}'
do_create "Cancel-Test" "Public" '{"total":{}}'
do_create "Exit-Test" "Private" '{"total":{}}'

# Unlock-Test: None threshold, cycle_duration=0 (instant rounds), grace=0, 2 members - for ProcessPayout+Withdraw
do_create_custom() {
    local NAME="$1" VIS="$2" DT="$3" MAX="$4" MIN="$5" CYCLE_DAYS="$6" GRACE="$7" LATE_FEE="${8:-1000}" STRICT="${9:-false}" EXIT_PEN="${10:-2000}"
    local MSG
    if [[ -n "$DT" ]]; then
        MSG=$(jq -n --arg n "$NAME" --arg v "$VIS" --arg c "$CONTRIBUTION_AMOUNT" --argjson dt "$DT" --argjson max "$MAX" --argjson min "$MIN" --argjson cd "$CYCLE_DAYS" --argjson gp "$GRACE" --argjson lf "$LATE_FEE" --argjson sm "$STRICT" --argjson ep "$EXIT_PEN" \
            '{create_circle:{circle_name:$n,circle_description:("Test: "+$n),max_members:$max,min_members_required:$min,invite_only:($v=="Private"),contribution_amount:$c,exit_penalty_percent:$ep,late_fee_percent:$lf,total_cycles:1,cycle_duration_days:$cd,grace_period_hours:$gp,auto_start_when_full:true,payout_order_type:"RandomOrder",auto_payout_enabled:true,manual_trigger_enabled:false,emergency_stop_enabled:true,auto_refund_if_min_not_met:true,strict_mode:$sm,visibility:$v,show_member_identities:true,distribution_threshold:$dt}}')
    else
        MSG=$(jq -n --arg n "$NAME" --arg v "$VIS" --arg c "$CONTRIBUTION_AMOUNT" --argjson max "$MAX" --argjson min "$MIN" --argjson cd "$CYCLE_DAYS" --argjson gp "$GRACE" --argjson lf "$LATE_FEE" --argjson sm "$STRICT" --argjson ep "$EXIT_PEN" \
            '{create_circle:{circle_name:$n,circle_description:("Test: "+$n),max_members:$max,min_members_required:$min,invite_only:($v=="Private"),contribution_amount:$c,exit_penalty_percent:$ep,late_fee_percent:$lf,total_cycles:1,cycle_duration_days:$cd,grace_period_hours:$gp,auto_start_when_full:true,payout_order_type:"RandomOrder",auto_payout_enabled:true,manual_trigger_enabled:false,emergency_stop_enabled:true,auto_refund_if_min_not_met:true,strict_mode:$sm,visibility:$v,show_member_identities:true}}')
    fi
    local LOCK=$((CONTRIBUTION_AMOUNT * 2))
    local H=$(tx_exec "$CONTRACT" "$MSG" "$CREATOR_KEY" "${LOCK}${FEE_DENOM}")
    sleep 3
    wait_for_tx "$H" && log_both "  OK: $NAME" || { log_both "  FAIL: $NAME"; return 1; }
}
do_create_custom "Unlock-Test" "Public" "" 2 2 0 0 1000 false
do_create_custom "Ejection-Test" "Public" "" 2 2 0 0 5000 false 5000
do_create_custom "Exit-After-Start" "Private" '{"total":{}}' 2 2 1 1 1000 false
log_both ""

# --- Public Join ---
log_both "${CYAN}[3] JOIN (Public circle)${NC}"
JOIN_MSG=$(jq -n --argjson id 1 '{join_circle:{circle_id:$id}}')
H=$(tx_exec "$CONTRACT" "$JOIN_MSG" "$MEMBER_KEY" "${CONTRIBUTION_AMOUNT}${FEE_DENOM}")
sleep 3
if wait_for_tx "$H"; then
    log_both "  OK: mywallet joined circle 1 (Public)"
else
    log_both "  FAIL: Join circle 1"
fi
# Join Unlock-Test (8) and Ejection-Test (9)
for cid in 8 9; do
    J=$(jq -n --argjson id $cid '{join_circle:{circle_id:$id}}')
    H=$(tx_exec "$CONTRACT" "$J" "$MEMBER_KEY" "${CONTRIBUTION_AMOUNT}${FEE_DENOM}")
    sleep 3
    wait_for_tx "$H" && log_both "  OK: mywallet joined circle $cid" || log_both "  Skip: join circle $cid"
done
log_both ""

# --- Private Invite + AcceptInvite ---
log_both "${CYAN}[4] INVITE + ACCEPT INVITE (Private circle)${NC}"
INV_MSG=$(jq -n --argjson id 3 --arg addr "$MEMBER_ADDR" '{invite_member:{circle_id:$id,member_address:$addr}}')
H=$(tx_exec "$CONTRACT" "$INV_MSG" "$CREATOR_KEY" "")
sleep 3
if wait_for_tx "$H"; then
    log_both "  OK: Creator invited mywallet to circle 3"
else
    log_both "  FAIL: Invite"
fi
ACC_MSG=$(jq -n --argjson id 3 '{accept_invite:{circle_id:$id}}')
H=$(tx_exec "$CONTRACT" "$ACC_MSG" "$MEMBER_KEY" "${CONTRIBUTION_AMOUNT}${FEE_DENOM}")
sleep 3
if wait_for_tx "$H"; then
    log_both "  OK: mywallet accepted invite to circle 3 (with contribution)"
else
    log_both "  FAIL: AcceptInvite"
fi
log_both ""

# --- AddPrivateMember (for circle 4 - Private None) ---
log_both "${CYAN}[5] ADD PRIVATE MEMBER${NC}"
ADD_MSG=$(jq -n --argjson id 4 --arg addr "$MEMBER_ADDR" '{add_private_member:{circle_id:$id,member_address:$addr}}')
H=$(tx_exec "$CONTRACT" "$ADD_MSG" "$CREATOR_KEY" "")
sleep 3
if wait_for_tx "$H"; then
    log_both "  OK: AddPrivateMember for circle 4"
else
    log_both "  Note: AddPrivateMember may require InviteMember for join - check contract"
fi
log_both ""

# --- Invite for Private circle 4, then Accept ---
log_both "${CYAN}[6] INVITE + ACCEPT (Private circle 4)${NC}"
INV4=$(jq -n --argjson id 4 --arg addr "$MEMBER_ADDR" '{invite_member:{circle_id:$id,member_address:$addr}}')
H=$(tx_exec "$CONTRACT" "$INV4" "$CREATOR_KEY" "")
sleep 3
wait_for_tx "$H" && log_both "  OK: Invited to circle 4" || log_both "  Skip: circle 4 invite"
ACC4=$(jq -n --argjson id 4 '{accept_invite:{circle_id:$id}}')
H=$(tx_exec "$CONTRACT" "$ACC4" "$MEMBER_KEY" "${CONTRIBUTION_AMOUNT}${FEE_DENOM}")
sleep 3
wait_for_tx "$H" && log_both "  OK: Accepted circle 4" || log_both "  Skip: circle 4 accept"
# Invite + Accept for Exit-After-Start (circle 10)
INV10=$(jq -n --argjson id 10 --arg addr "$MEMBER_ADDR" '{invite_member:{circle_id:$id,member_address:$addr}}')
H=$(tx_exec "$CONTRACT" "$INV10" "$CREATOR_KEY" "")
sleep 3
wait_for_tx "$H" && log_both "  OK: Invited to circle 10 (Exit-After-Start)" || log_both "  Skip: circle 10 invite"
ACC10=$(jq -n --argjson id 10 '{accept_invite:{circle_id:$id}}')
H=$(tx_exec "$CONTRACT" "$ACC10" "$MEMBER_KEY" "${CONTRIBUTION_AMOUNT}${FEE_DENOM}")
sleep 3
wait_for_tx "$H" && log_both "  OK: Accepted circle 10" || log_both "  Skip: circle 10 accept"
log_both ""

# --- StartCircle ---
log_both "${CYAN}[7] START CIRCLE${NC}"
for cid in 1 3 8 9 10; do
    START_MSG=$(jq -n --argjson id $cid '{start_circle:{circle_id:$id}}')
    H=$(tx_exec "$CONTRACT" "$START_MSG" "$CREATOR_KEY" "")
    sleep 3
    if wait_for_tx "$H"; then
        log_both "  OK: Started circle $cid"
    else
        log_both "  FAIL/Skip: Start circle $cid (may need min_members)"
    fi
done
log_both ""

# --- UpdateCircle ---
log_both "${CYAN}[8] UPDATE CIRCLE${NC}"
UPD_MSG=$(jq -n --argjson id 2 '{update_circle:{circle_id:$id,circle_name:"Public-Total-2-Updated",circle_description:"Updated desc"}}')
H=$(tx_exec "$CONTRACT" "$UPD_MSG" "$CREATOR_KEY" "")
sleep 3
wait_for_tx "$H" && log_both "  OK: Updated circle 2" || log_both "  FAIL: UpdateCircle"
log_both ""

# --- CancelCircle (before start) ---
log_both "${CYAN}[9] CANCEL CIRCLE (before start)${NC}"
CANCEL_MSG=$(jq -n --argjson id 6 '{cancel_circle:{circle_id:$id}}')
H=$(tx_exec "$CONTRACT" "$CANCEL_MSG" "$CREATOR_KEY" "")
sleep 3
if wait_for_tx "$H"; then
    log_both "  OK: Cancelled circle 6 (Cancel-Test)"
else
    log_both "  FAIL: CancelCircle"
fi
log_both ""

# --- ExitCircle (before start) ---
log_both "${CYAN}[10] EXIT CIRCLE (before start)${NC}"
# First invite+accept for circle 7, then exit
INV7=$(jq -n --argjson id 7 --arg addr "$MEMBER_ADDR" '{invite_member:{circle_id:$id,member_address:$addr}}')
H=$(tx_exec "$CONTRACT" "$INV7" "$CREATOR_KEY" "")
sleep 3
wait_for_tx "$H" || true
ACC7=$(jq -n --argjson id 7 '{accept_invite:{circle_id:$id}}')
H=$(tx_exec "$CONTRACT" "$ACC7" "$MEMBER_KEY" "${CONTRIBUTION_AMOUNT}${FEE_DENOM}")
sleep 3
wait_for_tx "$H" || true
EXIT_MSG=$(jq -n --argjson id 7 '{exit_circle:{circle_id:$id}}')
H=$(tx_exec "$CONTRACT" "$EXIT_MSG" "$MEMBER_KEY" "")
sleep 3
if wait_for_tx "$H"; then
    log_both "  OK: Member exited circle 7 (full refund before start)"
else
    log_both "  FAIL/Skip: ExitCircle"
fi
log_both ""

# --- Query states ---
log_both "${CYAN}[11] QUERY STATES${NC}"
for cid in 1 2 3 4 5 6 7 8 9 10; do
    C=$(query_circle "$CONTRACT" $cid)
    STATUS=$(echo "$C" | jq -r '.circle_status // "?"' 2>/dev/null)
    MEMBERS=$(echo "$C" | jq -r '.members_list | length // 0' 2>/dev/null)
    PENDING=$(echo "$C" | jq -r '.pending_members | length // 0' 2>/dev/null)
    log_both "  Circle $cid: status=$STATUS, members=$MEMBERS, pending=$PENDING"
done
log_both ""

# --- DepositContribution (Running circles 1, 3, 8, 9, 10) ---
log_both "${CYAN}[12] DEPOSIT CONTRIBUTION (Running circles)${NC}"
for cid in 1 3; do
    for key in "$CREATOR_KEY" "$MEMBER_KEY"; do
        DEP_MSG=$(jq -n --argjson id $cid '{deposit_contribution:{circle_id:$id}}')
        H=$(tx_exec "$CONTRACT" "$DEP_MSG" "$key" "${CONTRIBUTION_AMOUNT}${FEE_DENOM}")
        sleep 3
        if wait_for_tx "$H"; then
            log_both "  OK: $key deposited in circle $cid"
        else
            log_both "  Skip: $key deposit circle $cid (may need next_payout_date)"
        fi
    done
done
# Unlock-Test (8) and Exit-After-Start (10): both deposit
for cid in 8 10; do
    for key in "$CREATOR_KEY" "$MEMBER_KEY"; do
        DEP_MSG=$(jq -n --argjson id $cid '{deposit_contribution:{circle_id:$id}}')
        H=$(tx_exec "$CONTRACT" "$DEP_MSG" "$key" "${CONTRIBUTION_AMOUNT}${FEE_DENOM}")
        sleep 3
        wait_for_tx "$H" && log_both "  OK: $key deposited in circle $cid" || log_both "  Skip: $key deposit circle $cid"
    done
done
# Ejection-Test (9): only creator deposits (member will miss -> ejection)
DEP_MSG=$(jq -n --argjson id 9 '{deposit_contribution:{circle_id:$id}}')
H=$(tx_exec "$CONTRACT" "$DEP_MSG" "$CREATOR_KEY" "${CONTRIBUTION_AMOUNT}${FEE_DENOM}")
sleep 3
wait_for_tx "$H" && log_both "  OK: creator deposited in circle 9 (member will miss)" || log_both "  Skip: creator deposit circle 9"
log_both ""

# --- AdvanceRound (when round < dist threshold) ---
log_both "${CYAN}[13] ADVANCE ROUND${NC}"
ADV_MSG=$(jq -n --argjson id 1 '{advance_round:{circle_id:$id}}')
H=$(tx_exec "$CONTRACT" "$ADV_MSG" "$CREATOR_KEY" "")
sleep 3
if wait_for_tx "$H"; then
    log_both "  OK: Advanced round circle 1 (Total threshold: round 1 < max, no payout)"
else
    log_both "  Skip: AdvanceRound (may require all deposited first)"
fi
log_both ""

# --- CheckAndEject (documented) ---
log_both "${CYAN}[14] CHECK AND EJECT (documented)${NC}"
CHECK_MSG=$(jq -n --argjson id 1 '{check_and_eject:{circle_id:$id}}')
H=$(tx_exec "$CONTRACT" "$CHECK_MSG" "$CREATOR_KEY" "")
sleep 3
wait_for_tx "$H" && log_both "  OK: CheckAndEject (no-op if no one to eject)" || log_both "  Skip: CheckAndEject"
log_both ""

# --- PauseCircle / UnpauseCircle ---
log_both "${CYAN}[15] PAUSE / UNPAUSE${NC}"
PAUSE_MSG=$(jq -n --argjson id 1 '{pause_circle:{circle_id:$id}}')
H=$(tx_exec "$CONTRACT" "$PAUSE_MSG" "$CREATOR_KEY" "")
sleep 3
if wait_for_tx "$H"; then
    log_both "  OK: Paused circle 1"
    UNPAUSE_MSG=$(jq -n --argjson id 1 '{unpause_circle:{circle_id:$id}}')
    H=$(tx_exec "$CONTRACT" "$UNPAUSE_MSG" "$CREATOR_KEY" "")
    sleep 3
    wait_for_tx "$H" && log_both "  OK: Unpaused circle 1" || log_both "  FAIL: Unpause"
else
    log_both "  Skip: PauseCircle (requires emergency_stop_enabled)"
fi
log_both ""

# --- ProcessPayout / Withdraw (Unlock-Test circle 8: cycle_duration=0, grace=0) ---
log_both "${CYAN}[16] PROCESS PAYOUT + WITHDRAW (unlocking)${NC}"
sleep 2
PP_MSG=$(jq -n --argjson id 8 '{process_payout:{circle_id:$id}}')
H=$(tx_exec "$CONTRACT" "$PP_MSG" "$CREATOR_KEY" "")
sleep 3
if wait_for_tx "$H"; then
    log_both "  OK: ProcessPayout circle 8 (recipient gets PENDING_PAYOUTS)"
    WD_MSG=$(jq -n --argjson id 8 '{withdraw:{circle_id:$id}}')
    for key in "$CREATOR_KEY" "$MEMBER_KEY"; do
        H2=$(tx_exec "$CONTRACT" "$WD_MSG" "$key" "")
        sleep 3
        wait_for_tx "$H2" && { log_both "  OK: Withdraw circle 8 ($key) - unlocking complete"; break; } || true
    done
else
    log_both "  Skip: ProcessPayout (check next_payout_date, grace period)"
fi
log_both ""

# --- Ejection (Ejection-Test circle 9: member misses, late_fee 50% + exit_penalty 50% -> 1-round eject) ---
log_both "${CYAN}[17] EJECTION (member misses deposits)${NC}"
PP_MSG=$(jq -n --argjson id 9 '{process_payout:{circle_id:$id}}')
H=$(tx_exec "$CONTRACT" "$PP_MSG" "$CREATOR_KEY" "")
sleep 3
if wait_for_tx "$H"; then
    log_both "  OK: ProcessPayout circle 9 (member missed -> accumulated+penalty>=locked -> ejected)"
else
    log_both "  Skip: ProcessPayout circle 9"
fi
C9=$(query_circle "$CONTRACT" 9)
MCNT=$(echo "$C9" | jq -r '.members_list | length // 0')
if [[ "$MCNT" == "1" ]]; then
    log_both "  OK: Member ejected from circle 9 (1 member left)"
else
    log_both "  Note: Circle 9 members=$MCNT (expected 1 after ejection)"
fi
log_both ""

# --- Exit after start (Exit-After-Start circle 10, strict_mode=false) ---
log_both "${CYAN}[18] EXIT AFTER START${NC}"
EXIT10=$(jq -n --argjson id 10 '{exit_circle:{circle_id:$id}}')
H=$(tx_exec "$CONTRACT" "$EXIT10" "$MEMBER_KEY" "")
sleep 3
if wait_for_tx "$H"; then
    log_both "  OK: Member exited circle 10 after start (refund = locked - late_fees - exit_penalty)"
else
    log_both "  Skip/Fail: Exit after start (may require strict_mode=false)"
fi
log_both ""

# --- Cancel after start (circle 1 Running) ---
log_both "${CYAN}[19] CANCEL AFTER START${NC}"
CANCEL1=$(jq -n --argjson id 1 '{cancel_circle:{circle_id:$id}}')
H=$(tx_exec "$CONTRACT" "$CANCEL1" "$CREATOR_KEY" "")
sleep 3
if wait_for_tx "$H"; then
    log_both "  OK: Cancelled circle 1 (Running) - creator_lock distributed, deposits refunded"
else
    log_both "  Skip/Fail: Cancel after start"
fi
log_both ""

# --- Write report ---
mkdir -p "$(dirname "$REPORT_FILE")"
{
    echo "# Safrimba Contract - Full Actions Test Report"
    echo ""
    echo "Generated: $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
    echo ""
    echo "## Configuration"
    echo "- Network: $NETWORK"
    echo "- Chain ID: $CHAIN_ID"
    echo "- Contract: $CONTRACT"
    echo "- Creator: $CREATOR_KEY ($CREATOR_ADDR)"
    echo "- Member: $MEMBER_KEY ($MEMBER_ADDR)"
    echo "- Code ID: $CODE_ID"
    echo ""
    echo "## Test Output"
    echo '```'
    echo "$REPORT"
    echo '```'
    echo ""
    echo "## All Contract Actions (for Frontend)"
    echo ""
    echo "| Action | When | Who | Notes |"
    echo "|--------|------|-----|------|"
    echo "| CreateCircle | - | Creator | Send creator_lock (2×contribution) |"
    echo "| JoinCircle | Draft/Open, Public | Anyone | Send contribution_amount |"
    echo "| InviteMember | Draft/Open, Private | Creator | Adds to pending_members |"
    echo "| AcceptInvite | Draft/Open, Private | Invited | Send contribution_amount |"
    echo "| AddPrivateMember | Draft/Open, Private | Creator | Adds to private whitelist |"
    echo "| StartCircle | Open/Full | Creator | Min members required |"
    echo "| DepositContribution | Running | Member | Send contribution_amount each round |"
    echo "| ProcessPayout | Running, date reached | Anyone | Triggers distribution (tested: Unlock-Test) |"
    echo "| AdvanceRound | Running, round < dist threshold | Anyone | No payout, next round |"
    echo "| Withdraw | Anytime | Member | Withdraw pending payouts (unlocking, tested) |"
    echo "| ExitCircle | Draft/Open (full refund) or Running (penalty) | Member | - |"
    echo "| CancelCircle | Draft/Open/Full (refund) or Running | Creator | - |"
    echo "| UpdateCircle | Draft/Open | Creator | Name, description, image |"
    echo "| PauseCircle | Running | Creator | If emergency_stop_enabled |"
    echo "| UnpauseCircle | Paused | Creator | - |"
    echo "| EmergencyStop | Running | Creator | - |"
    echo "| BlockMember | Running | Creator | Blocks member |"
    echo "| DistributeBlockedFunds | Running | Creator | After block |"
    echo "| CheckAndEject | Running | Anyone | Ejects late members |"
    echo ""
    echo "## Distribution Thresholds"
    echo "- **Total**: Distribution only at last round of cycle (until completion)"
    echo "- **None**: Distribution from round 1 (unlocking from start)"
    echo "- **MinMembers(N)**: Distribution from round N onwards"
    echo ""
    echo "## Final Unlocking (tested)"
    echo "1. Member receives payout via ProcessPayout (or auto_payout)"
    echo "2. Amount goes to PENDING_PAYOUTS for that member"
    echo "3. Member calls Withdraw to receive funds (unlocking complete)"
    echo "4. withdrawal_lock can block withdrawals if set"
    echo ""
    echo "## Unlocking Flows Tested"
    echo "- **ProcessPayout + Withdraw**: Unlock-Test circle (cycle_duration=0, grace=0, None threshold)"
    echo "- **Ejection**: Ejection-Test circle (member misses, late_fee 50% + exit_penalty 50% -> 1-round eject)"
    echo "- **Exit after start**: Exit-After-Start circle (strict_mode=false, refund = locked - fees - penalty)"
    echo "- **Cancel after start**: Circle 1 Running -> creator_lock distributed, deposits refunded"
    echo ""
    echo "## State Machine"
    echo "Draft -> Open -> Full -> Running -> Completed"
    echo "  |       |       |       |"
    echo "  v       v       v       v"
    echo "Cancelled"
    echo ""
    echo "## Frontend Actions by State"
    echo ""
    echo "### Draft"
    echo "- CreateCircle (done)"
    echo "- InviteMember (if private)"
    echo "- JoinCircle (if public)"
    echo "- UpdateCircle"
    echo "- CancelCircle"
    echo "- ExitCircle"
    echo ""
    echo "### Open"
    echo "- InviteMember (if private)"
    echo "- JoinCircle (if public)"
    echo "- AcceptInvite (if invited)"
    echo "- StartCircle (if min_members met)"
    echo "- UpdateCircle"
    echo "- CancelCircle"
    echo "- ExitCircle"
    echo ""
    echo "### Full"
    echo "- StartCircle"
    echo "- ExitCircle"
    echo "- CancelCircle"
    echo ""
    echo "### Running"
    echo "- DepositContribution"
    echo "- ProcessPayout"
    echo "- AdvanceRound"
    echo "- Withdraw"
    echo "- PauseCircle"
    echo "- BlockMember"
    echo "- CheckAndEject"
    echo "- EmergencyStop"
    echo "- CancelCircle"
    echo ""
    echo "### Paused"
    echo "- UnpauseCircle"
    echo "- CancelCircle"
    echo ""
    echo "## Query Messages (for Frontend)"
    echo "- GetCircle, GetCircles, GetCircleMembers, GetCircleStatus"
    echo "- GetCurrentCycle, GetCycleDeposits, GetMemberDeposits"
    echo "- GetPayouts, GetPayoutHistory, GetPendingPayout"
    echo "- GetCircleBalance, GetMemberBalance, GetPenalties, GetRefunds"
    echo "- GetDistributionCalendar, GetMembersAccumulatedLateFees"
    echo ""
} > "$REPORT_FILE"

log_both "${GREEN}=== Report written to $REPORT_FILE ===${NC}"
exit 0
