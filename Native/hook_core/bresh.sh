# NOTE: use git bash, not WSL
rustup default nightly-i686-pc-windows-msvc

# if [ "$1" == "profile" ]; then
#     BCMD="cargo build --release --features=profile"
# else
#     BCMD="cargo build --release"
# fi

# to build a submodule with a feature specific to that module, it needs to be built explicitly first.
# then build everything else that doesn't use it.
# example: prepend this to the build command below to build
# with "noautohook":
# ( cd hook_core && cargo build --release --features=noautohook ) &&
# ("noautohook" is a nonexistance feature that was once going to be used for the late-init mode
# but turned out to be unnecessary).

# (can't specify features on the root workspace due to:
# https://github.com/rust-lang/cargo/issues/4753
# https://github.com/rust-lang/cargo/issues/5015
#)

cargo build --release && cp -v target/release/hook_core.dll ../Release/MMNative32.dll
