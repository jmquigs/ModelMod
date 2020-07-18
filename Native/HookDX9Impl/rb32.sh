rustup default nightly-i686-pc-windows-msvc

if [ "$1" == "profile" ]; then
BCMD="cargo build --release --features=profile"
else
BCMD="cargo build --release"
fi

$BCMD && cp -v target/release/mm_native.dll /d/Guild\ Wars\ 2/d3d9.dll && RUST_BACKTRACE=1 /d/Guild\ Wars\ 2/Gw2.exe -32