//! Speed benchmarks: `blakity` against upstream BLAKE3 implementations.
//!
//! Adapted from upstream BLAKE3 `benches/bench.rs` (same `RandomInput`, same
//! input sizes, same naming style). Every benchmark hashes the same kind of
//! input (random bytes at a random page offset) to a 32-byte output.
//!
//! Implementations, by bench name prefix:
//!
//! - `bench_blakity_*`: this crate (`blakity::Hasher`, new + update + finalize).
//! - `bench_reference_*`: upstream `reference_impl`, the baseline blakity
//!   started from. Same API, so it is measured the same way.
//! - `bench_atonce_*` / `bench_incremental_*`: the official `blake3` crate,
//!   `blake3::hash` vs `Hasher::new().update().finalize()`. Default features,
//!   i.e. SIMD with runtime CPU detection.
//! - `bench_c_*`: the official C implementation (via `blake3_c_rust_bindings`,
//!   also SIMD with runtime detection). The C API has no one-shot hash, so
//!   init + update + finalize is the only variant (as upstream).
//!
//! Sizes go from one 64-byte block to 1024 KiB.
//!
//! Running (from the crate root, inside `nix develop .`):
//!
//! ```sh
//! cargo bench                    # everything
//! cargo bench blakity            # only blakity (substring filter on the name)
//! cargo bench 1024_kib           # one size, all implementations
//! # official crate without SIMD (see below):
//! cargo bench -F blake3/pure,blake3/no_sse2,blake3/no_sse41,blake3/no_avx2,blake3/no_avx512 atonce
//! just bench-outliner            # with -Cllvm-args=-enable-machine-outliner=always
//! benches/matrix.sh              # lto x opt-level sweep, see that script
//! ```
//!
//! The official crate's portable (no SIMD) code is the fairest upper bound for
//! blakity, which uses no explicit SIMD (bread's SSE2 UEFI target only gets
//! what LLVM auto-vectorizes). `blake3/pure` alone only drops the C/assembly;
//! the Rust SSE/AVX intrinsics stay, so the `no_*` features are needed too.
//! Cargo cannot link the same crate twice with different features, so rather
//! than a second dependency this is a feature flag on the command line: with
//! it, the `bench_atonce_*` / `bench_incremental_*` rows measure the portable
//! code.
//!
//! Reading the output: each line reads `X ns/iter (+/- Y) = Z MB/s`. ns/iter
//! is the time for one hash of the given size, and MB/s is the throughput
//! (from `b.bytes`), which is the column to compare across implementations.
//! Small sizes are dominated by per-call overhead, large ones by the
//! compression function and (for SIMD implementations) by parallelism across
//! chunks.

#![feature(test)]

extern crate test;

use rand::prelude::*;
use test::Bencher;

const KIB: usize = 1024;
const BLOCK_LEN: usize = 64;

// This struct randomizes two things:
// 1. The actual bytes of input.
// 2. The page offset the input starts at.
pub struct RandomInput {
    buf: Vec<u8>,
    len: usize,
    offsets: Vec<usize>,
    offset_index: usize,
}

impl RandomInput {
    pub fn new(b: &mut Bencher, len: usize) -> Self {
        b.bytes += len as u64;
        let page_size: usize = page_size::get();
        let mut buf = vec![0u8; len + page_size];
        let mut rng = rand::rng();
        rng.fill_bytes(&mut buf);
        let mut offsets: Vec<usize> = (0..page_size).collect();
        offsets.shuffle(&mut rng);
        Self {
            buf,
            len,
            offsets,
            offset_index: 0,
        }
    }

    pub fn get(&mut self) -> &[u8] {
        let offset = self.offsets[self.offset_index];
        self.offset_index += 1;
        if self.offset_index >= self.offsets.len() {
            self.offset_index = 0;
        }
        &self.buf[offset..][..self.len]
    }
}

/// Expands to one `#[bench]` per input size, named `<prefix>_<size>`, each
/// calling `$f(b, len)`.
macro_rules! size_sweep {
    ($f:ident: $($name:ident = $len:expr),* $(,)?) => {
        $(
            #[bench]
            fn $name(b: &mut Bencher) {
                $f(b, $len);
            }
        )*
    };
}

fn bench_blakity(b: &mut Bencher, len: usize) {
    let mut input = RandomInput::new(b, len);
    b.iter(|| {
        let mut hasher = blakity::Hasher::new();
        hasher.update(input.get());
        let mut out = [0; 32];
        hasher.finalize(&mut out);
        out
    });
}

