#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
NETWORK="${1:-testnet}"
KEY_NAME="${2:-mycontractadmin}"
CODE_ID="${3:-91}"
DEPLOY_ON_ERROR="${DEPLOY_ON_ERROR:-true}"

# Validate network
if [[ "$NETWORK" != "testnet" && "$NETWORK" != "mainnet" ]]; then
    echo -e "${RED}Error: Network must be 'testnet' or 'mainnet'${NC}"
    echo "Usage: $0 [testnet|mainnet] [key_name] [code_id]"
    echo "  Set DEPLOY_ON_ERROR=false to skip deploy-on-error (default: true)"
    exit 1
fi

# Paths
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONTRACT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CHAIN_CONFIG="$CONTRACT_DIR/chain/$NETWORK/safrochain.json"

# Check if chain config exists
if [[ ! -f "$CHAIN_CONFIG" ]]; then
    echo -e "${RED}Error: Chain config not found at $CHAIN_CONFIG${NC}"
    exit 1
fi

# Extract chain info from config
CHAIN_ID=$(jq -r '.chainId' "$CHAIN_CONFIG")
RPC_URL=$(jq -r '.rpc' "$CHAIN_CONFIG")
DENOM=$(jq -r '.feeCurrencies[0].coinMinimalDenom' "$CHAIN_CONFIG")

# Use usaf for testnet (balances typically in usaf)
FEE_DENOM="${DENOM}"
if [[ "$DENOM" == "saf" && "$NETWORK" == "testnet" ]]; then
    FEE_DENOM="usaf"
fi

# Test params: minimal timing, small circles
CONTRIBUTION_AMOUNT="100000"
# Creator lock = contribution * 2 (from compute_creator_lock)
CREATOR_LOCK=$((CONTRIBUTION_AMOUNT * 2))

echo -e "${GREEN}=== Distribution Threshold Full Test Suite ===${NC}"
echo -e "Network: ${YELLOW}$NETWORK${NC}"
echo -e "Chain ID: ${YELLOW}$CHAIN_ID${NC}"
echo -e "Code ID: ${YELLOW}$CODE_ID${NC}"
echo -e "Key Name: ${YELLOW}$KEY_NAME${NC}"
echo -e "Creator lock per circle: ${YELLOW}${CREATOR_LOCK}${FEE_DENOM}${NC}"
echo ""

# Check if safrochaind and jq are installed
if ! command -v safrochaind &> /dev/null; then
    echo -e "${RED}Error: safrochaind command not found. Please install safrochaind.${NC}"
    exit 1
fi
if ! command -v jq &> /dev/null; then
    echo -e "${RED}Error: jq command not found. Please install jq.${NC}"
    exit 1
fi

# Get key address (try os keyring first, then default)
KEY_ADDRESS=$(safrochaind keys show "$KEY_NAME" -a --keyring-backend os 2>/dev/null || \
    safrochaind keys show "$KEY_NAME" -a 2>/dev/null || echo "")
# Use OS keyring (user wallets are in keyring-backend os)
KEYRING_OPTS="--keyring-backend os"
if [[ -z "$KEY_ADDRESS" ]]; then
    echo -e "${RED}Error: Key '$KEY_NAME' not found${NC}"
    echo "Available keys:"
    safrochaind keys list --keyring-backend os 2>/dev/null || safrochaind keys list 2>/dev/null || true
    exit 1
fi

echo -e "Key Address: ${BLUE}$KEY_ADDRESS${NC}"

# Check account balance
ALL_BALANCES_JSON=$(safrochaind query bank balances "$KEY_ADDRESS" --node "$RPC_URL" --output json 2>/dev/null || echo '{"balances":[]}')
BALANCE=$(echo "$ALL_BALANCES_JSON" | jq -r ".balances[] | select(.denom == \"$FEE_DENOM\") | .amount" 2>/dev/null || echo "0")
if [[ -z "$BALANCE" || "$BALANCE" == "null" ]]; then
    BALANCE=$(echo "$ALL_BALANCES_JSON" | jq -r ".balances[] | select(.denom == \"usaf\") | .amount" 2>/dev/null || echo "0")
fi
if [[ -z "$BALANCE" || "$BALANCE" == "null" ]]; then
    BALANCE="0"
fi

REQUIRED_TOTAL=$((CREATOR_LOCK * 8))
echo -e "Balance: ${YELLOW}${BALANCE}${FEE_DENOM}${NC} (need ~${REQUIRED_TOTAL} for 8 circles + gas)"
if (( BALANCE < REQUIRED_TOTAL )); then
    echo -e "${YELLOW}Warning: Balance may be low for 8 circles. Proceeding anyway.${NC}"
fi
echo ""

