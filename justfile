run:
	RUSTFLAGS="-Clink-arg=--export-table" cargo build --target wasm32-wasi -p crucible
	cp target/wasm32-wasi/debug/crucible.wasm demo/server.wasm
	cargo run -p crucible-server -- start -c demo/crucible.toml
