# this used to be entirely in bash but I ported it to Fake since this didn't work on
# appveyor.  But then appveyor has cargo issues (it can't reliably update its index due to
# some SSL issue).  But I still kept the new Fake implementation

# search upwards in the directory tree for the build.fsx file
DIR="./"
for i in {1..4}; do
    if [ -f "$DIR/build.fsx" ]; then
        break
    fi
    DIR="$DIR/.."
done

if [ ! -f "$DIR/build.fsx" ]; then
    echo "Can't find build.fsx"
    exit 1
fi

(cd $DIR && TARGET=BuildNativeOnly fsi build.fsx)

