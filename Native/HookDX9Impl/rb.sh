if [ "$1" == "profile" ]; then
    BCMD="cargo build --release --features=profile"
else
    BCMD="cargo build --release"
fi

# NOTE: use git bash, not WSL
GPATH="/f"

$BCMD && cp -v target/release/mm_native.dll $GPATH/Guild\ Wars\ 2/bin64/d3d9.dll && RUST_BACKTRACE=1 $GPATH/Guild\ Wars\ 2/Gw2-64.exe
