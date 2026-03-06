#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Default values
NETWORK="${1:-testnet}"
KEY_NAME="${2:-mywallet}"
ADMIN_ADDRESS="${3:-addr_safro1f8a9m8r5dq046qvmm9h5eryk0fn0u7tqu6v6sn}"
PLATFORM_FEE_PERCENT="${4:-100}"  # 1% in basis points
PLATFORM_ADDRESS="${5:-$ADMIN_ADDRESS}"

# Validate network
if [[ "$NETWORK" != "testnet" && "$NETWORK" != "mainnet" ]]; then
    echo -e "${RED}Error: Network must be 'testnet' or 'mainnet'${NC}"
    echo "Usage: $0 [testnet|mainnet] [key_name] [admin_address] [platform_fee_percent] [platform_address]"
    exit 1
fi

# Paths
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONTRACT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CHAIN_CONFIG="$CONTRACT_DIR/chain/$NETWORK/safrochain.json"
FRONTEND_DIR="$CONTRACT_DIR/../safrimba-frontend"
WASM_FILE="$CONTRACT_DIR/target/wasm32-unknown-unknown/release/safrimba_contract.wasm"
OPTIMIZED_WASM="$CONTRACT_DIR/artifacts/safrimba_contract.wasm"

# Check if chain config exists
if [[ ! -f "$CHAIN_CONFIG" ]]; then
    echo -e "${RED}Error: Chain config not found at $CHAIN_CONFIG${NC}"
    exit 1
fi

# Extract chain info from config
CHAIN_ID=$(jq -r '.chainId' "$CHAIN_CONFIG")
RPC_URL=$(jq -r '.rpc' "$CHAIN_CONFIG")
REST_URL=$(jq -r '.rest' "$CHAIN_CONFIG")
DENOM=$(jq -r '.feeCurrencies[0].coinMinimalDenom' "$CHAIN_CONFIG")

# Check for alternative denominations (usaf might be used on testnet)
ALT_DENOM="usaf"

echo -e "${GREEN}=== Safrimba Contract Deployment ===${NC}"
echo -e "Network: ${YELLOW}$NETWORK${NC}"
echo -e "Chain ID: ${YELLOW}$CHAIN_ID${NC}"
echo -e "RPC URL: ${YELLOW}$RPC_URL${NC}"
echo -e "Key Name: ${YELLOW}$KEY_NAME${NC}"
echo -e "Admin Address: ${YELLOW}$ADMIN_ADDRESS${NC}"
echo ""

# Check if safrochaind is installed
if ! command -v safrochaind &> /dev/null; then
    echo -e "${RED}Error: safrochaind command not found. Please install safrochaind.${NC}"
    exit 1
fi

# Check if jq is installed
if ! command -v jq &> /dev/null; then
    echo -e "${RED}Error: jq command not found. Please install jq (brew install jq or apt-get install jq).${NC}"
    exit 1
fi

# Check if key exists
if ! safrochaind keys show "$KEY_NAME" &> /dev/null; then
    echo -e "${YELLOW}Warning: Key '$KEY_NAME' not found in keyring.${NC}"
    echo -e "${YELLOW}Attempting to use address directly: $ADMIN_ADDRESS${NC}"
    USE_ADDRESS=true
    KEY_ADDRESS="$ADMIN_ADDRESS"
    
    # Verify the address format
    if [[ ! "$KEY_ADDRESS" =~ ^addr_safro ]]; then
        echo -e "${RED}Error: Invalid address format: $KEY_ADDRESS${NC}"
        echo -e "${YELLOW}Safrochain addresses should start with 'addr_safro'${NC}"
        exit 1
    fi
else
    USE_ADDRESS=false
    KEY_ADDRESS=$(safrochaind keys show "$KEY_NAME" -a)
    echo -e "Key Address: ${YELLOW}$KEY_ADDRESS${NC}"
    
    # Verify key can be used for transactions
    echo -e "${YELLOW}Verifying key can be used for transactions...${NC}"
    if ! safrochaind keys show "$KEY_NAME" &>/dev/null; then
        echo -e "${RED}Error: Key '$KEY_NAME' exists but cannot be accessed${NC}"
        echo -e "${YELLOW}Please check your keyring configuration${NC}"
        exit 1
    fi
