build:
    cargo autoken check --old-artifacts=delete
    cargo build

run-unchecked:
    RUST_BACKTRACE=1 RUST_LOG=info cargo run

run:
    just build
    just run-unchecked
