.PHONY: build, run, run-unchecked

build:
	cargo autoken check
	cargo build

run-unchecked:
	RUST_BACKTRACE=1 RUST_LOG=info cargo run

run: build, run-unchecked
