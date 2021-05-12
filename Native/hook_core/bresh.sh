# NOTE: use git bash, not WSL
rustup default nightly-i686-pc-windows-msvc

# if [ "$1" == "profile" ]; then
#     BCMD="cargo build --release --features=profile,noautohook"
# else
#     BCMD="cargo build --release --features=noautohook"
# fi

# first build hook core with noautohook, then build from the root to get everything else
# (can't specify features on the root workspace due to:
# https://github.com/rust-lang/cargo/issues/4753
# https://github.com/rust-lang/cargo/issues/5015
#)

( cd hook_core && cargo build --release --features=noautohook ) && cargo build --release && cp -v target/release/hook_core.dll ../Release/MMNative32.dll