fi

# Check account balance
echo -e "\n${GREEN}Checking account balance...${NC}"
ALL_BALANCES_JSON=$(safrochaind query bank balances "$KEY_ADDRESS" --node "$RPC_URL" --output json 2>/dev/null || echo '{"balances":[]}')

# Try to get balance in the expected denomination
BALANCE=$(echo "$ALL_BALANCES_JSON" | jq -r ".balances[] | select(.denom == \"$DENOM\") | .amount" || echo "0")
ACTUAL_DENOM="$DENOM"

# If no balance in expected denom, check for alternative (usaf)
if [[ -z "$BALANCE" || "$BALANCE" == "null" || "$BALANCE" == "0" ]]; then
    ALT_BALANCE=$(echo "$ALL_BALANCES_JSON" | jq -r ".balances[] | select(.denom == \"$ALT_DENOM\") | .amount" || echo "0")
    if [[ -n "$ALT_BALANCE" && "$ALT_BALANCE" != "null" && "$ALT_BALANCE" != "0" ]]; then
        BALANCE="$ALT_BALANCE"
        ACTUAL_DENOM="$ALT_DENOM"
        echo -e "${YELLOW}Note: Using $ALT_DENOM denomination (found balance in $ALT_DENOM, not $DENOM)${NC}"
        # Update DENOM to use the actual denomination found
        DENOM="$ALT_DENOM"
    fi
fi

if [[ -z "$BALANCE" || "$BALANCE" == "null" ]]; then
    BALANCE="0"
fi

echo -e "Current balance: ${YELLOW}${BALANCE}${ACTUAL_DENOM}${NC}"

# Show all balances for reference
ALL_BALANCES=$(echo "$ALL_BALANCES_JSON" | jq -r '.balances[] | "\(.amount)\(.denom)"' 2>/dev/null || echo "")
if [[ -n "$ALL_BALANCES" ]]; then
    echo -e "All balances:"
    echo "$ALL_BALANCES" | while read -r bal; do
        echo -e "  ${YELLOW}$bal${NC}"
    done
fi

# Determine fee denomination - chain might require "saf" even if account has "usaf"
# Check what the chain config says
FEE_DENOM=$(jq -r '.feeCurrencies[0].coinMinimalDenom' "$CHAIN_CONFIG")

# If account has usaf but chain expects saf, we might need to check conversion
# For now, use the denomination the account actually has for balance check
# But we'll use FEE_DENOM for actual transactions
if [[ "$ACTUAL_DENOM" != "$FEE_DENOM" ]]; then
    echo -e "${YELLOW}Note: Account has ${ACTUAL_DENOM} but chain expects ${FEE_DENOM} for fees${NC}"
    echo -e "${YELLOW}Will attempt to use ${ACTUAL_DENOM} for fees${NC}"
    # Use the actual denomination for fees if account has it
    FEE_DENOM="$ACTUAL_DENOM"
fi

# Calculate required balance (upload only, no instantiation)
# Based on actual gas estimates: upload ~2.8M gas
# With gas price 0.025usaf: (2.8M * 0.025) + buffer = ~100K usaf
# Using conservative estimate
REQUIRED_BALANCE=100000  # ~100K usaf for upload transaction with buffer

if (( BALANCE < REQUIRED_BALANCE )); then
    echo -e "\n${RED}Error: Insufficient balance${NC}"
    echo -e "Required: ${YELLOW}${REQUIRED_BALANCE}${ACTUAL_DENOM}${NC}"
    echo -e "Current: ${YELLOW}${BALANCE}${ACTUAL_DENOM}${NC}"
    echo -e "\nPlease fund your account with at least ${YELLOW}${REQUIRED_BALANCE}${ACTUAL_DENOM}${NC}"
    echo -e "Account address: ${YELLOW}$KEY_ADDRESS${NC}"
    exit 1
fi

# Update DENOM to use fee denomination for transactions
DENOM="$FEE_DENOM"
echo -e "Using denomination for fees: ${YELLOW}${DENOM}${NC}"

