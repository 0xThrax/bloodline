# Run every check that CI runs.
check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    cargo test --workspace --all-features
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features
