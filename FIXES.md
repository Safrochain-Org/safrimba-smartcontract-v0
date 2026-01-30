# Fixes Applied

## Issue 1: Reference-Types Not Enabled Error

**Problem**: The WASM file was being rejected with error:
```
reference-types not enabled: zero byte expected (at offset 0xba3)
```

**Solution**: Added `-C target-feature=-reference-types` to `.cargo/config.toml` to disable reference-types during compilation.

**File Changed**: `.cargo/config.toml`
```toml
[target.wasm32-unknown-unknown]
rustflags = ["-C", "link-arg=-s", "-C", "target-feature=-reference-types"]
```

## Issue 2: Docker Optimizer Workspace Error

**Problem**: The workspace-optimizer was failing with:
```
missing field `workspace`
```

**Solution**: 
- Switched to using `cosmwasm/optimizer:0.14.0` (single contract optimizer) instead of `cosmwasm/workspace-optimizer:0.14.0`
- Added fallback to use `wasm-opt` locally if Docker is not available
- Updated Makefile to use the single contract optimizer

**Files Changed**:
- `Makefile`: Updated optimize target to use `cosmwasm/optimizer:0.14.0`
- `scripts/deploy.sh`: Added fallback logic for optimization

## Testing

The contract now builds with reference-types disabled. To verify:

```bash
# Clean and rebuild
cargo clean
cargo build --release --target wasm32-unknown-unknown

# The WASM file should now work on the chain
```

## Deployment

Run the deployment script:
```bash
./scripts/deploy.sh testnet mycontractadmin addr_safro1x25weznnzd5k6jv663sdldehqwcjatc44gvrvq 100 addr_safro1x25weznnzd5k6jv663sdldehqwcjatc44gvrvq
```

The script will:
1. Always rebuild to ensure reference-types are disabled
2. Try to optimize with Docker (cosmwasm/optimizer)
3. Fall back to wasm-opt if Docker fails
4. Use unoptimized WASM if both fail (but with reference-types disabled)