# ---------------------------------------------------------------------------
# Helper: run deploy on error, optionally update CODE_ID from deployment
# ---------------------------------------------------------------------------
run_deploy_on_error() {
    if [[ "$DEPLOY_ON_ERROR" != "true" ]]; then
        return 1
    fi
    echo -e "${YELLOW}Running deploy script...${NC}"
    (cd "$CONTRACT_DIR" && ./scripts/deploy.sh "$NETWORK") || return 1
    # Use new code_id from deployment if available
    local DEPLOY_JSON="$CONTRACT_DIR/deployment-$NETWORK.json"
    if [[ -f "$DEPLOY_JSON" ]]; then
        local NEW_CODE=$(jq -r '.codeId // empty' "$DEPLOY_JSON" 2>/dev/null)
        if [[ -n "$NEW_CODE" && "$NEW_CODE" != "null" ]]; then
            CODE_ID="$NEW_CODE"
            echo -e "${YELLOW}Using code_id from deployment: $CODE_ID${NC}"
        fi
    fi
    return 0
}

# ---------------------------------------------------------------------------
# Helper: wait for tx and extract success
# ---------------------------------------------------------------------------
wait_for_tx() {
    local TX_HASH="$1"
    local MAX_ATTEMPTS="${2:-30}"
    local ATTEMPT=1

    while [[ $ATTEMPT -le $MAX_ATTEMPTS ]]; do
        TX_RESULT=$(safrochaind query tx "$TX_HASH" --node "$RPC_URL" --output json 2>/dev/null || echo "")
        if echo "$TX_RESULT" | jq -e '.code == 0' >/dev/null 2>&1; then
            return 0
        fi
        if echo "$TX_RESULT" | jq -e '.code' >/dev/null 2>&1; then
            local CODE=$(echo "$TX_RESULT" | jq -r '.code')
            if [[ "$CODE" != "0" && "$CODE" != "null" ]]; then
                echo "$TX_RESULT" | jq -r '.raw_log' 2>/dev/null || true
                return 1
            fi
        fi
        sleep 2
        ATTEMPT=$((ATTEMPT + 1))
    done
    return 1
}

# ---------------------------------------------------------------------------
# Helper: instantiate contract and return address
# ---------------------------------------------------------------------------
do_instantiate() {
    local INSTANTIATE_MSG=$(jq -n \
        --arg addr "$KEY_ADDRESS" \
        '{"platform_fee_percent":0,"platform_address":$addr}')

    echo -e "${YELLOW}Instantiating contract (code_id=$CODE_ID)...${NC}" >&2
    local OUTPUT
    OUTPUT=$(safrochaind tx wasm instantiate "$CODE_ID" "$INSTANTIATE_MSG" \
        --from "$KEY_NAME" \
        --admin "$KEY_ADDRESS" \
        --label "safrimba-dt-test-$(date +%s)" \
        --chain-id "$CHAIN_ID" \
        --node "$RPC_URL" \
        --broadcast-mode sync \
        --gas auto \
        --gas-adjustment 1.4 \
        --gas-prices "0.025${FEE_DENOM}" \
        $KEYRING_OPTS \
        -y --output json 2>&1) || true

    local TX_HASH=$(echo "$OUTPUT" | jq -r '.txhash // empty' 2>/dev/null)
    if [[ -z "$TX_HASH" ]]; then
        TX_HASH=$(echo "$OUTPUT" | grep -oE '[A-F0-9]{64}' | head -1 || echo "")
    fi
    if [[ -z "$TX_HASH" ]]; then
        echo -e "${RED}Instantiate failed: could not get tx hash${NC}" >&2
        echo "$OUTPUT" >&2
        return 1
    fi

    sleep 5
    if ! wait_for_tx "$TX_HASH"; then
        echo -e "${RED}Instantiate tx failed${NC}" >&2
        return 1
    fi

    TX_RESULT=$(safrochaind query tx "$TX_HASH" --node "$RPC_URL" --output json 2>/dev/null)
    local ADDR=$(echo "$TX_RESULT" | jq -r '.events[] | select(.type == "instantiate") | .attributes[] | select(.key == "_contract_address") | .value' 2>/dev/null | head -1)
    if [[ -z "$ADDR" || "$ADDR" == "null" ]]; then
        ADDR=$(echo "$TX_RESULT" | jq -r '.logs[].events[]? | select(.type == "instantiate") | .attributes[]? | select(.key == "_contract_address") | .value' 2>/dev/null | head -1)
    fi
    # Only output the address to stdout (for capture) - no ANSI codes
    printf '%s' "$ADDR"
}

