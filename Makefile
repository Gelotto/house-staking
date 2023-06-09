network 				?= devnet  # network := devnet|mainnet|testnet
sender 					?= juno16g2rahf5846rxzp3fwlswy08fz8ccuwk03k57y
build_dir 				?= ./builds
wasm_filename 			?= house_staking.wasm

# build optimized WASM artifact
build:
	./bin/build

# deploy WASM file (generated from `make build`)
deploy:
	./bin/deploy ./artifacts/$(wasm_filename) $(network) $(sender) $(tag)

# instantiate last contract to be deployed using code ID in release dir code-id file
instantiate:
	./bin/instantiate $(network) $(sender) $(tag)

# run all unit tests
test:
	RUST_BACKTRACE=1 cargo unit-test

# Generate the contract's JSONSchema JSON files in schemas/
schemas:
	cargo schema

# Run/start local "devnet" validator docker image	
devnet:
	./bin/devnet

connect:
	./client.sh connect-client $(network) $(tag) $(sender)

resume:
	./client.sh resume-client $(network) $(tag) $(sender) $(address)

select:
	./client.sh query-select $(network) $(tag) $(sender)

accounts:
	./client.sh query-accounts $(network) $(tag)
