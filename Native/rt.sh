# eventually can eliminate the 'cargo build' here
# if the dll import in managed code can be dynamically
# changed (or the lib name can be made constant)
cargo build && RUST_BACKTRACE=1 cargo test -- --nocapture --test-threads=1