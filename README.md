# blakity

A tiny, portable BLAKE3, derived from the upstream
[reference implementation](https://github.com/BLAKE3-team/BLAKE3/tree/master/reference_impl).

## Comparison

Speed, MB/s (x86_64 host, fat LTO, opt-level 3; `just bench`, `just bench-portable`):

| input    | blakity | reference_impl | blake3 (portable) | blake3 (SIMD) | C (SIMD) |
| -------- | ------: | -------------: | ----------------: | ------------: | -------: |
| 1 KiB    |     970 |            974 |              1638 |          1008 |     1013 |
| 64 KiB   |     924 |            925 |              1534 |         12752 |    12857 |
| 1024 KiB |     925 |            918 |              1527 |         13018 |    13131 |

Bytes added to a `no_std` binary (code + data sections, over a baseline that does
not hash; bread's profile: opt-level z, the hash crate at 3, fat LTO; `just size`):

| target                           | blakity | reference_impl | blake3 `pure` | blake3 | C portable |
| -------------------------------- | ------: | -------------: | ------------: | -----: | ---------: |
| x86_64-unknown-uefi+SSE2 (bread) |   4,041 |          4,041 |        17,688 | 57,608 |     16,641 |
| x86_64-unknown-uefi              |   4,461 |          4,461 |         9,852 | 58,460 |     15,681 |
| aarch64-unknown-uefi             |   2,816 |          2,816 |         5,456 | 10,232 |     12,748 |
| x86_64-unknown-none              |   4,303 |          4,304 |        12,229 | 58,582 |     15,663 |

## License

Like upstream BLAKE3, at your option: [CC0-1.0](LICENSE_CC0),
[Apache-2.0](LICENSE_A2), or [Apache-2.0 WITH LLVM-exception](LICENSE_A2LLVM).