# Build contract (always rebuild to ensure reference-types are disabled)
echo -e "\n${GREEN}[1/4] Building contract...${NC}"
cd "$CONTRACT_DIR"
echo "Building WASM file with reference-types disabled..."
make build

# Optimize contract (REQUIRED to remove reference-types)
echo -e "\n${GREEN}[2/4] Optimizing contract (required to remove reference-types)...${NC}"
cd "$CONTRACT_DIR"
mkdir -p artifacts

# Always optimize to ensure reference-types are removed
# IMPORTANT: Safrochain's wasmvm (v0.54.0) does NOT support bulk memory operations
# We must use the CosmWasm optimizer (Docker) which properly handles this
# The CosmWasm optimizer will optimize without enabling bulk memory
# Use wasm-opt directly (preferred: faster, no Docker, handles modern Rust editions)
# Install on macOS: brew install binaryen
# Install on Ubuntu: apt-get install binaryen
if command -v wasm-opt &> /dev/null; then
    echo -e "${YELLOW}Warning: Using wasm-opt instead of CosmWasm optimizer${NC}"
    echo -e "${YELLOW}Note: Safrochain's wasmvm v0.54.0 does NOT support bulk memory${NC}"
    echo -e "${YELLOW}Lowering memory.copy/memory.fill operations to regular memory ops...${NC}"
    
    # Create temporary file for intermediate step
    TEMP_WASM="$CONTRACT_DIR/temp_optimized.wasm"
    
    # Step 1: Enable bulk memory temporarily to allow wasm-opt to process the file
    # Then lower bulk memory operations to regular operations
    # --enable-bulk-memory: needed to read the file (it contains bulk memory ops)
    # --llvm-memory-copy-fill-lowering: converts memory.copy/memory.fill to regular ops and disables bulk-memory
    echo "Step 1: Lowering bulk memory operations..."
    if wasm-opt --enable-bulk-memory --llvm-memory-copy-fill-lowering \
        "$WASM_FILE" \
        -o "$TEMP_WASM" 2>&1; then
        echo -e "${GREEN}Step 1 successful: Bulk memory operations lowered${NC}"
        
        # Step 2: Optimize and remove reference-types
        echo "Step 2: Optimizing and removing reference-types..."
        if wasm-opt -Os --strip-debug --strip-producers --disable-reference-types \
            "$TEMP_WASM" \
            -o "$OPTIMIZED_WASM" 2>&1; then
            echo -e "${GREEN}wasm-opt optimization successful${NC}"
            echo -e "${GREEN}Bulk memory operations lowered to regular memory operations${NC}"
            rm -f "$TEMP_WASM"
            OPTIMIZE_SUCCESS=true
        else
            echo -e "${RED}Error: Step 2 (optimization) failed${NC}"
            rm -f "$TEMP_WASM"
            OPTIMIZE_SUCCESS=false
        fi
    else
        echo -e "${RED}Error: Step 1 (bulk memory lowering) failed${NC}"
        rm -f "$TEMP_WASM"
        OPTIMIZE_SUCCESS=false
    fi
else
    echo -e "${RED}Error: No optimizer found${NC}"
    echo -e "${RED}Reference-types must be removed for the contract to work on chain${NC}"
    echo -e "${RED}For Safrochain (wasmvm v0.54.0), Docker with cosmwasm/optimizer is REQUIRED${NC}"
    echo -e "\nPlease install Docker:"
    echo -e "  - Docker: https://docs.docker.com/get-docker/"
    exit 1
fi

# Verify optimized WASM exists
if [[ "$OPTIMIZE_SUCCESS" != "true" || ! -f "$OPTIMIZED_WASM" ]]; then
    echo -e "${RED}Error: Failed to create optimized WASM file${NC}"
    echo -e "${RED}Optimization is required to remove reference-types${NC}"
    exit 1
fi

WASM_SIZE=$(du -h "$OPTIMIZED_WASM" | cut -f1)
echo -e "WASM file size: ${YELLOW}$WASM_SIZE${NC}"

# Upload contract
echo -e "\n${GREEN}[3/4] Uploading contract to chain...${NC}"
echo -e "This may take a minute. Please wait...${NC}"

