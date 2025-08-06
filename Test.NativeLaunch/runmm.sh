set -e

mmdir=../Native

RELEASE=0

# change into mm native dir, check target symlink and build mm dll
if [ "$RELEASE" -eq 1 ]; then
    (cd $mmdir && rm -f $mmdir/target && sh ./setjunc.sh >/dev/null && cargo build --release)
    cp -v $mmdir/target/release/hook_core.dll target/release/d3d11.dll
    RUST_BACKTRACE=full cargo run --release -- $@
else
    (cd $mmdir && rm -f $mmdir/target && sh ./setjunc.sh >/dev/null && cargo build)
    cp -v $mmdir/target/debug/hook_core.dll target/debug/d3d11.dll
    RUST_BACKTRACE=full cargo run -- $@
fi


# ad hoc benchmark functions for testing the load_mmobj module. intended
# to be pasted into a bash shell.
#cat /m/ModelMod/Logs/ModelMod.test_native_launch.log  | grep -i "M:SW:readmmob" | awk '{print $NF}' | sed 's/ms//' | awk '{s+=$1} END {print s}'

# function to record a new sample named by its first argument, fails if that files already exists
function record_sample {
    if [ -f $1 ]; then
        echo "existing sample: "
        cat $1 | awk '{print $NF}' | sed 's/ms//' | awk '{s+=$1} END {print s}'
        #echo "file $1 already exists"
        return
    fi
    cat /m/ModelMod/Logs/ModelMod.test_native_launch.log  | grep -i "M:SW:readmmob" > $1
    cat $1 | awk '{print $NF}' | sed 's/ms//' | awk '{s+=$1} END {print s}'
}

# average of the given N sample files
function avg_samples {
    for f in $@; do
        cat $f | awk '{print $NF}' | sed 's/ms//' | awk '{s+=$1} END {print s}'
    done | awk '{s+=$1} END {print s/NR}'
}
