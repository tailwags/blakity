// Adapted from upstream BLAKE3 `test_vectors/src/lib.rs` (commit f55849f): the
// `reference_impl` checks, with `reference_impl::` replaced by `blakity::` (and
// the `test_reference_impl_*` helpers renamed to `test_blakity_*`
// accordingly). The checks of the `blake3` crate and the JSON generator are
// dropped.

use blake3::KEY_LEN;
use serde::Deserialize;

// Reading files at runtime requires special configuration under WASM/WASI, so including this at
// compile time is simpler.
const TEST_VECTORS_JSON: &str = include_str!("../upstream/test_vectors/test_vectors.json");

pub const TEST_CONTEXT: &str = "BLAKE3 2019-12-27 16:29:52 test vectors context";

// Paint the input with a repeating byte pattern. We use a cycle length of 251,
// because that's the largest prime number less than 256. This makes it
// unlikely to swapping any two adjacent input blocks or chunks will give the
// same answer.
pub fn paint_test_input(buf: &mut [u8]) {
    for (i, b) in buf.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }
}

#[derive(Debug, Deserialize)]
pub struct Cases {
    pub _comment: String,
    pub key: String,
    pub context_string: String,
    pub cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
pub struct Case {
    pub input_len: usize,
    pub hash: String,
    pub keyed_hash: String,
    pub derive_key: String,
}

pub fn parse_test_cases() -> Cases {
    serde_json::from_str(TEST_VECTORS_JSON).expect("failed to parse test_vectors.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Each expected output is extended, and the vectors also ask to check its
    // first 32 bytes against the default-length output.
    fn check(hasher: &blakity::Hasher, expected: &[u8]) {
        let mut out = vec![0; expected.len()];
        hasher.finalize(&mut out);
        assert_eq!(expected, &out[..]);

        let mut out = [0; 32];
        hasher.finalize(&mut out);
        assert_eq!(expected[..32], out);
    }

    fn test_blakity_all_at_once(
        key: &[u8; KEY_LEN],
        input: &[u8],
        expected_hash: &[u8],
        expected_keyed_hash: &[u8],
        expected_derive_key: &[u8],
    ) {
        let mut hasher = blakity::Hasher::new();
        hasher.update(input);
        check(&hasher, expected_hash);

        let mut hasher = blakity::Hasher::new_keyed(key);
        hasher.update(input);
        check(&hasher, expected_keyed_hash);

        let mut hasher = blakity::Hasher::new_derive_key(TEST_CONTEXT);
        hasher.update(input);
        check(&hasher, expected_derive_key);
    }

    fn test_blakity_one_at_a_time(
        key: &[u8; KEY_LEN],
        input: &[u8],
        expected_hash: &[u8],
        expected_keyed_hash: &[u8],
        expected_derive_key: &[u8],
    ) {
        let mut hasher = blakity::Hasher::new();
        for &b in input {
            hasher.update(&[b]);
        }
        check(&hasher, expected_hash);

        let mut hasher = blakity::Hasher::new_keyed(key);
        for &b in input {
            hasher.update(&[b]);
        }
        check(&hasher, expected_keyed_hash);

        let mut hasher = blakity::Hasher::new_derive_key(TEST_CONTEXT);
        for &b in input {
            hasher.update(&[b]);
        }
        check(&hasher, expected_derive_key);
    }

    #[test]
    fn run_test_vectors() {
        let cases = parse_test_cases();
        let key: &[u8; KEY_LEN] = cases.key.as_bytes().try_into().unwrap();
        assert_eq!(TEST_CONTEXT, cases.context_string);
        for case in &cases.cases {
            dbg!(case.input_len);
            let mut input = vec![0; case.input_len];
            paint_test_input(&mut input);
            let expected_hash = hex::decode(&case.hash).unwrap();
            let expected_keyed_hash = hex::decode(&case.keyed_hash).unwrap();
            let expected_derive_key = hex::decode(&case.derive_key).unwrap();

            test_blakity_all_at_once(
                key,
                &input,
                &expected_hash,
                &expected_keyed_hash,
                &expected_derive_key,
            );

            test_blakity_one_at_a_time(
                key,
                &input,
                &expected_hash,
                &expected_keyed_hash,
                &expected_derive_key,
            );
        }
    }
}
