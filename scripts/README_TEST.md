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

---

## Test Script: `test_distribution_threshold.sh`

This script exercises all three Distribution Threshold variants (Total, None, MinMembers) on the smart contract.

### Usage

```bash
cd safrimba-smartcontract-
./scripts/test_distribution_threshold.sh [network] [key_name] [code_id]
```

### Parameters

- `network`: `testnet` or `mainnet` (default: `testnet`)
- `key_name`: Your key name in safrochaind (default: `mycontractadmin`)
- `code_id`: The deployed contract code ID (default: `91`)

Set `DEPLOY_ON_ERROR=false` to skip automatic deploy-and-retry on instantiate failure.

### Example

```bash
./scripts/test_distribution_threshold.sh testnet mycontractadmin 91
```

### What it does

1. **Instantiates the contract** using the provided code ID
2. **Creates 8 circles** covering all distribution threshold variants:
   - 2 public + 2 private with **Total** (100% - distribution at end of each cycle)
   - 2 private with **None** (distribution from round 1)
   - 2 private with **MinMembers** (distribution from round 2 onwards)
3. **Verifies** each circle's `distribution_threshold` via `GetCircle` query
4. **Deploy-on-error**: If instantiate fails, runs `./scripts/deploy.sh testnet` and retries with the new code ID

### Distribution Threshold Variants

| Variant | Contract JSON | Description |
|---------|---------------|-------------|
| Total | `{"total": null}` | 100% - distribution only at last round of each cycle |
| None | omit | Distribution from round 1 (every round) |
| MinMembers | `{"min_members": {"count": N}}` | Distribution from round N onwards |

Note: Public circles always use Total regardless of input.

### Expected Output

If successful, you'll see:
```
=== Distribution Threshold Full Test Suite ===
...
[1/4] Instantiate contract
Contract: addr_safro1...
[2/4] Distribution Threshold: Total (100% - until completion)
  Creating: DT-Total-Public-1 (visibility=Public)
    OK
  ...
[5/5] Verifying distribution_threshold for all circles
  Circle 1 (expected total): OK
  ...
=== Test Summary ===
All distribution threshold tests passed!
```

### Troubleshooting

- **Instantiate fails**: The script will run `./scripts/deploy.sh testnet` and retry. Ensure your key has sufficient balance for 8 circles (creator lock = 200000 usaf per circle) plus gas.
- **Key not found**: Use `--keyring-backend os` if your keys are in the OS keyring: the script auto-detects this.
- **Insufficient funds**: Each circle requires creator lock = 2 × contribution_amount (200000 usaf with default contribution 100000).

---

## Test Script: `test_actions_full.sh`

Comprehensive test of **all contract actions** with full MD report for frontend correction.

### Usage

```bash
cd safrimba-smartcontract-
./scripts/test_actions_full.sh [network] [creator_key] [member_key] [code_id] [report_file]
```

### Parameters

- `network`: testnet or mainnet (default: testnet)
- `creator_key`: Creator wallet (default: mycontractadmin)
- `member_key`: Member wallet for join/invite tests (default: mywallet)
- `code_id`: Contract code ID (default: 93)
- `report_file`: Output MD path (default: TEST_ACTIONS_REPORT.md)

### Example

```bash
./scripts/test_actions_full.sh testnet mycontractadmin mywallet 93
```

### Actions Tested

| Action | Tested |
|--------|--------|
| Instantiate | Yes |
| CreateCircle (all 3 distribution thresholds) | Yes |
| JoinCircle (Public) | Yes |
| InviteMember + AcceptInvite (Private) | Yes |
| AddPrivateMember | Yes |
| StartCircle | Yes |
| UpdateCircle | Yes |
| CancelCircle (before start) | Yes |
| ExitCircle (before start) | Yes |
| DepositContribution | Yes |
| AdvanceRound | Yes |
| CheckAndEject | Yes |
| PauseCircle / UnpauseCircle | Yes |
| ProcessPayout / Withdraw | Documented (time-dependent) |

### Report Output

The script writes `TEST_ACTIONS_REPORT.md` with:

- Test output and circle states
- All contract actions table (for frontend)
- Distribution thresholds
- Final unlocking flow
- State machine
- Frontend actions by state
- Query messages
