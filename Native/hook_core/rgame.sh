#!/bin/bash

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

# $1 is the game name (used as the symlink name)
if [ "$1" == "" ]; then
    echo "Usage: $0 <game_name> [features]"
    echo "  e.g.: $0 2025g1"
    exit 1
fi
GLINK="G$1"

GPATH="$MMPATH/$GLINK"
if [ ! -f "$GPATH" ]; then
    echo "Possible game symlink $GPATH does not exist"
    GPATH="$MMPATH/../GameLink/$GLINK"
    if [ ! -f "$GPATH" ]; then
        echo "Possible game symlink $GPATH does not exist"
        GPATH="$MMPATH/../GameLink/$GLINK.$(hostname)"
        if [ ! -f "$GPATH" ]; then
            echo "Possible game symlink $GPATH does not exist"
            exit 1
        fi
    fi
fi
echo "Using game symlink: $GPATH"

if [[ "$(uname -s)" == "Linux" ]]; then
    BUILD="cargo xwin build --release --target x86_64-pc-windows-msvc"
else
    BUILD="cargo build --release"
fi

# possible features:
#   profile
#   mmdisable
if [ "$2" != "" ]; then
    echo "Building with features: $2"
    BCMD="$BUILD --features=$2"
else
    BCMD="$BUILD"
fi

echo "==> Using d3d11"
DEST=$(dirname "$(readlink $GPATH)")
DEST=$DEST/d3d11.dll

$BCMD 

# this is the rust "source" target dir, not the copy dest
if [[ "$(uname -s)" == "Linux" ]]; then
    TARGDIR="target/x86_64-pc-windows-msvc"
    if [ ! -d "$TARGDIR" ]; then
        TARGDIR="../target/x86_64-pc-windows-msvc"
    fi
else
    TARGDIR="target"
    if [ ! -d "$TARGDIR" ]; then
        TARGDIR="../target"
    fi
fi
cp -v $TARGDIR/release/hook_core.dll "$DEST" 
echo "press enter to run game now or ctrl-c to abort..."

read $discard

PRESCRIPT="$MMPATH/${GLINK,}_pre.sh"
if [ -f "$PRESCRIPT" ]; then 
    set +e 
    source "$PRESCRIPT"
    if [ "$?" -ne 0 ]; then 
        echo "pre script had error, aborting"
        exit 1 
    fi 
    echo "ran pre script"
    set -e
fi 

# if the pre script defined a launch_helper function, call it, otherwise just launch the game exe
if declare -F launch_helper > /dev/null; then
    launch_helper
else
    REXE="$(readlink $GPATH)"
    export RUST_BACKTRACE=1 && "$REXE"
    echo "game has exited"
fi