# Test RPC connection first
echo -e "${YELLOW}Testing RPC connection...${NC}"
if ! safrochaind query bank balances "$KEY_ADDRESS" --node "$RPC_URL" --output json >/dev/null 2>&1; then
    echo -e "${RED}Error: Cannot connect to RPC endpoint${NC}"
    echo -e "${YELLOW}RPC URL: $RPC_URL${NC}"
    echo -e "${YELLOW}Please check:${NC}"
    echo -e "  1. RPC endpoint is accessible"
    echo -e "  2. Network connection is working"
    echo -e "  3. Chain ID is correct: $CHAIN_ID"
    exit 1
fi
echo -e "${GREEN}RPC connection OK${NC}"

# Use gas-prices instead of fixed fees (chain calculates fees automatically)
GAS_PRICE="0.025${DENOM}"

echo -e "Running: safrochaind tx wasm store $OPTIMIZED_WASM --from $KEY_NAME --chain-id $CHAIN_ID --node $RPC_URL --broadcast-mode sync --gas auto --gas-adjustment 1.4 --gas-prices $GAS_PRICE -y"
echo ""

# Capture both stdout and stderr with timeout
# Use timeout to prevent hanging (60 seconds should be enough)
# Portable timeout: gtimeout (macOS), timeout (Linux), or no-timeout fallback
UPLOAD_TIMEOUT=""
if command -v gtimeout &> /dev/null; then
    UPLOAD_TIMEOUT="gtimeout 60"
elif command -v timeout &> /dev/null; then
    UPLOAD_TIMEOUT="timeout 60"
fi
if [[ -n "$UPLOAD_TIMEOUT" ]]; then
    UPLOAD_OUTPUT=$($UPLOAD_TIMEOUT safrochaind tx wasm store "$OPTIMIZED_WASM" \
        --from "$KEY_NAME" \
        --chain-id "$CHAIN_ID" \
        --node "$RPC_URL" \
        --broadcast-mode sync \
        --gas auto \
        --gas-adjustment 1.4 \
        --gas-prices "$GAS_PRICE" \
        -y \
        --output json 2>&1)
    UPLOAD_EXIT_CODE=$?
    
    # Check if timeout occurred
    if [[ $UPLOAD_EXIT_CODE -eq 124 ]]; then
        echo -e "${RED}Error: Command timed out after 60 seconds${NC}"
        echo -e "${YELLOW}The transaction may still be processing. Check manually with:${NC}"
        echo -e "  ${YELLOW}safrochaind query wasm list-code --node $RPC_URL --limit 5${NC}"
        exit 1
    fi
else
    # Fallback if timeout command is not available
    UPLOAD_OUTPUT=$(safrochaind tx wasm store "$OPTIMIZED_WASM" \
        --from "$KEY_NAME" \
        --chain-id "$CHAIN_ID" \
        --node "$RPC_URL" \
        --broadcast-mode sync \
        --gas auto \
        --gas-adjustment 1.4 \
        --gas-prices "$GAS_PRICE" \
        -y \
        --output json 2>&1)
    UPLOAD_EXIT_CODE=$?
fi

# Always show the output for debugging
echo "Transaction output:"
echo "$UPLOAD_OUTPUT"
echo ""

# Check if output is empty (command might have failed silently)
if [[ -z "$UPLOAD_OUTPUT" || "$UPLOAD_OUTPUT" == "" ]]; then
    echo -e "${RED}Error: No output from transaction command${NC}"
    echo -e "${YELLOW}This might indicate:${NC}"
    echo -e "  - Network connection issue"
    echo -e "  - RPC endpoint not responding"
    echo -e "  - Key not found or incorrect"
    echo -e "  - Command is hanging"
    echo ""
    echo -e "Trying to run command without output capture to debug..."
    echo -e "${YELLOW}If this hangs, press Ctrl+C and check your connection${NC}"
    echo ""
    
    # Try running without capture to see what happens
    safrochaind tx wasm store "$OPTIMIZED_WASM" \
        --from "$KEY_NAME" \
        --chain-id "$CHAIN_ID" \
        --node "$RPC_URL" \
        --broadcast-mode sync \
        --gas auto \
        --gas-adjustment 1.4 \
        --gas-prices "$GAS_PRICE" \
        -y \
        --output json
    
    echo ""
    echo -e "${YELLOW}If you see output above, the command works but output capture failed.${NC}"
    echo -e "${YELLOW}Please run the deployment manually or check the script.${NC}"
    exit 1
