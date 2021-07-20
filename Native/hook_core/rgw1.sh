SPATH=$(dirname $0)
. $SPATH/shutil.sh
REQ=i686
check_tc $REQ

if [ "$1" == "profile" ]; then
    BCMD="cargo build --release --features=profile"
else
    BCMD="cargo build --release"
fi

# NOTE: use git bash, not WSL
GPATH="/f"

if [ ! -d "$GPATH" ]; then
    GPATH="/e"
fi

$BCMD && cp -v target/release/hook_core.dll $GPATH/Guild\ Wars/d3d9.dll && RUST_BACKTRACE=1 $GPATH/Guild\ Wars/Gw.exe
