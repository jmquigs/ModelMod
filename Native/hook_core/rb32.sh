SPATH=$(dirname $0)
. $SPATH/shutil.sh
REQ=i686
check_tc $REQ


if [ "$1" == "profile" ]; then
BCMD="cargo build --release --features=profile"
else
BCMD="cargo build --release"
fi

$BCMD && cp -v target/release/hook_core.dll /d/Guild\ Wars\ 2/d3d9.dll && RUST_BACKTRACE=1 /d/Guild\ Wars\ 2/Gw2.exe -32