fi

if [[ $UPLOAD_EXIT_CODE -ne 0 ]]; then
    echo -e "${RED}Error uploading contract (exit code: $UPLOAD_EXIT_CODE)${NC}"
    
    # Check for specific error types
    if echo "$UPLOAD_OUTPUT" | grep -qi "insufficient"; then
        echo -e "${YELLOW}Insufficient balance or fees error detected${NC}"
    fi
    if echo "$UPLOAD_OUTPUT" | grep -qi "connection\|timeout\|refused"; then
        echo -e "${YELLOW}Connection error - check RPC endpoint${NC}"
        echo -e "${YELLOW}RPC URL: $RPC_URL${NC}"
    fi
    if echo "$UPLOAD_OUTPUT" | grep -qi "key not found\|no such key"; then
        echo -e "${YELLOW}Key '$KEY_NAME' not found in keyring${NC}"
        echo -e "${YELLOW}Available keys:${NC}"
        safrochaind keys list 2>/dev/null || echo "  (Could not list keys)"
    fi
    if echo "$UPLOAD_OUTPUT" | grep -qi "account.*not found"; then
        echo -e "${YELLOW}Account not found. Please fund your account first.${NC}"
        echo -e "${YELLOW}Account address: $KEY_ADDRESS${NC}"
    fi
    
    echo ""
    echo "Full error output:"
    echo "$UPLOAD_OUTPUT"
    exit 1
fi

# Check if output contains error messages even if exit code is 0
if echo "$UPLOAD_OUTPUT" | grep -qi "error\|failed\|denied"; then
    echo -e "${YELLOW}Warning: Output contains error messages${NC}"
    echo "$UPLOAD_OUTPUT" | grep -i "error\|failed\|denied" || true
fi

# Extract JSON from output (may contain "gas estimate" line before JSON)
# Try to find JSON line (starts with { and ends with })
UPLOAD_TX=$(echo "$UPLOAD_OUTPUT" | grep -E '^\{.*\}$' | tail -1)

# If no JSON found, try each line with jq to find valid JSON
if [[ -z "$UPLOAD_TX" ]]; then
    while IFS= read -r line; do
        if echo "$line" | jq -e . >/dev/null 2>&1; then
            UPLOAD_TX="$line"
            break
        fi
    done <<< "$UPLOAD_OUTPUT"
fi

# Extract transaction hash from JSON
TX_HASH=""
if [[ -n "$UPLOAD_TX" ]]; then
    TX_HASH=$(echo "$UPLOAD_TX" | jq -r '.txhash // empty' 2>/dev/null)
fi

# Fallback: extract from raw output using grep
if [[ -z "$TX_HASH" || "$TX_HASH" == "null" ]]; then
    TX_HASH=$(echo "$UPLOAD_OUTPUT" | grep -oE '"txhash":\s*"[^"]+"' | sed -E 's/"txhash":\s*"([^"]+)"/\1/' | head -1)
fi

# Final fallback: extract any hex string that looks like a tx hash
if [[ -z "$TX_HASH" || "$TX_HASH" == "null" ]]; then
    TX_HASH=$(echo "$UPLOAD_OUTPUT" | grep -oE '[A-F0-9]{64}' | head -1)
fi

if [[ -z "$TX_HASH" || "$TX_HASH" == "null" ]]; then
    echo -e "${RED}Error: Could not extract transaction hash${NC}"
    echo "Transaction output:"
    echo "$UPLOAD_OUTPUT"
    exit 1
fi

echo -e "Transaction hash: ${YELLOW}$TX_HASH${NC}"
echo "Waiting for transaction to be included in block..."
echo -e "${YELLOW}This may take 10-30 seconds. Please wait...${NC}"

# Wait a bit for transaction to be included
sleep 5

