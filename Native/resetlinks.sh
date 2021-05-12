set -e

DIRS=$(ls)

WD=$(pwd)
for d in $DIRS; do
    libsrc="$WD/$d/src/lib.rs"
    echo "Checking $libsrc"
    if [ -f "$libsrc" ]; then
        cd $WD/$d
        echo "Resetting link in $WD/$d"
        rm -rf ./target
        sh ../setjunc.sh ModelMod
    fi
done