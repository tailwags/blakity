//! With the `c` feature, builds upstream's portable C implementation with every
//! SIMD backend compiled out. measure.sh points CC at clang for the target and
//! adds the freestanding flags (see ../README.md).

fn main() {
    if std::env::var_os("CARGO_FEATURE_C").is_none() {
        return;
    }
    let c = "../../upstream/c";
    let mut build = cc::Build::new();
    for f in ["blake3.c", "blake3_dispatch.c", "blake3_portable.c"] {
        build.file(format!("{c}/{f}"));
        println!("cargo::rerun-if-changed={c}/{f}");
    }
    for d in ["BLAKE3_NO_SSE2", "BLAKE3_NO_SSE41", "BLAKE3_NO_AVX2", "BLAKE3_NO_AVX512"] {
        build.define(d, None);
    }
    build
        .define("BLAKE3_USE_NEON", "0")
        .define("NDEBUG", None)
        .include(c)
        .include("../c-shim")
        .flag("-std=c11")
        .flag("-ffreestanding")
        .flag("-fno-stack-protector")
        .flag("-ffunction-sections")
        .flag("-fdata-sections");
    build.compile("blake3_portable");
    println!("cargo::rerun-if-changed=../c-shim");
}