# Try multiple times to get the transaction result
CODE_ID=""
MAX_RETRIES=30
for i in $(seq 1 $MAX_RETRIES); do
    echo -e "Checking transaction status (attempt $i/$MAX_RETRIES)..."
    TX_RESULT=$(safrochaind query tx "$TX_HASH" --node "$RPC_URL" --output json 2>/dev/null)
    TX_QUERY_EXIT=$?
    
    if [[ $TX_QUERY_EXIT -eq 0 && -n "$TX_RESULT" && "$TX_RESULT" != "null" ]]; then
        # Check if transaction was successful (code 0)
        TX_CODE=$(echo "$TX_RESULT" | jq -r '.code' 2>/dev/null)
        
        if [[ "$TX_CODE" == "0" ]]; then
            echo -e "${GREEN}Transaction confirmed! Extracting code ID...${NC}"
            
            # Try to extract code ID from events - multiple methods
            # Method 1: Check top-level events array (CosmWasm stores code_id here)
            CODE_ID=$(echo "$TX_RESULT" | jq -r '.events[]? | select(.type == "store_code") | .attributes[]? | select(.key == "code_id") | .value' 2>/dev/null | head -1)
            
            # Method 2: Check top-level events for any code_id attribute
            if [[ -z "$CODE_ID" || "$CODE_ID" == "null" ]]; then
                CODE_ID=$(echo "$TX_RESULT" | jq -r '.events[]?.attributes[]? | select(.key == "code_id") | .value' 2>/dev/null | head -1)
            fi
            
            # Method 3: Check logs events (fallback)
            if [[ -z "$CODE_ID" || "$CODE_ID" == "null" ]]; then
                CODE_ID=$(echo "$TX_RESULT" | jq -r '.logs[0].events[]? | select(.type == "store_code") | .attributes[]? | select(.key == "code_id") | .value' 2>/dev/null | head -1)
            fi
            
            # Method 4: Search all logs for store_code
            if [[ -z "$CODE_ID" || "$CODE_ID" == "null" ]]; then
                CODE_ID=$(echo "$TX_RESULT" | jq -r '.logs[].events[]? | select(.type == "store_code") | .attributes[]? | select(.key == "code_id") | .value' 2>/dev/null | head -1)
            fi
            
            # Method 5: Search for code_id in any log event
            if [[ -z "$CODE_ID" || "$CODE_ID" == "null" ]]; then
                CODE_ID=$(echo "$TX_RESULT" | jq -r '.logs[].events[]?.attributes[]? | select(.key == "code_id") | .value' 2>/dev/null | head -1)
            fi
            
            if [[ -n "$CODE_ID" && "$CODE_ID" != "null" && "$CODE_ID" != "" ]]; then
                echo -e "${GREEN}Code ID found: $CODE_ID${NC}"
                break
            else
                echo -e "${YELLOW}Transaction confirmed but code ID not found yet. Checking events...${NC}"
                # Debug: show event types
                echo "$TX_RESULT" | jq -r '.logs[].events[]?.type' 2>/dev/null | sort -u | head -5
            fi
        elif [[ "$TX_CODE" != "null" && "$TX_CODE" != "" ]]; then
            echo -e "${RED}Transaction failed with code: $TX_CODE${NC}"
            RAW_LOG=$(echo "$TX_RESULT" | jq -r '.raw_log' 2>/dev/null)
            if [[ -n "$RAW_LOG" && "$RAW_LOG" != "null" ]]; then
                echo -e "${RED}Error: $RAW_LOG${NC}"
            fi
            echo "Transaction result:"
            echo "$TX_RESULT" | jq '.' 2>/dev/null || echo "$TX_RESULT"
            exit 1
        fi
    elif [[ $TX_QUERY_EXIT -ne 0 ]]; then
        # Transaction not found yet - this is normal, just wait
        if [[ $i -eq 1 ]]; then
            echo -e "${YELLOW}Transaction not yet included in block. Waiting...${NC}"
        fi
    fi
    
    if [[ $i -lt $MAX_RETRIES ]]; then
        sleep 3
    fi
done

