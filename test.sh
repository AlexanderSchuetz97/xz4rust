#!/usr/bin/env bash
set -e

#Valgrind
cargo clean
# This cannot be done with newer versions of rust because there are memory leaks in the standard library that the maintainers refuse to fix.
rustup default 1.84.1
export VALGRINDFLAGS="--error-exitcode=1 --leak-check=full --show-leak-kinds=all"
cargo valgrind test --package xz4rust --test tiny_stack test_tiny_stack -- --exact
cargo clean

rustup default stable
cargo msrv verify

# 64 bit little endian
cross test --target x86_64-unknown-linux-gnu
cross test --target x86_64-unknown-linux-musl
cross test --release --target x86_64-unknown-linux-gnu
cross test --release --target x86_64-unknown-linux-musl

# 32 bit little endian
cross test --target i686-unknown-linux-gnu
cross test --target i686-unknown-linux-musl
cross test --release --target i686-unknown-linux-gnu
cross test --release --target i686-unknown-linux-musl

#64 bit big endian
cross test --target s390x-unknown-linux-gnu
cross test --release --target s390x-unknown-linux-gnu

#32 bit big endian
cross test --target powerpc-unknown-linux-gnu
cross test --release --target powerpc-unknown-linux-gnu

#Windows (this test requires mingw and wine and wine-binfmt to work)
cargo test --target x86_64-pc-windows-gnu
cargo test --target i686-pc-windows-gnu
cargo test --release --target x86_64-pc-windows-gnu
cargo test --release --target i686-pc-windows-gnu

#Miri
cargo +nightly miri test --package xz4rust --test tiny_stack test_tiny_stack -- --exact
cargo +nightly miri test --package xz4rust --test test_static_uninit test_static_uninit -- --exact

# Verify minimum rust version works
cargo msrv verify

# Clippy checks and checks if it compiles in some combinations
/usr/bin/env bash clippy.sh

# Build examples
cargo build --examples

# Run bench just to make sure I didnt break it!
cargo +nightly bench

#Test some edge cases with usize/u32
export TEST_LARGE_SEED=true
cargo test --package xz4rust --test test_large_seeded --target x86_64-unknown-linux-gnu
cargo test --package xz4rust --test test_large_seeded --target i686-unknown-linux-gnu
