set -e

mmdir=../Native

# change into mm native dir, check target symlink and build mm dll
(cd $mmdir && rm -f $mmdir/target && sh ./setjunc.sh >/dev/null && cargo build)
cp -v $mmdir/target/debug/hook_core.dll target/debug/d3d11.dll
cargo run -- $@