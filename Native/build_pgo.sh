set -e 

toolchain=$(rustup show active-toolchain | cut -d' ' -f 1 | sed -e 's/nightly-//')
echo $toolchain

pgodir=$(pwd)/pgo-data
rm -rf $pgodir 

# would be nice to try this, but it doesn't work on windows with msvc and panic=unwind
# https://github.com/rust-lang/rust/issues/61002
# even if don't use panic=unwind here, a bunch of crates that I depend on do use it, and 
# I can't turn it off there.

RUSTFLAGS="-Cprofile-generate=$pgodir" \
    cargo build --release --target=$toolchain

# would do more stuff here as described in 
# https://doc.rust-lang.org/rustc/profile-guided-optimization.html