if [[ -z "$CODE_ID" || "$CODE_ID" == "null" ]]; then
    echo -e "${YELLOW}Warning: Could not extract code ID from transaction${NC}"
    echo -e "Transaction hash: ${YELLOW}$TX_HASH${NC}"
    echo -e "\nPlease check the transaction manually:"
    echo -e "  ${YELLOW}safrochaind query tx $TX_HASH --node $RPC_URL${NC}"
    echo ""
    echo "You can also check on the explorer:"
    echo -e "  ${YELLOW}https://explorer.testnet.safrochain.com/safrochain/tx/$TX_HASH${NC}"
    echo ""
    echo "Last transaction query result:"
    if [[ -n "$TX_RESULT" && "$TX_RESULT" != "null" ]]; then
        echo "$TX_RESULT" | jq '.' 2>/dev/null || echo "$TX_RESULT"
        echo ""
        echo "Event types found:"
        echo "$TX_RESULT" | jq -r '.logs[].events[]?.type' 2>/dev/null | sort -u
        echo ""
        echo "All attributes with 'code' in the key:"
        echo "$TX_RESULT" | jq -r '.logs[].events[]?.attributes[]? | select(.key | contains("code")) | "\(.key): \(.value)"' 2>/dev/null
    else
        echo "Transaction not found in block yet. It may still be pending."
    fi
    exit 1
fi

echo -e "Code ID: ${GREEN}$CODE_ID${NC}"

# Note: Contract instantiation will be done by the frontend
# Each user's circle will create a new instance of the smart contract
echo -e "\n${YELLOW}Note: Contract will be instantiated by the frontend.${NC}"
echo -e "${YELLOW}Each circle will create a new unique contract instance.${NC}"

# Update codeId in chain config file
echo -e "\n${GREEN}[4/5] Updating codeId in chain config file...${NC}"
FRONTEND_CHAIN_CONFIG="$FRONTEND_DIR/chain/$NETWORK/safrochain.json"
if [[ -f "$FRONTEND_CHAIN_CONFIG" ]]; then
    # Create backup
    cp "$FRONTEND_CHAIN_CONFIG" "$FRONTEND_CHAIN_CONFIG.bak"
    # Update codeId using jq
    jq ".codeId = $CODE_ID" "$FRONTEND_CHAIN_CONFIG" > "$FRONTEND_CHAIN_CONFIG.tmp" && mv "$FRONTEND_CHAIN_CONFIG.tmp" "$FRONTEND_CHAIN_CONFIG"
    echo -e "${GREEN}Updated codeId in: ${YELLOW}$FRONTEND_CHAIN_CONFIG${NC}"
    echo -e "  ${GREEN}codeId: ${YELLOW}$CODE_ID${NC}"
else
    echo -e "${YELLOW}Warning: Frontend chain config not found at $FRONTEND_CHAIN_CONFIG${NC}"
    echo -e "${YELLOW}Please update codeId manually in the chain config file.${NC}"
fi

# Save to .env file in frontend
echo -e "\n${GREEN}[5/5] Saving contract CODE ID to frontend .env file...${NC}"
ENV_FILE="$FRONTEND_DIR/.env"
ENV_LOCAL_FILE="$FRONTEND_DIR/.env.local"

