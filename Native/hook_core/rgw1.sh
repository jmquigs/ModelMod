rustup default nightly-i686-pc-windows-msvc

if [ "$1" == "profile" ]; then
    BCMD="cargo build --release --features=profile"
else
    BCMD="cargo build --release"
fi

# NOTE: use git bash, not WSL
GPATH="/f"

$BCMD && cp -v target/release/hook_core.dll $GPATH/Guild\ Wars/d3d9.dll && RUST_BACKTRACE=1 $GPATH/Guild\ Wars/Gw.exe
