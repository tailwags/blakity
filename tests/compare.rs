// Adapted from upstream BLAKE3 `src/test.rs` (commit f55849f): the tests that
// compare the `blake3` crate against `reference_impl`, turned around so that
// `blake3` is the oracle and `blakity` is the implementation under test (it
// takes `reference_impl`'s place, and `crate::` becomes `blake3::`). Tests
// about `blake3` internals (platform, guts, compression functions, etc.) are
// not ported. Partial XOF reads have no blakity equivalent, since its only
// output API is `finalize(&mut [u8])`; the XOF tests vary the output length
// instead.

use blake3::{BLOCK_LEN, CHUNK_LEN};
use rand::prelude::*;

// Interesting input lengths to run tests on.
pub const TEST_CASES: &[usize] = &[
    0,
    1,
    2,
    3,
    4,
    5,
    6,
    7,
    8,
    BLOCK_LEN - 1,
    BLOCK_LEN,
    BLOCK_LEN + 1,
    2 * BLOCK_LEN - 1,
    2 * BLOCK_LEN,
    2 * BLOCK_LEN + 1,
    CHUNK_LEN - 1,
    CHUNK_LEN,
    CHUNK_LEN + 1,
    2 * CHUNK_LEN,
    2 * CHUNK_LEN + 1,
    3 * CHUNK_LEN,
    3 * CHUNK_LEN + 1,
    4 * CHUNK_LEN,
    4 * CHUNK_LEN + 1,
    5 * CHUNK_LEN,
    5 * CHUNK_LEN + 1,
    6 * CHUNK_LEN,
    6 * CHUNK_LEN + 1,
    7 * CHUNK_LEN,
    7 * CHUNK_LEN + 1,
    8 * CHUNK_LEN,
    8 * CHUNK_LEN + 1,
    16 * CHUNK_LEN - 1,
    16 * CHUNK_LEN, // AVX512's bandwidth
    16 * CHUNK_LEN + 1,
    31 * CHUNK_LEN - 1,
    31 * CHUNK_LEN, // 16 + 8 + 4 + 2 + 1
    31 * CHUNK_LEN + 1,
    100 * CHUNK_LEN, // subtrees larger than MAX_SIMD_DEGREE chunks
];

pub const TEST_CASES_MAX: usize = 100 * CHUNK_LEN;

pub const TEST_KEY: [u8; blake3::KEY_LEN] = *b"whats the Elvish word for friend";

// Paint the input with a repeating byte pattern. We use a cycle length of 251,
// because that's the largest prime number less than 256. This makes it
// unlikely to swapping any two adjacent input blocks or chunks will give the
// same answer.
pub fn paint_test_input(buf: &mut [u8]) {
    for (i, b) in buf.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }
}

#[test]
fn test_blakity_size() {
    // Not upstream. This pins the current layout of `blakity::Hasher`, which
    // is still identical to the reference implementation's. It is expected to
    // change as the implementation is improved: when it does, update the
    // number deliberately, so that size changes never go unnoticed.
    assert_eq!(1880, core::mem::size_of::<blakity::Hasher>());
}

#[test]
fn test_compare_blake3() {
    const OUT: usize = 303; // more than 64, not a multiple of 4
    let context = "BLAKE3 2019-12-27 16:13:59 example context (not the test vector one)";
    let mut input_buf = [0; TEST_CASES_MAX];
    paint_test_input(&mut input_buf);
    for &case in TEST_CASES {
        let input = &input_buf[..case];
        dbg!(case);

        let modes = [
            (blake3::Hasher::new(), blakity::Hasher::new()),
            (
                blake3::Hasher::new_keyed(&TEST_KEY),
                blakity::Hasher::new_keyed(&TEST_KEY),
            ),
            (
                blake3::Hasher::new_derive_key(context),
                blakity::Hasher::new_derive_key(context),
            ),
        ];
        for (mut expected_hasher, mut hasher) in modes {
            expected_hasher.update(input);
            let mut expected = [0; OUT];
            expected_hasher.finalize_xof().fill(&mut expected);

            hasher.update(input);
            // default length
            let mut out = [0; 32];
            hasher.finalize(&mut out);
            assert_eq!(out, expected[..32]);
            // extended, twice, since finalizing must not change the state
            for _ in 0..2 {
                let mut out = [0; OUT];
                hasher.finalize(&mut out);
                assert_eq!(out, expected);
            }
        }
    }
}

#[test]
fn test_compare_blake3_long_xof() {
    let mut expected = [0u8; 32 * BLOCK_LEN - 1];
    blake3::Hasher::new_keyed(&TEST_KEY)
        .update(b"hello world")
        .finalize_xof()
        .fill(&mut expected);

    let mut out = [0u8; 32 * BLOCK_LEN - 1];
    let mut hasher = blakity::Hasher::new_keyed(&TEST_KEY);
    hasher.update(b"hello world");
    hasher.finalize(&mut out);

    assert_eq!(expected, out);
}

#[test]
fn test_xof_lengths() {
    // Stands in for upstream's `test_xof_partial_blocks`: every output length
    // up to 6 blocks, including 0 and lengths that end mid-word.
    const OUT_LEN: usize = 6 * BLOCK_LEN;
    let mut expected = [0u8; OUT_LEN];
    blake3::Hasher::new().finalize_xof().fill(&mut expected);

    let hasher = blakity::Hasher::new();
    for len in 0..=OUT_LEN {
        let mut out = [0u8; OUT_LEN];
        hasher.finalize(&mut out[..len]);
        assert_eq!(out[..len], expected[..len]);
    }
}