# Create .env.local file (Vite uses .env.local for local overrides)
# Store CODE ID (not contract address) - frontend will instantiate contracts
if [[ -f "$ENV_LOCAL_FILE" ]]; then
    # Update existing values
    if grep -q "VITE_SMARTCONTRACT_CODE_ID=" "$ENV_LOCAL_FILE"; then
        sed -i.bak "s|VITE_SMARTCONTRACT_CODE_ID=.*|VITE_SMARTCONTRACT_CODE_ID=$CODE_ID|" "$ENV_LOCAL_FILE"
    else
        echo "VITE_SMARTCONTRACT_CODE_ID=$CODE_ID" >> "$ENV_LOCAL_FILE"
    fi
    
    # Keep VITE_SMARTCONTRACT_ID for backward compatibility (also set to CODE_ID)
    if grep -q "VITE_SMARTCONTRACT_ID=" "$ENV_LOCAL_FILE"; then
        sed -i.bak "s|VITE_SMARTCONTRACT_ID=.*|VITE_SMARTCONTRACT_ID=$CODE_ID|" "$ENV_LOCAL_FILE"
    else
        echo "VITE_SMARTCONTRACT_ID=$CODE_ID" >> "$ENV_LOCAL_FILE"
    fi
    
    if grep -q "VITE_NETWORK=" "$ENV_LOCAL_FILE"; then
        sed -i.bak "s|VITE_NETWORK=.*|VITE_NETWORK=$NETWORK|" "$ENV_LOCAL_FILE"
    else
        echo "VITE_NETWORK=$NETWORK" >> "$ENV_LOCAL_FILE"
    fi
    
    if grep -q "VITE_CHAIN_ID=" "$ENV_LOCAL_FILE"; then
        sed -i.bak "s|VITE_CHAIN_ID=.*|VITE_CHAIN_ID=$CHAIN_ID|" "$ENV_LOCAL_FILE"
    else
        echo "VITE_CHAIN_ID=$CHAIN_ID" >> "$ENV_LOCAL_FILE"
    fi
    
    if grep -q "VITE_RPC_URL=" "$ENV_LOCAL_FILE"; then
        sed -i.bak "s|VITE_RPC_URL=.*|VITE_RPC_URL=$RPC_URL|" "$ENV_LOCAL_FILE"
    else
        echo "VITE_RPC_URL=$RPC_URL" >> "$ENV_LOCAL_FILE"
    fi
    
    if grep -q "VITE_REST_URL=" "$ENV_LOCAL_FILE"; then
        sed -i.bak "s|VITE_REST_URL=.*|VITE_REST_URL=$REST_URL|" "$ENV_LOCAL_FILE"
    else
        echo "VITE_REST_URL=$REST_URL" >> "$ENV_LOCAL_FILE"
    fi
    
    rm -f "$ENV_LOCAL_FILE.bak"
else
    # Create new file
    cat > "$ENV_LOCAL_FILE" <<EOF
# Safrimba Smart Contract Configuration
# Generated by deploy script on $(date)
# CODE ID will be used by frontend to instantiate new contract instances for each circle

VITE_SMARTCONTRACT_CODE_ID=$CODE_ID
VITE_SMARTCONTRACT_ID=$CODE_ID
VITE_NETWORK=$NETWORK
VITE_CHAIN_ID=$CHAIN_ID
VITE_RPC_URL=$RPC_URL
VITE_REST_URL=$REST_URL
EOF
fi

echo -e "Saved to: ${YELLOW}$ENV_LOCAL_FILE${NC}"

# Also save deployment info to contract directory
echo -e "\n${GREEN}Saving deployment info...${NC}"
DEPLOYMENT_INFO="$CONTRACT_DIR/deployment-$NETWORK.json"
cat > "$DEPLOYMENT_INFO" <<EOF
{
  "network": "$NETWORK",
  "chainId": "$CHAIN_ID",
  "codeId": "$CODE_ID",
  "adminAddress": "$ADMIN_ADDRESS",
  "platformFeePercent": $PLATFORM_FEE_PERCENT,
  "platformAddress": "$PLATFORM_ADDRESS",
  "deployedAt": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
  "wasmFile": "$(basename $OPTIMIZED_WASM)",
  "note": "Contract instances will be created by the frontend for each circle"
}
EOF

echo -e "Deployment info saved to: ${YELLOW}$DEPLOYMENT_INFO${NC}"

# Summary
echo -e "\n${GREEN}=== Deployment Summary ===${NC}"
echo -e "Network: ${YELLOW}$NETWORK${NC}"
echo -e "Code ID: ${GREEN}$CODE_ID${NC}"
echo -e "Admin Address: ${YELLOW}$ADMIN_ADDRESS${NC}"
echo -e "Platform Fee: ${YELLOW}$PLATFORM_FEE_PERCENT${NC} basis points (1% = 100)"
echo -e "\n${GREEN}Contract code uploaded successfully!${NC}"
echo -e "${YELLOW}The frontend will instantiate new contract instances for each circle.${NC}"
echo -e "Frontend .env file updated: ${YELLOW}$ENV_LOCAL_FILE${NC}"

