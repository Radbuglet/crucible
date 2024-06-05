.PHONY: build, run

build:
	cargo autoken check
	cargo build

run: build
	RUST_BACKTRACE=1 RUST_LOG=info cargo run
