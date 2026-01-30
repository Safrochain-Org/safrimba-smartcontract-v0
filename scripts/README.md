# Deployment Scripts

## deploy.sh

Deploys the Safrimba smart contract to Safrochain (testnet or mainnet).

### Usage

```bash
./scripts/deploy.sh [network] [key_name] [admin_address] [platform_fee_percent] [platform_address]
```

### Parameters

- `network` (optional, default: `testnet`): Network to deploy to (`testnet` or `mainnet`)
- `key_name` (optional, default: `mycontractadmin`): Key name in safrochaind keyring, or address if key doesn't exist
- `admin_address` (optional, default: `addr_safro1x25weznnzd5k6jv663sdldehqwcjatc44gvrvq`): Admin address for the contract
- `platform_fee_percent` (optional, default: `100`): Platform fee in basis points (100 = 1%)
- `platform_address` (optional, default: same as admin_address): Address to receive platform fees

### Examples

Deploy to testnet with default settings:
```bash
./scripts/deploy.sh
```

Deploy to testnet with custom key:
```bash
./scripts/deploy.sh testnet mykey
```

Deploy to mainnet:
```bash
./scripts/deploy.sh mainnet mycontractadmin addr_safro1x25weznnzd5k6jv663sdldehqwcjatc44gvrvq 100 addr_safro1x25weznnzd5k6jv663sdldehqwcjatc44gvrvq
```

### What it does

1. **Builds** the contract (if not already built)
2. **Optimizes** the WASM file using Docker (if available)
3. **Uploads** the contract to the chain
4. **Instantiates** the contract with the provided parameters
5. **Saves** the contract address to `safrimba-frontend/.env.local` as `VITE_SMARTCONTRACT_ID`
6. **Saves** network configuration to the frontend `.env.local` file
7. **Creates** a deployment info JSON file in the contract directory

### Output Files

- `safrimba-frontend/.env.local`: Frontend environment variables
  - `VITE_SMARTCONTRACT_ID`: Contract address
  - `VITE_NETWORK`: Network (testnet/mainnet)
  - `VITE_CHAIN_ID`: Chain ID
  - `VITE_RPC_URL`: RPC endpoint
  - `VITE_REST_URL`: REST endpoint

- `deployment-{network}.json`: Deployment information including code ID, contract address, and deployment timestamp

### Prerequisites

- `safrochaind` CLI installed and configured
- Key added to keyring (or use address directly)
- Sufficient balance for gas fees
- Docker (optional, for WASM optimization)

### Notes

- The script automatically detects if a key exists in the keyring
- If the key doesn't exist, it will attempt to use the address directly
- The frontend `.env.local` file is created/updated automatically
- Deployment info is saved for reference and tracking

