chain-id 				?= devnet  # devnet|mainnet|testnet
tag						?= dev
network 				?= juno # juno|archway
sender 					?= juno16g2rahf5846rxzp3fwlswy08fz8ccuwk03k57y
build_dir 				?= ./builds
wasm_filename 			?= house_staking.wasm

# build optimized WASM artifact
build:
	./bin/build

deploy:
	./bin/deploy ./artifacts/$(wasm_filename) $(chain-id) $(network) $(sender) $(tag)

# instantiate last contract to be deployed using code ID in release dir code-id file
instantiate:
	./bin/instantiate $(chain-id) $(sender) $(tag)

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
	./client.sh connect-client $(chain-id) $(tag) $(sender)

resume:
	./client.sh resume-client $(chain-id) $(tag) $(sender) $(address)

select:
	./client.sh query-select $(chain-id) $(tag) $(sender)

accounts:
	./client.sh query-accounts $(chain-id) $(tag)
