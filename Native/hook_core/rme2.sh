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
GPATH="/d"

if [ ! -d "$GPATH" ]; then
    GPATH="/e"
fi

GREL="Steam/steamapps/common/mass effect 2/Binaries"

echo "TDIR:"
set -e
ls -ld "$GPATH/$GREL"
#exit 1
$BCMD && cp -v target/release/hook_core.dll "$GPATH/$GREL/d3d9.dll" && RUST_BACKTRACE=1 "$GPATH/$GREL/MassEffect2.exe"
