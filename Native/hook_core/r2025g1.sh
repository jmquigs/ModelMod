SPATH=$(dirname $0)
. $SPATH/shutil.sh
REQ=x86_64
check_tc $REQ

set -e 
# find the parent MM directory, where the symlink to the game exe should be set
# (developer should manually set that up with mklink /j )
MMPATH=$SPATH

find_mm

echo "MM: $MMPATH"

# find the symlink to the exe
GLINK="G2025g1"

GPATH="$MMPATH/$GLINK"
if [ ! -f "$GPATH" ]; then 
    echo "Game symlink $GPATH does not exist" 
    exit 1
fi 

# possible features:
#   profile
#   mmdisable
if [ "$1" != "" ]; then
    echo "Building with features: $1"
    BCMD="cargo build --release --features=$1"
else
    BCMD="cargo build --release"
fi

echo "==> Using d3d11"
DEST=$(dirname "$(readlink $GPATH)")
DEST=$DEST/d3d11.dll

$BCMD 

# this is the rust "source" target dir, not the copy dest
TARGDIR="target"
if [ ! -d "TARGDIR" ]; then 
    TARGDIR="../target"
fi 
cp -v $TARGDIR/release/hook_core.dll "$DEST" 
echo "press enter to run game now or ctrl-c to abort..."

read $discard

if [ -f "$MMPATH/g2025g1_pre.sh" ]; then 
    set +e 
    source "$MMPATH/g2025g1_pre.sh"
    if [ "$?" -ne 0 ]; then 
        echo "pre script had error, aborting"
        exit 1 
    fi 
    echo "ran pre script"
    set -e
fi 

REXE="$(readlink $GPATH)"
export RUST_BACKTRACE=1 && "$REXE"
echo "game has exited"