# blakity

_A tiny, portable BLAKE3, optimized for size and speed without needing SIMD_

[![Crates.io](https://img.shields.io/crates/v/blakity.svg)](https://crates.io/crates/blakity)
[![Documentation](https://docs.rs/blakity/badge.svg)](https://docs.rs/blakity)
[![License](https://img.shields.io/crates/l/blakity.svg)](#license)

blakity is a `#![no_std]` [BLAKE3](https://github.com/BLAKE3-team/BLAKE3) in a
single file with no dependencies, derived from upstream's `reference_impl`. It
targets small binaries, such as UEFI applications, while staying faster than the
reference implementation and needing no SIMD or platform-specific code.

## Usage

Add blakity to your `Cargo.toml`:

```toml
[dependencies]
blakity = "0.1.0"
```

The API is the same as upstream's `reference_impl`. `finalize` fills an output
slice of any length, so it covers both the 32-byte hash and the extendable
output:

```rust
let mut hasher = blakity::Hasher::new();
hasher.update(b"abc");
hasher.update(b"def");
let mut hash = [0; 32];
hasher.finalize(&mut hash);
```

`Hasher::new_keyed(&[u8; 32])` gives the keyed hash and
`Hasher::new_derive_key(context)` the key derivation function.

blakity has no SIMD and no multithreading, so for large inputs on a host with
`std`, the official [`blake3`](https://crates.io/crates/blake3) crate is much
faster.

## Comparison

### Speed

Throughput in MB/s on an x86_64 host (fat LTO, `opt-level = 3`).
Run with `just bench` and `just bench-portable`.

| input    | blakity | reference_impl | blake3 (portable) | blake3 (SIMD) | C (SIMD) |
| -------- | ------: | -------------: | ----------------: | ------------: | -------: |
| 1 KiB    |   1,227 |            968 |             1,592 |           999 |    1,003 |
| 64 KiB   |   1,155 |            916 |             1,524 |        12,835 |   12,872 |
| 1024 KiB |   1,153 |            913 |             1,524 |        12,857 |   13,071 |

### Size

Bytes added to a `no_std` binary (code + data sections), measured against a
baseline that doesn't hash. Built with bread's profile: `opt-level = "z"`, the
hash crate at `opt-level = 3`, fat LTO. Run with `just size`.

| target                           | blakity | reference_impl | blake3 (`pure`) | blake3 | C (portable) |
| -------------------------------- | ------: | -------------: | --------------: | -----: | -----------: |
| x86_64-unknown-uefi+SSE2 (bread) |   3,240 |          4,041 |          17,688 | 57,608 |       16,641 |
| x86_64-unknown-uefi              |   3,392 |          4,461 |           9,852 | 58,460 |       15,681 |
| aarch64-unknown-uefi             |   2,720 |          2,816 |           5,456 | 10,232 |       12,748 |
| x86_64-unknown-none              |   3,358 |          4,304 |          12,229 | 58,582 |       15,663 |

## Scope

blakity is developed alongside [bread](https://github.com/bread-bootloader/bread),
an experimental UEFI bootloader for Linux, where it is tuned for an x86_64 UEFI
target with SSE2.

## License

Like upstream BLAKE3, at your option: [CC0-1.0](LICENSE_CC0),
[Apache-2.0](LICENSE_A2), or [Apache-2.0 WITH LLVM-exception](LICENSE_A2LLVM).

`src/lib.rs` is derived from the
[`reference_impl`](https://github.com/BLAKE3-team/BLAKE3/tree/master/reference_impl)
of [BLAKE3](https://github.com/BLAKE3-team/BLAKE3) by the BLAKE3 team, used
under the same licenses.
