.PHONY: build optimize schema test clean

# Build the contract
build:
	cargo build --release --target wasm32-unknown-unknown

# Optimize the contract using Docker (single contract optimizer)
optimize:
	@mkdir -p artifacts
	docker run --rm -v "$(CURDIR)":/code \
		--mount type=volume,source="$(shell basename $(CURDIR))_cache",target=/code/target \
		--mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
		cosmwasm/optimizer:0.14.0
	@if [ -f artifacts/safrimba_contract.wasm ]; then \
		echo "Optimization successful"; \
	else \
		echo "Warning: Optimization may have failed, check Docker output"; \
	fi

# Alternative: Optimize using local wasm-opt if available
# NOTE: Safrochain's wasmvm v0.54.0 does NOT support bulk memory
# Use two-pass optimization: first lower bulk memory, then optimize
optimize-local:
	@mkdir -p artifacts
	@if command -v wasm-opt >/dev/null 2>&1; then \
		echo "Warning: Using wasm-opt (Safrochain wasmvm v0.54.0 does NOT support bulk memory)"; \
		echo "Step 1: Lowering memory.copy/memory.fill operations..."; \
		wasm-opt --enable-bulk-memory --llvm-memory-copy-fill-lowering \
			target/wasm32-unknown-unknown/release/safrimba_contract.wasm \
			-o artifacts/temp_optimized.wasm && \
		echo "Step 2: Optimizing and removing reference-types..."; \
		wasm-opt -Os --strip-debug --strip-producers --disable-reference-types \
			artifacts/temp_optimized.wasm \
			-o artifacts/safrimba_contract.wasm && \
		rm -f artifacts/temp_optimized.wasm && \
		echo "Optimization complete - bulk memory operations lowered"; \
	else \
		echo "Error: wasm-opt not found. Install binaryen package or use 'make optimize' (Docker)."; \
		exit 1; \
	fi

# Generate JSON schemas
schema:
	cargo run --example schema

# Run tests
test:
	cargo test

# Clean build artifacts
clean:
	cargo clean
	rm -rf artifacts

# Install wasm32 target
install:
	rustup target add wasm32-unknown-unknown

# Deploy to testnet
deploy-testnet:
	./scripts/deploy.sh testnet

# Deploy to mainnet
deploy-mainnet:
	./scripts/deploy.sh mainnet

