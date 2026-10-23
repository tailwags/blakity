# size: what BLAKE3 costs in a no_std binary

`hashsize` is a `#![no_std] #![no_main]` binary that hashes 4 KiB at a pointer
the compiler can't see (`core::hint::black_box` on the pointer and the length)
and publishes the digest through `black_box`. One cargo feature picks the
implementation:

| feature | implementation | API used |
| --- | --- | --- |
| *(none)* | baseline: copies the first 32 input bytes | |
| `impl-blakity` | `..` (this repo's crate), a plain path dependency | `Hasher::new/update/finalize` |
| `impl-reference` | `../upstream/reference_impl`, through `upstream/` | same |
| `impl-blake3` | crates.io `blake3` 1.8.7, `default-features = false` (SIMD with runtime detection) | `blake3::hash`, as bread does |
| `impl-blake3-pure` | the same with `features = ["pure"]` (bread's setup) | same |
| `impl-c` | upstream portable C (`blake3.c`, `blake3_dispatch.c`, `blake3_portable.c`, every `BLAKE3_NO_*` set, `BLAKE3_USE_NEON=0`), built with clang by `upstream/` | `blake3_hasher_*` |

`upstream/` is one small crate over the `../upstream` submodule, with a
`reference` and a `c` feature. `reference_impl` doesn't declare `#![no_std]`,
so it includes `reference_impl.rs` as a module, and its build script compiles
the C files only with `c`. It is a crate (not a module of `hashsize`) so that
the `bread` profile can give it opt-level 3, the same as the hash crate.

The reported cost of an implementation is its build minus the baseline built
the same way.

## Matrix

- **Targets:** `x86_64-unknown-uefi-sse2` (the main case: `targets/*.json`,
  the built-in UEFI spec with `+mmx,+sse,+sse2` and the hard-float ABI, like
  bread's custom target), `x86_64-unknown-uefi` (soft-float),
  `aarch64-unknown-uefi`, `x86_64-unknown-none`. Every target uses
  `-Zbuild-std=core,compiler_builtins` with bread's `optimize_for_size`
  (plus `compiler-builtins-mem` on `none`).
- **Profiles:** `bread` is bread's release profile exactly, and the first row
  of every table: fat LTO, opt-level z, the hash crate at opt-level 3, and its
  profile rustflags (`-Zfmt-debug=none -Cllvm-args=-enable-machine-outliner=always`).
  `size` (Cargo.toml) is the base for the sweep: codegen-units=1, panic=abort,
  strip, trim-paths. On top of it, with `--config profile.size.*`, the script
  sweeps `lto` ∈ {off, thin, fat} × `opt-level` ∈ {2, 3, s, z} × the hash
  crate's own opt-level ∈ {same, 3} × the machine outliner ∈ {off, on}
  (`OPT="1 2 3 s z"` adds opt-level 1 back). The
  hash crate is the one holding the implementation under test (`blakity`,
  `upstream` or `blake3`); its override also reaches the C code, since `cc`
  takes the package's opt-level. A hash opt-level equal to the global one is
  skipped as a duplicate. The `fat / z / 3 / on` row differs from `bread` only
  by `-Zfmt-debug=none`, which doesn't touch the hash code, so their deltas
  match.

## Running

```sh
size/measure.sh                       # whole matrix; enters the dev shell itself
TARGETS=x86_64-unknown-uefi-sse2 LTO=fat OPT="z 3" IMPLS="blakity blake3-pure" size/measure.sh
OUTLINER=1 HASH_OPT="same 3 s" OPT=z LTO=fat size/measure.sh   # other axes: see the script header
```

The script prints one markdown table per target. Baseline cells hold absolute
bytes and implementation cells hold the delta vs the baseline, both as
`sections (file)`. "Sections" means `.text` + `.rdata`/`.rodata` + `.data`
as `llvm-size -A` reports them. A PE file grows in 512 B `FileAlignment` steps,
so the sections column is the more precise of the two. Raw numbers are written
to `results/<target>.tsv` and the printed tables to `results/sizes.md`. Each target has its own target dir under
`target/<target>/`, and `JOBS` targets build at once. If any build fails, the
script prints cargo's output and exits non-zero (the last build's output is
also kept in `target/<target>/build.log`).

The dev shell (`../flake.nix`) provides the tools as `LLVM_SIZE` and
`HASHSIZE_CC`, and the script re-enters the shell when they're unset.
`HASHSIZE_CC` is `clang-unwrapped`: the nix-wrapped clang adds host hardening
flags that break these cross targets. The C code is built for the COFF/ELF
targets with `TARGET_CC=$HASHSIZE_CC`, and `c-shim/` stands in for
`<string.h>`/`<assert.h>`. Matching each Rust target's ABI also takes
`-mgeneral-regs-only` on the soft-float targets, `-mno-red-zone` everywhere on
x86_64, and `-fPIC` on `none`. The non-pure `blake3` crate needs the same
clang for its assembly (x86_64) and NEON C (aarch64).

Manual build of one combination, from `size/` inside `nix develop ..`:

```sh
cargo build --profile size --target x86_64-unknown-uefi \
  -Zbuild-std=core,compiler_builtins -Zbuild-std-features=optimize_for_size \
  -Zjson-target-spec -Zprofile-rustflags --features impl-blakity
```

## Caveats

- C is never LTO'd into the Rust code (there is no cross-language LTO, and
  clang's LLVM differs from rustc's). Upstream's `INLINE` is `always_inline`,
  so the C compression function is fully unrolled at every opt-level.
- `lto = false` (thin-local) is a no-op at codegen-units=1, so its rows match
  `off`. The default sweep leaves it out; `LTO="off false thin fat"` adds it back.
- Without fat LTO, opt-level 1/s/z baselines keep some core code (~1.5 KB),
  which shifts every delta in those rows.
