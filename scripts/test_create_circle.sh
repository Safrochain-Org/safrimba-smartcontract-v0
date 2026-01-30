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
CODE_ID="${3:-66}"  # Use the deployed code ID

# Validate network
if [[ "$NETWORK" != "testnet" && "$NETWORK" != "mainnet" ]]; then
    echo -e "${RED}Error: Network must be 'testnet' or 'mainnet'${NC}"
    echo "Usage: $0 [testnet|mainnet] [key_name] [code_id]"
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

# Check for alternative denominations
ALT_DENOM="usaf"

echo -e "${GREEN}=== Testing Safrimba Contract CreateCircle ===${NC}"
echo -e "Network: ${YELLOW}$NETWORK${NC}"
echo -e "Chain ID: ${YELLOW}$CHAIN_ID${NC}"
echo -e "Code ID: ${YELLOW}$CODE_ID${NC}"
echo -e "Key Name: ${YELLOW}$KEY_NAME${NC}"
echo ""

# Check if safrochaind is installed
if ! command -v safrochaind &> /dev/null; then
    echo -e "${RED}Error: safrochaind command not found. Please install safrochaind.${NC}"
    exit 1
fi

# Get key address - try with different methods
KEY_ADDRESS=$(safrochaind keys show "$KEY_NAME" --address 2>/dev/null || echo "")
if [[ -z "$KEY_ADDRESS" ]]; then
    # Try to get from deploy script default
    if [[ "$KEY_NAME" == "mycontractadmin" ]]; then
        KEY_ADDRESS="addr_safro1x25weznnzd5k6jv663sdldehqwcjatc44gvrvq"
        echo -e "${YELLOW}Warning: Key '$KEY_NAME' not found, using default address${NC}"
    else
        echo -e "${RED}Error: Key '$KEY_NAME' not found${NC}"
        echo "Available keys:"
        safrochaind keys list 2>&1 | head -10 || echo "Could not list keys"
        exit 1
    fi
fi

echo -e "Key Address: ${BLUE}$KEY_ADDRESS${NC}"
echo ""

# Check account balance to determine which denomination to use
echo "Checking account balance..."
ALL_BALANCES_JSON=$(safrochaind query bank balances "$KEY_ADDRESS" --node "$RPC_URL" --output json 2>/dev/null || echo '{"balances":[]}')

# Check if we have balance in usaf or saf
HAS_USAF=$(echo "$ALL_BALANCES_JSON" | jq -r '.balances[] | select(.denom == "usaf") | .amount' 2>/dev/null | head -1 || echo "")
HAS_SAF=$(echo "$ALL_BALANCES_JSON" | jq -r '.balances[] | select(.denom == "saf") | .amount' 2>/dev/null | head -1 || echo "")

# Determine which denomination to use for fees
if [[ -n "$HAS_USAF" && "$HAS_USAF" != "0" && "$HAS_USAF" != "null" ]]; then
    FEE_DENOM="usaf"
    echo "Note: Using usaf denomination (found balance in usaf, not saf)"
elif [[ -n "$HAS_SAF" && "$HAS_SAF" != "0" && "$HAS_SAF" != "null" ]]; then
    FEE_DENOM="saf"
    echo "Note: Using saf denomination"
else
    # Default to usaf for testnet
    FEE_DENOM="usaf"
    echo "Note: No balance found, defaulting to usaf for testnet"
fi

echo "Using denomination for fees: $FEE_DENOM"
echo ""

# Step 1: Instantiate the contract first
echo -e "${YELLOW}[1/3] Instantiating contract...${NC}"

# Create instantiate message
INSTANTIATE_MSG=$(cat <<EOF
{
  "platform_fee_percent": 0,
  "platform_address": "$KEY_ADDRESS"
}
EOF
)

echo "Instantiate message:"
echo "$INSTANTIATE_MSG" | jq '.'
echo ""

