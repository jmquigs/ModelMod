SPATH=$(dirname $0)
. $SPATH/shutil.sh
REQ=x86_64
check_tc $REQ

if [ "$1" == "profile" ]; then
    BCMD="cargo build --release --features=profile"
else
    BCMD="cargo build --release"
fi

# NOTE: use git bash, not WSL
GPATH="/f"

$BCMD && cp -v target/release/hook_core.dll $GPATH/Guild\ Wars\ 2/bin64/d3d9.dll && RUST_BACKTRACE=1 $GPATH/Guild\ Wars\ 2/Gw2-64.exe
