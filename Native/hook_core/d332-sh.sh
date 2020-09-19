if [ "$1" == "profile" ]; then
BCMD="cargo build --release --features=profile"
else
BCMD="cargo build --release"
fi

$BCMD && cp -v target/release/hook_core.dll  /d/Diablo\ 3/Diablo\ III/d3d9.dll && RUST_BACKTRACE=1  /d/Diablo\ 3/Diablo\ III/Diablo\ III.exe