_:
    @just --list

# Fetch the upstream submodule if it isn't checked out
_upstream:
    @[ -f upstream/Cargo.toml ] || git submodule update --init

# Run blakity's tests (vectors, and cross-checks against the blake3 crate); ARGS go to cargo test
test *ARGS: _upstream
    cargo test {{ARGS}}

# Speed benches; FILTER is a bench name substring, e.g. `blakity` or `1024_kib`
bench *FILTER: _upstream
    cargo bench {{FILTER}}

# Speed benches with the official Rust crate forced onto its portable (no SIMD) code
bench-portable *FILTER: _upstream
    cargo bench -F blake3/pure,blake3/no_sse2,blake3/no_sse41,blake3/no_avx2,blake3/no_avx512 {{FILTER}}

# Speed benches with LLVM's machine outliner forced on for every crate (own target dir)
bench-outliner *FILTER: _upstream
    CARGO_TARGET_DIR=target/bench-outliner RUSTFLAGS="-Cllvm-args=-enable-machine-outliner=always" cargo bench {{FILTER}}

# Speed benches under every lto x opt-level combination
bench-matrix *FILTER: _upstream
    benches/matrix.sh {{FILTER}}

# no_std binary sizes; filter with TARGETS=, IMPLS=, LTO=, OPT=, HASH_OPT=, OUTLINER=, BREAD=, JOBS= (see size/measure.sh)
size: _upstream
    size/measure.sh