fn blakity_hash(updates: &[&[u8]]) -> [u8; 32] {
    let mut hasher = blakity::Hasher::new();
    for update in updates {
        hasher.update(update);
    }
    let mut out = [0; 32];
    hasher.finalize(&mut out);
    out
}

#[test]
fn test_compare_update_multiple() {
    // Don't use all the long test cases here, since that's unnecessarily slow
    // in debug mode.
    let mut short_test_cases = TEST_CASES;
    while *short_test_cases.last().unwrap() > 4 * CHUNK_LEN {
        short_test_cases = &short_test_cases[..short_test_cases.len() - 1];
    }
    assert_eq!(*short_test_cases.last().unwrap(), 4 * CHUNK_LEN);

    let mut input_buf = [0; 2 * TEST_CASES_MAX];
    paint_test_input(&mut input_buf);

    for &first_update in short_test_cases {
        dbg!(first_update);
        for &second_update in short_test_cases {
            dbg!(second_update);
            let total_input = &input_buf[..first_update + second_update];
            let (first_input, second_input) = total_input.split_at(first_update);
            let expected = blake3::hash(total_input);
            assert_eq!(expected, blakity_hash(&[first_input, second_input]));
        }
    }
}

#[test]
fn test_compare_large_input() {
    // Not upstream. The other tests stop at 100 chunks, which fill at most 6
    // CV stack entries; 64 MiB + 1 byte fills 16 (at chunk 2^16 - 1), closer
    // to the kernels and initrds bread hashes. The updates are a prime number of bytes so that
    // they end mid-block and mid-chunk, merging the tree mid-update.
    const LEN: usize = (1 << 16) * CHUNK_LEN + 1;
    const UPDATE_LEN: usize = 1_000_003;
    let mut input = vec![0; LEN];
    paint_test_input(&mut input);

    let mut hasher = blakity::Hasher::new();
    for update in input.chunks(UPDATE_LEN) {
        hasher.update(update);
    }
    let mut out = [0; 32];
    hasher.finalize(&mut out);
    assert_eq!(blake3::hash(&input), out);
}

#[test]
fn test_fuzz_hasher() {
    const INPUT_MAX: usize = 4 * CHUNK_LEN;
    let mut input_buf = [0; 3 * INPUT_MAX];
    paint_test_input(&mut input_buf);

    // Use a fixed RNG seed for reproducibility.
    let mut rng = chacha20::ChaCha8Rng::from_seed([1; 32]);
    for _num_test in 0..10_000 {
        dbg!(_num_test);
        let mut updates = [&[][..]; 3];
        let mut total_input = 0;
        // For each test, write 3 inputs of random length.
        for update in &mut updates {
            let input_len = rng.random_range(0..(INPUT_MAX + 1));
            dbg!(input_len);
            *update = &input_buf[total_input..][..input_len];
            total_input += input_len;
        }
        let expected = blake3::hash(&input_buf[..total_input]);
        assert_eq!(expected, blakity_hash(&updates));
    }
}

#[test]
fn test_fuzz_xof() {
    let mut input_buf = [0u8; 3 * BLOCK_LEN];
    paint_test_input(&mut input_buf);

    // Use a fixed RNG seed for reproducibility.
    let mut rng = chacha20::ChaCha8Rng::from_seed([1; 32]);
    for _num_test in 0..2500 {
        dbg!(_num_test);
        // up to 31 (16 + 8 + 4 + 2 + 1) chunks of output
        let input_len = rng.random_range(0..input_buf.len());
        let output_len = rng.random_range(0..=31 * CHUNK_LEN);
        let input = &input_buf[..input_len];

        let mut expected = [0; 31 * CHUNK_LEN];
        blake3::Hasher::new()
            .update(input)
            .finalize_xof()
            .fill(&mut expected[..output_len]);

        let mut out = [0; 31 * CHUNK_LEN];
        let mut hasher = blakity::Hasher::new();
        hasher.update(input);
        hasher.finalize(&mut out[..output_len]);

        assert_eq!(expected[..output_len], out[..output_len]);
    }
}

#[test]
fn test_compare_hash_and_hex() {
    // Not upstream. `blakity::hash` and `blakity::Hash`'s hex conversions,
    // against the `blake3` crate's.
    let mut input_buf = [0; TEST_CASES_MAX];
    paint_test_input(&mut input_buf);
    for &case in TEST_CASES {
        let input = &input_buf[..case];
        let expected = blake3::hash(input);
        let hash = blakity::hash(input);
        assert_eq!(hash.as_bytes(), expected.as_bytes());

        let hex = hash.to_string();
        assert_eq!(hex, expected.to_hex().as_str());
        assert_eq!(blakity::Hash::from_hex(&hex), Some(hash));
        assert_eq!(blakity::Hash::from_hex(&hex.to_uppercase()), Some(hash));
    }
}

#[test]
fn test_from_hex_invalid() {
    // Not upstream.
    let valid = blakity::hash(b"").to_string();
    assert!(blakity::Hash::from_hex(&valid).is_some());
    assert!(blakity::Hash::from_hex("").is_none());
    assert!(blakity::Hash::from_hex(&valid[1..]).is_none());
    assert!(blakity::Hash::from_hex(&format!("{valid}0")).is_none());
    // A non-hex character anywhere, including the ones that integer parsing
    // accepts ('+') and a multibyte one that keeps the byte length at 64.
    for bad in ["g", "+", " ", "é"] {
        for at in [0, 31, 62] {
            let mut hex = valid.clone();
            hex.replace_range(at..at + bad.len(), bad);
            assert_eq!(hex.len(), 64);
            assert!(blakity::Hash::from_hex(&hex).is_none(), "{hex}");
        }
    }
}