# Instantiate the contract
INSTANTIATE_OUTPUT=$(safrochaind tx wasm instantiate "$CODE_ID" "$INSTANTIATE_MSG" \
    --from "$KEY_NAME" \
    --admin "$KEY_ADDRESS" \
    --label "safrimba-test-$(date +%s)" \
    --chain-id "$CHAIN_ID" \
    --node "$RPC_URL" \
    --broadcast-mode sync \
    --gas auto \
    --gas-adjustment 1.4 \
    --gas-prices "0.025$FEE_DENOM" \
    -y 2>&1)

echo "Instantiate output:"
echo "$INSTANTIATE_OUTPUT"
echo ""

# Check if transaction failed immediately
INSTANTIATE_CODE=$(echo "$INSTANTIATE_OUTPUT" | jq -r '.code' 2>/dev/null || echo "")
if [[ -n "$INSTANTIATE_CODE" && "$INSTANTIATE_CODE" != "0" && "$INSTANTIATE_CODE" != "null" ]]; then
    RAW_LOG=$(echo "$INSTANTIATE_OUTPUT" | jq -r '.raw_log' 2>/dev/null || echo "")
    echo -e "${RED}✗ Transaction failed immediately with code: $INSTANTIATE_CODE${NC}"
    echo "Error: $RAW_LOG"
    exit 1
fi

# Extract transaction hash
TX_HASH=$(echo "$INSTANTIATE_OUTPUT" | grep -oE '"txhash":"[A-F0-9]{64}"' | cut -d'"' -f4 || echo "")
if [[ -z "$TX_HASH" ]]; then
    TX_HASH=$(echo "$INSTANTIATE_OUTPUT" | grep -i "txhash" | grep -oE '[A-F0-9]{64}' | head -1 || echo "")
fi

if [[ -z "$TX_HASH" ]]; then
    echo -e "${RED}Error: Could not extract transaction hash from instantiate output${NC}"
    echo "Full output:"
    echo "$INSTANTIATE_OUTPUT"
    exit 1
fi

echo -e "Transaction hash: ${BLUE}$TX_HASH${NC}"
echo "Waiting for transaction to be included in block..."
sleep 5

# Query transaction to get contract address
MAX_ATTEMPTS=30
ATTEMPT=1
CONTRACT_ADDRESS=""

while [[ $ATTEMPT -le $MAX_ATTEMPTS ]]; do
    echo "Checking transaction status (attempt $ATTEMPT/$MAX_ATTEMPTS)..."
    
    TX_RESULT=$(safrochaind query tx "$TX_HASH" --node "$RPC_URL" --output json 2>&1 || echo "")
    
    if echo "$TX_RESULT" | jq -e '.code == 0' >/dev/null 2>&1; then
        # Extract contract address from events
        CONTRACT_ADDRESS=$(echo "$TX_RESULT" | jq -r '.events[] | select(.type == "instantiate") | .attributes[] | select(.key == "_contract_address") | .value' 2>/dev/null | head -1 || echo "")
        
        if [[ -n "$CONTRACT_ADDRESS" && "$CONTRACT_ADDRESS" != "null" ]]; then
            echo -e "${GREEN}Contract instantiated successfully!${NC}"
            echo -e "Contract Address: ${BLUE}$CONTRACT_ADDRESS${NC}"
            break
        fi
    elif echo "$TX_RESULT" | jq -e '.code' >/dev/null 2>&1; then
        CODE=$(echo "$TX_RESULT" | jq -r '.code')
        RAW_LOG=$(echo "$TX_RESULT" | jq -r '.raw_log' 2>/dev/null || echo "")
        if [[ "$CODE" != "0" ]]; then
            echo -e "${RED}✗ Transaction failed with code: $CODE${NC}"
            echo "Error: $RAW_LOG"
            exit 1
        fi
    fi
    
    if [[ $ATTEMPT -eq $MAX_ATTEMPTS ]]; then
        echo -e "${RED}Error: Transaction not confirmed after $MAX_ATTEMPTS attempts${NC}"
        echo "Transaction hash: $TX_HASH"
        exit 1
    fi
    
    sleep 2
    ATTEMPT=$((ATTEMPT + 1))
