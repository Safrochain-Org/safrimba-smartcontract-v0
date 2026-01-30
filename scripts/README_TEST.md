# Testing the Safrimba Smart Contract

## Test Script: `test_create_circle.sh`

This script tests the `CreateCircle` execute message on a deployed contract.

### Usage

```bash
cd safrimba-smartcontract
./scripts/test_create_circle.sh [network] [key_name] [code_id]
```

### Parameters

- `network`: `testnet` or `mainnet` (default: `testnet`)
- `key_name`: Your key name in safrochaind (default: `mycontractadmin`)
- `code_id`: The deployed contract code ID (default: `66`)

### Example

```bash
./scripts/test_create_circle.sh testnet mycontractadmin 66
```

### What it does

1. **Instantiates the contract** using the provided code ID
2. **Executes CreateCircle** with a test message
3. **Verifies the transaction** was successful

### Test Message Format

The script tests with a message that:
- Omits optional fields (matching `skip_serializing_if` behavior)
- Uses correct types (Uint128 as strings, Timestamp with seconds as string)
- Matches the frontend's message format

### Expected Output

If successful, you'll see:
```
✓ Contract instantiated: addr_safro1...
✓ CreateCircle executed successfully
✓ Transaction hash: ABC123...
All tests passed! The contract is working correctly.
```

### Troubleshooting

- **Key not found**: Make sure your key exists: `safrochaind keys list`
- **Transaction fails**: Check the error message for parsing issues
- **Code ID not found**: Verify the code ID exists: `safrochaind query wasm code-info <code_id> --node <rpc_url>`