# ---------------------------------------------------------------------------
# Helper: execute CreateCircle and verify
# ---------------------------------------------------------------------------
do_create_circle() {
    local CONTRACT_ADDR="$1"
    local NAME="$2"
    local VISIBILITY="$3"
    local DIST_THRESHOLD_JSON="$4"  # JSON for distribution_threshold or empty to omit

    local CREATE_MSG
    if [[ -n "$DIST_THRESHOLD_JSON" ]]; then
        CREATE_MSG=$(jq -n \
            --arg name "$NAME" \
            --arg vis "$VISIBILITY" \
            --argjson dt "$DIST_THRESHOLD_JSON" \
            --arg contrib "$CONTRIBUTION_AMOUNT" \
            '{
              create_circle: {
                circle_name: $name,
                circle_description: ("Distribution threshold test: " + $name),
                max_members: 3,
                min_members_required: 2,
                invite_only: (if $vis == "Private" then true else false end),
                contribution_amount: $contrib,
                exit_penalty_percent: 2000,
                late_fee_percent: 1000,
                total_cycles: 1,
                cycle_duration_days: 1,
                grace_period_hours: 1,
                auto_start_when_full: true,
                payout_order_type: "RandomOrder",
                auto_payout_enabled: true,
                manual_trigger_enabled: false,
                emergency_stop_enabled: false,
                auto_refund_if_min_not_met: true,
                strict_mode: false,
                visibility: $vis,
                show_member_identities: true,
                distribution_threshold: $dt
              }
            }')
    else
        CREATE_MSG=$(jq -n \
            --arg name "$NAME" \
            --arg vis "$VISIBILITY" \
            --arg contrib "$CONTRIBUTION_AMOUNT" \
            '{
              create_circle: {
                circle_name: $name,
                circle_description: ("Distribution threshold test: " + $name),
                max_members: 3,
                min_members_required: 2,
                invite_only: (if $vis == "Private" then true else false end),
                contribution_amount: $contrib,
                exit_penalty_percent: 2000,
                late_fee_percent: 1000,
                total_cycles: 1,
                cycle_duration_days: 1,
                grace_period_hours: 1,
                auto_start_when_full: true,
                payout_order_type: "RandomOrder",
                auto_payout_enabled: true,
                manual_trigger_enabled: false,
                emergency_stop_enabled: false,
                auto_refund_if_min_not_met: true,
                strict_mode: false,
                visibility: $vis,
                show_member_identities: true
              }
            }')
    fi

    echo -e "  Creating: ${BLUE}$NAME${NC} (visibility=$VISIBILITY)"
    local OUTPUT
    OUTPUT=$(safrochaind tx wasm execute "$CONTRACT_ADDR" "$CREATE_MSG" \
        --from "$KEY_NAME" \
        --amount "${CREATOR_LOCK}${FEE_DENOM}" \
        --chain-id "$CHAIN_ID" \
        --node "$RPC_URL" \
        --broadcast-mode sync \
        --gas auto \
        --gas-adjustment 1.4 \
        --gas-prices "0.025${FEE_DENOM}" \
        $KEYRING_OPTS \
        -y --output json 2>&1) || true

    local TX_HASH=$(echo "$OUTPUT" | jq -r '.txhash // empty' 2>/dev/null)
    if [[ -z "$TX_HASH" ]]; then
        TX_HASH=$(echo "$OUTPUT" | grep -oE '[A-F0-9]{64}' | head -1 || echo "")
    fi
    if [[ -z "$TX_HASH" ]]; then
        echo -e "    ${RED}Failed: no tx hash${NC}"
        echo "$OUTPUT" | head -20
        return 1
    fi

    sleep 3
    if ! wait_for_tx "$TX_HASH"; then
        echo -e "    ${RED}CreateCircle tx failed${NC}"
        return 1
    fi
    echo -e "    ${GREEN}OK${NC} (tx: ${TX_HASH:0:16}...)"
    return 0
}

# ---------------------------------------------------------------------------
# Helper: verify circle distribution_threshold via query
# ---------------------------------------------------------------------------
verify_circle() {
    local CONTRACT_ADDR="$1"
    local CIRCLE_ID="$2"
    local EXPECTED="$3"  # "total", "min_members", or "none"

    local QUERY=$(jq -n --argjson id "$CIRCLE_ID" '{get_circle:{circle_id:$id}}')
    local RESULT
    RESULT=$(safrochaind query wasm contract-state smart "$CONTRACT_ADDR" "$QUERY" --node "$RPC_URL" --output json 2>/dev/null) || return 1

    # Response may be .circle or .data.circle depending on SDK version
    local ACTUAL=$(echo "$RESULT" | jq -r '.circle.distribution_threshold // .data.circle.distribution_threshold // empty' 2>/dev/null)

    # Normalize: contract returns {"total":null} or {"min_members":{"count":N}} or null
    local NORMALIZED="none"
    if [[ -n "$ACTUAL" && "$ACTUAL" != "null" ]]; then
        if echo "$ACTUAL" | jq -e 'has("total")' >/dev/null 2>&1; then
            NORMALIZED="total"
        elif echo "$ACTUAL" | jq -e 'has("min_members")' >/dev/null 2>&1; then
            NORMALIZED="min_members"
        fi
    fi

    if [[ "$NORMALIZED" != "$EXPECTED" ]]; then
        echo -e "    ${RED}Verify failed: expected $EXPECTED, got $NORMALIZED (raw: $ACTUAL)${NC}"
        return 1
    fi
    return 0
}