done

if [[ -z "$CONTRACT_ADDRESS" ]]; then
    echo -e "${RED}Error: Could not extract contract address${NC}"
    exit 1
fi

echo ""

# Step 2: Test CreateCircle execute message
echo -e "${YELLOW}[2/3] Testing CreateCircle execute message...${NC}"

# Create the execute message (matching frontend format - omitting optional fields)
# Using compact JSON format to avoid shell escaping issues with multi-line strings
CREATE_CIRCLE_MSG='{"create_circle":{"circle_name":"Test Circle","circle_description":"This is a test circle for contract testing","max_members":5,"min_members_required":3,"invite_only":false,"contribution_amount":"100000000","penalty_fee_amount":"10000000","late_fee_amount":"5000000","total_cycles":1,"cycle_duration_days":30,"grace_period_hours":72,"auto_start_when_full":true,"payout_order_type":"RandomOrder","auto_payout_enabled":true,"manual_trigger_enabled":false,"emergency_stop_enabled":false,"auto_refund_if_min_not_met":true,"max_missed_payments_allowed":3,"strict_mode":false,"member_exit_allowed_before_start":true,"visibility":"Public","show_member_identities":true}}'

echo "CreateCircle message:"
echo "$CREATE_CIRCLE_MSG" | jq '.' 2>/dev/null || echo "$CREATE_CIRCLE_MSG"
echo ""

# Execute the contract
echo "Executing CreateCircle..."
echo "This may take a moment..."

# Use direct JSON string (compact format to avoid shell escaping issues)
# Use timeout to prevent hanging (60 seconds timeout for execute)
if command -v timeout >/dev/null 2>&1; then
    EXECUTE_OUTPUT=$(timeout 60 safrochaind tx wasm execute "$CONTRACT_ADDRESS" "$CREATE_CIRCLE_MSG" \
        --from "$KEY_NAME" \
        --chain-id "$CHAIN_ID" \
        --node "$RPC_URL" \
        --broadcast-mode sync \
        --gas auto \
        --gas-adjustment 1.4 \
        --gas-prices "0.025$FEE_DENOM" \
        -y 2>&1) || EXECUTE_OUTPUT="TIMEOUT_ERROR"
else
    # Fallback if timeout command not available
    EXECUTE_OUTPUT=$(safrochaind tx wasm execute "$CONTRACT_ADDRESS" "$CREATE_CIRCLE_MSG" \
        --from "$KEY_NAME" \
        --chain-id "$CHAIN_ID" \
        --node "$RPC_URL" \
        --broadcast-mode sync \
        --gas auto \
        --gas-adjustment 1.4 \
        --gas-prices "0.025$FEE_DENOM" \
        -y 2>&1)
fi

# Check for timeout
if [[ "$EXECUTE_OUTPUT" == "TIMEOUT_ERROR" ]]; then
    echo -e "${RED}Error: Execute command timed out after 60 seconds${NC}"
    echo "The command may be hanging. Please check your network connection and RPC endpoint."
    exit 1
fi

echo "Execute output:"
echo "$EXECUTE_OUTPUT"
echo ""

# Check if transaction failed immediately
EXEC_CODE=$(echo "$EXECUTE_OUTPUT" | jq -r '.code' 2>/dev/null || echo "")
if [[ -n "$EXEC_CODE" && "$EXEC_CODE" != "0" && "$EXEC_CODE" != "null" ]]; then
    RAW_LOG=$(echo "$EXECUTE_OUTPUT" | jq -r '.raw_log' 2>/dev/null || echo "")
    echo -e "${RED}✗ Transaction failed immediately with code: $EXEC_CODE${NC}"
    echo "Error: $RAW_LOG"
    exit 1
fi

