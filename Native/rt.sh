# 'cargo test' will generate an executable for the tests to run from, but won't build the dll.
# we need the DLL for the managed code to integrate; it can't import symbols from an executable.
# so need to 'cargo build' before running the tests to generate the dll.
# any test which invokes the clr will load the dll,
# which means that during tests there are two copies of the code present in the same
# process, one from the test executable and one from the dll.  Other than the
# "duplicate globals" problem (which the code works around by passing a pointer to global
# state in and out of managed code), I haven't seen other issues.  but this isn't an ideal
# situation.
cargo build && RUST_BACKTRACE=1 cargo test -- --nocapture --test-threads=1