# ---------------------------------------------------------------------------
# Main: Run full test suite
# ---------------------------------------------------------------------------
CONTRACT_ADDRESS=""
CIRCLE_ID=1

# Step 1: Instantiate (with retry on error)
echo -e "${GREEN}[1/4] Instantiate contract${NC}"
CONTRACT_ADDRESS=$(do_instantiate) || true
if [[ -z "$CONTRACT_ADDRESS" || "$CONTRACT_ADDRESS" == "null" ]]; then
    if run_deploy_on_error; then
        echo -e "${YELLOW}Retrying instantiate after deploy...${NC}"
        CONTRACT_ADDRESS=$(do_instantiate) || true
    fi
    if [[ -z "$CONTRACT_ADDRESS" || "$CONTRACT_ADDRESS" == "null" ]]; then
        echo -e "${RED}Failed to instantiate contract${NC}"
        exit 1
    fi
fi
echo -e "${GREEN}Contract: ${BLUE}$CONTRACT_ADDRESS${NC}"
echo ""

# Step 2: Create circles - Total (2 public + 2 private)
echo -e "${GREEN}[2/4] Distribution Threshold: Total (100% - until completion)${NC}"
for i in 1 2; do
    do_create_circle "$CONTRACT_ADDRESS" "DT-Total-Public-$i" "Public" '{"total":{}}' || exit 1
    CIRCLE_ID=$((CIRCLE_ID + 1))
done
for i in 1 2; do
    do_create_circle "$CONTRACT_ADDRESS" "DT-Total-Private-$i" "Private" '{"total":{}}' || exit 1
    CIRCLE_ID=$((CIRCLE_ID + 1))
done
echo ""

# Step 3: Create circles - None (2 private only; Public forces Total)
echo -e "${GREEN}[3/4] Distribution Threshold: None (round 1 - unlocking from start)${NC}"
for i in 1 2; do
    do_create_circle "$CONTRACT_ADDRESS" "DT-None-Private-$i" "Private" "" || exit 1
    CIRCLE_ID=$((CIRCLE_ID + 1))
done
echo ""

# Step 4: Create circles - MinMembers (2 private only)
echo -e "${GREEN}[4/4] Distribution Threshold: MinMembers (from round 2)${NC}"
for i in 1 2; do
    do_create_circle "$CONTRACT_ADDRESS" "DT-MinMembers-Private-$i" "Private" '{"min_members":{"count":2}}' || exit 1
    CIRCLE_ID=$((CIRCLE_ID + 1))
done
echo ""

# Step 5: Verify all circles
echo -e "${GREEN}[5/5] Verifying distribution_threshold for all circles${NC}"
CIRCLE_ID=1
# Circles 1-2: Public Total -> total
# Circles 3-4: Private Total -> total
for i in 1 2 3 4; do
    echo -n "  Circle $CIRCLE_ID (expected total): "
    verify_circle "$CONTRACT_ADDRESS" "$CIRCLE_ID" "total" && echo -e "${GREEN}OK${NC}" || exit 1
    CIRCLE_ID=$((CIRCLE_ID + 1))
done
# Circles 5-6: Private None -> none
for i in 1 2; do
    echo -n "  Circle $CIRCLE_ID (expected none): "
    verify_circle "$CONTRACT_ADDRESS" "$CIRCLE_ID" "none" && echo -e "${GREEN}OK${NC}" || exit 1
    CIRCLE_ID=$((CIRCLE_ID + 1))
done
# Circles 7-8: Private MinMembers -> min_members
for i in 1 2; do
    echo -n "  Circle $CIRCLE_ID (expected min_members): "
    verify_circle "$CONTRACT_ADDRESS" "$CIRCLE_ID" "min_members" && echo -e "${GREEN}OK${NC}" || exit 1
    CIRCLE_ID=$((CIRCLE_ID + 1))
done
echo ""

echo -e "${GREEN}=== Test Summary ===${NC}"
echo -e "Contract: ${BLUE}$CONTRACT_ADDRESS${NC}"
echo -e "Circles created: ${GREEN}8${NC} (2 public Total, 2 private Total, 2 private None, 2 private MinMembers)"
echo -e "${GREEN}All distribution threshold tests passed!${NC}"
exit 0