# Extract transaction hash
EXEC_TX_HASH=$(echo "$EXECUTE_OUTPUT" | grep -oE '"txhash":"[A-F0-9]{64}"' | cut -d'"' -f4 || echo "")
if [[ -z "$EXEC_TX_HASH" ]]; then
    EXEC_TX_HASH=$(echo "$EXECUTE_OUTPUT" | grep -i "txhash" | grep -oE '[A-F0-9]{64}' | head -1 || echo "")
fi

if [[ -z "$EXEC_TX_HASH" ]]; then
    echo -e "${RED}Error: Could not extract transaction hash from execute output${NC}"
    echo -e "${RED}This likely means the execute message failed${NC}"
    echo "Full output:"
    echo "$EXECUTE_OUTPUT"
    exit 1
fi

echo -e "Transaction hash: ${BLUE}$EXEC_TX_HASH${NC}"
echo "Waiting for transaction to be included in block..."
echo "This may take 10-30 seconds. Please wait..."
sleep 5

# Step 3: Verify the transaction
echo -e "${YELLOW}[3/3] Verifying transaction...${NC}"

MAX_ATTEMPTS=30
ATTEMPT=1
SUCCESS=false

while [[ $ATTEMPT -le $MAX_ATTEMPTS ]]; do
    echo -n "Checking transaction status (attempt $ATTEMPT/$MAX_ATTEMPTS)... "
    
    EXEC_TX_RESULT=$(safrochaind query tx "$EXEC_TX_HASH" --node "$RPC_URL" --output json 2>&1 || echo "")
    
    # Check if we got a valid JSON response
    if echo "$EXEC_TX_RESULT" | jq -e '.' >/dev/null 2>&1; then
        if echo "$EXEC_TX_RESULT" | jq -e '.code == 0' >/dev/null 2>&1; then
            echo -e "${GREEN}✓ Success!${NC}"
            echo -e "${GREEN}✓ Transaction successful!${NC}"
            SUCCESS=true
            break
        elif echo "$EXEC_TX_RESULT" | jq -e '.code' >/dev/null 2>&1; then
            CODE=$(echo "$EXEC_TX_RESULT" | jq -r '.code')
            if [[ "$CODE" != "0" && "$CODE" != "null" ]]; then
                RAW_LOG=$(echo "$EXEC_TX_RESULT" | jq -r '.raw_log' 2>/dev/null || echo "")
                echo -e "${RED}✗ Failed!${NC}"
                echo -e "${RED}✗ Transaction failed with code: $CODE${NC}"
                echo "Error: $RAW_LOG"
                exit 1
            fi
        fi
    else
        # Not found yet, still waiting
        echo "not found yet..."
    fi
    
    if [[ $ATTEMPT -eq $MAX_ATTEMPTS ]]; then
        echo -e "${RED}Error: Transaction not confirmed after $MAX_ATTEMPTS attempts${NC}"
        echo "Transaction hash: $EXEC_TX_HASH"
        echo "You can check it manually:"
        echo "  safrochaind query tx $EXEC_TX_HASH --node $RPC_URL"
        echo ""
        echo "Or check on explorer:"
        echo "  https://explorer.testnet.safrochain.com/safrochain/tx/$EXEC_TX_HASH"
        exit 1
    fi
    
    sleep 2
    ATTEMPT=$((ATTEMPT + 1))
done

if [[ "$SUCCESS" == "true" ]]; then
    echo ""
    echo -e "${GREEN}=== Test Summary ===${NC}"
    echo -e "✓ Contract instantiated: ${BLUE}$CONTRACT_ADDRESS${NC}"
    echo -e "✓ CreateCircle executed successfully"
    echo -e "✓ Transaction hash: ${BLUE}$EXEC_TX_HASH${NC}"
    echo ""
    echo -e "${GREEN}All tests passed! The contract is working correctly.${NC}"
    exit 0
else
    echo -e "${RED}Test failed!${NC}"
    exit 1
fi