size_sweep!(bench_blakity:
    bench_blakity_0001_block = BLOCK_LEN,
    bench_blakity_0001_kib = 1 * KIB,
    bench_blakity_0002_kib = 2 * KIB,
    bench_blakity_0004_kib = 4 * KIB,
    bench_blakity_0008_kib = 8 * KIB,
    bench_blakity_0016_kib = 16 * KIB,
    bench_blakity_0032_kib = 32 * KIB,
    bench_blakity_0064_kib = 64 * KIB,
    bench_blakity_0128_kib = 128 * KIB,
    bench_blakity_0256_kib = 256 * KIB,
    bench_blakity_0512_kib = 512 * KIB,
    bench_blakity_1024_kib = 1024 * KIB,
);

fn bench_reference(b: &mut Bencher, len: usize) {
    let mut input = RandomInput::new(b, len);
    b.iter(|| {
        let mut hasher = reference_impl::Hasher::new();
        hasher.update(input.get());
        let mut out = [0; 32];
        hasher.finalize(&mut out);
        out
    });
}

size_sweep!(bench_reference:
    bench_reference_0001_block = BLOCK_LEN,
    bench_reference_0001_kib = 1 * KIB,
    bench_reference_0002_kib = 2 * KIB,
    bench_reference_0004_kib = 4 * KIB,
    bench_reference_0008_kib = 8 * KIB,
    bench_reference_0016_kib = 16 * KIB,
    bench_reference_0032_kib = 32 * KIB,
    bench_reference_0064_kib = 64 * KIB,
    bench_reference_0128_kib = 128 * KIB,
    bench_reference_0256_kib = 256 * KIB,
    bench_reference_0512_kib = 512 * KIB,
    bench_reference_1024_kib = 1024 * KIB,
);

fn bench_atonce(b: &mut Bencher, len: usize) {
    let mut input = RandomInput::new(b, len);
    b.iter(|| blake3::hash(input.get()));
}

size_sweep!(bench_atonce:
    bench_atonce_0001_block = BLOCK_LEN,
    bench_atonce_0001_kib = 1 * KIB,
    bench_atonce_0002_kib = 2 * KIB,
    bench_atonce_0004_kib = 4 * KIB,
    bench_atonce_0008_kib = 8 * KIB,
    bench_atonce_0016_kib = 16 * KIB,
    bench_atonce_0032_kib = 32 * KIB,
    bench_atonce_0064_kib = 64 * KIB,
    bench_atonce_0128_kib = 128 * KIB,
    bench_atonce_0256_kib = 256 * KIB,
    bench_atonce_0512_kib = 512 * KIB,
    bench_atonce_1024_kib = 1024 * KIB,
);

fn bench_incremental(b: &mut Bencher, len: usize) {
    let mut input = RandomInput::new(b, len);
    b.iter(|| blake3::Hasher::new().update(input.get()).finalize());
}

size_sweep!(bench_incremental:
    bench_incremental_0001_block = BLOCK_LEN,
    bench_incremental_0001_kib = 1 * KIB,
    bench_incremental_0002_kib = 2 * KIB,
    bench_incremental_0004_kib = 4 * KIB,
    bench_incremental_0008_kib = 8 * KIB,
    bench_incremental_0016_kib = 16 * KIB,
    bench_incremental_0032_kib = 32 * KIB,
    bench_incremental_0064_kib = 64 * KIB,
    bench_incremental_0128_kib = 128 * KIB,
    bench_incremental_0256_kib = 256 * KIB,
    bench_incremental_0512_kib = 512 * KIB,
    bench_incremental_1024_kib = 1024 * KIB,
);

fn bench_c_incremental(b: &mut Bencher, len: usize) {
    let mut input = RandomInput::new(b, len);
    b.iter(|| {
        let mut hasher = blake3_c_rust_bindings::Hasher::new();
        hasher.update(input.get());
        let mut out = [0; 32];
        hasher.finalize(&mut out);
        out
    });
}

size_sweep!(bench_c_incremental:
    bench_c_incremental_0001_block = BLOCK_LEN,
    bench_c_incremental_0001_kib = 1 * KIB,
    bench_c_incremental_0002_kib = 2 * KIB,
    bench_c_incremental_0004_kib = 4 * KIB,
    bench_c_incremental_0008_kib = 8 * KIB,
    bench_c_incremental_0016_kib = 16 * KIB,
    bench_c_incremental_0032_kib = 32 * KIB,
    bench_c_incremental_0064_kib = 64 * KIB,
    bench_c_incremental_0128_kib = 128 * KIB,
    bench_c_incremental_0256_kib = 256 * KIB,
    bench_c_incremental_0512_kib = 512 * KIB,
    bench_c_incremental_1024_kib = 1024 * KIB,
);
