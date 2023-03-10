set -e

CLEANARG=$1

TC_X64=stable-x86_64-pc-windows-msvc
TC_X32=stable-i686-pc-windows-msvc

ACTIVE_TC=$(rustup show active-toolchain | awk '{print $1}')

if [ ! -d "../Release" ]; then
    echo "../Release not found, are you running this script from the 'Native' directory?"
    exit 1
fi

function copy_to_dest {
    local dest=$1
    if [ ! -d ../Release/$dest ]; then
    mkdir ../Release/$dest
    fi
    if [ -f ../Release/$dest/d3d9.dll ]; then
        rm -fv ../Release/$dest/d3d9.dll
    fi
    if [ -f ../Release/$dest/d3d11.dll ]; then
        rm -fv ../Release/$dest/d3d11.dll
    fi
    cp -a target/release/hook_core.dll ../Release/$dest/d3d9.dll
    cp -a target/release/hook_core.dll ../Release/$dest/d3d11.dll
}

function clean {
    # if "target" is a symlink remove and recreate it
    if [ -L target ]; then
        if [ -d "target/debug" ]; then
            rm -rf target/debug
        fi
        if [ -d "target/release" ]; then
            rm -rf target/release
        fi
    else
        cargo clean
    fi

}
rustup default $TC_X64
if [ "$CLEANARG" != "noclean" ]; then
    clean
fi
cargo build --release
copy_to_dest modelmod_64

rustup default $TC_X32
if [ "$CLEANARG" != "noclean" ]; then
    clean
fi

cargo build --release
copy_to_dest modelmod_32

rustup default $ACTIVE_TC
if [ "$CLEANARG" != "noclean" ]; then
    clean
fi

echo "=== Toolchain reset to $TC_X64"