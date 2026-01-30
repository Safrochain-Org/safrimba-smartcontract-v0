# Safrimba Smart Contract

A CosmWasm smart contract implementing a digital tontine system on Safrochain. The contract manages all deposits, rules, and payouts in SAF tokens automatically.

## Features

- **Circle Management**: Create, join, and manage tontine circles with configurable parameters
- **Automatic Payouts**: Scheduled payouts based on cycle duration and payout order
- **Penalty System**: Late fees and penalties for missed payments
- **Security Controls**: Emergency stop, arbitration, and pause/unpause functionality
- **Transparent Tracking**: On-chain event logging for all operations
- **Flexible Membership**: Public or invite-only circles with configurable member limits

## Contract Structure

- `src/contract.rs` - Entry points (instantiate, execute, query)
- `src/execute.rs` - Execute message handlers
- `src/query.rs` - Query message handlers
- `src/state.rs` - State definitions and storage
- `src/msg.rs` - Message types
- `src/error.rs` - Error types

## Building

### Prerequisites

- Rust 1.70+
- `wasm32-unknown-unknown` target
- CosmWasm optimizer (optional, for optimized builds)

### Build Commands

```bash
# Install wasm32 target
rustup target add wasm32-unknown-unknown

# Build the contract
cargo build --release --target wasm32-unknown-unknown

# Generate schemas (optional)
cargo run --example schema
```

### Optimize Build (Recommended)

```bash
# Install cosmwasm-optimizer
docker pull cosmwasm/workspace-optimizer:0.14.0

# Optimize the contract
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/workspace-optimizer:0.14.0
```

## Deployment

The optimized WASM file will be in `artifacts/safrimba_contract.wasm` after optimization.

Deploy to Safrochain testnet using:

```bash
# Upload the contract
safrochaind tx wasm store artifacts/safrimba_contract.wasm \
  --from <your-key> \
  --chain-id safrochain-testnet \
  --gas auto \
  --gas-adjustment 1.3 \
  --fees 1000000saf

# Instantiate the contract
safrochaind tx wasm instantiate <code-id> \
  '{"platform_fee_percent": 100, "platform_address": "addr_safro..."}' \
  --from <your-key> \
  --chain-id safrochain-testnet \
  --label "Safrimba Contract" \
  --admin <admin-address> \
  --gas auto \
  --fees 1000000saf
```

## Usage

### Creating a Circle

```json
{
  "create_circle": {
    "circle_name": "My Tontine Circle",
    "circle_description": "A savings circle for friends",
    "max_members": 10,
    "min_members_required": 5,
    "contribution_amount": "1000000",
    "total_cycles": 10,
    "cycle_duration_days": 30,
    "payout_order_type": "random_order",
    "auto_payout_enabled": true,
    ...
  }
}
```

### Joining a Circle

```json
{
  "join_circle": {
    "circle_id": 1
  }
}
```

### Depositing Contribution

Send SAF tokens along with:

```json
{
  "deposit_contribution": {
    "circle_id": 1
  }
}
```

### Processing Payout

```json
{
  "process_payout": {
    "circle_id": 1
  }
}
```

## Query Examples

### Get Circle Information

```json
{
  "get_circle": {
    "circle_id": 1
  }
}
```

### Get Circle Members

```json
{
  "get_circle_members": {
    "circle_id": 1
  }
}
```

### Get Current Cycle

```json
{
  "get_current_cycle": {
    "circle_id": 1
  }
}
```

## License

[Your License Here]
