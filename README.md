# blakity

A tiny, portable BLAKE3, optimized for size and speed without needing SIMD.

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

## License

Like upstream BLAKE3, at your option: [CC0-1.0](LICENSE_CC0),
[Apache-2.0](LICENSE_A2), or [Apache-2.0 WITH LLVM-exception](LICENSE_A2LLVM).
