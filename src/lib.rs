#![no_std]
#![deny(missing_docs)]

//! **blakity** -- a tiny, portable BLAKE3.
//!
//! A `#![no_std]` [BLAKE3](https://github.com/BLAKE3-team/BLAKE3) in a single
//! file with no dependencies, derived from upstream's `reference_impl`. It
//! targets small binaries (UEFI and other `no_std` builds) while staying faster
//! than the reference implementation, without SIMD intrinsics or
//! platform-specific code.
//!
//! The API is the same as upstream's `reference_impl`: a [`Hasher`] for the
//! regular hash, the keyed hash and the key derivation function, whose
//! [`finalize`](Hasher::finalize) fills an output slice of any length (the
//! extendable output).
//!
//! # Example
//!
//! ```
//! let mut hasher = blakity::Hasher::new();
//! hasher.update(b"abc");
//! hasher.update(b"def");
//! let mut hash = [0; 32];
//! hasher.finalize(&mut hash);
//! let mut extended_hash = [0; 500];
//! hasher.finalize(&mut extended_hash);
//! assert_eq!(hash, extended_hash[..32]);
//! ```
//!
//! # Cargo features
//!
//! No features are currently defined.

use core::cmp::min;
use core::mem::MaybeUninit;

const OUT_LEN: usize = 32;
const KEY_LEN: usize = 32;
const BLOCK_LEN: usize = 64;
const CHUNK_LEN: usize = 1024;

const CHUNK_START: u32 = 1 << 0;
const CHUNK_END: u32 = 1 << 1;
const PARENT: u32 = 1 << 2;
const ROOT: u32 = 1 << 3;
const KEYED_HASH: u32 = 1 << 4;
const DERIVE_KEY_CONTEXT: u32 = 1 << 5;
const DERIVE_KEY_MATERIAL: u32 = 1 << 6;

const IV: [u32; 8] = [
    0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A, 0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19,
];

const MSG_PERMUTATION: [usize; 16] = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8];

// The mixing function, G, which mixes either a column or a diagonal.
#[inline(always)]
fn g(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize, mx: u32, my: u32) {
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(mx);
    state[d] = (state[d] ^ state[a]).rotate_right(16);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(12);
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(my);
    state[d] = (state[d] ^ state[a]).rotate_right(8);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(7);
}

fn first_8_words(compression_output: [u32; 16]) -> [u32; 8] {
    compression_output[0..8].try_into().unwrap()
}

fn words_from_little_endian_bytes(bytes: &[u8], words: &mut [u32]) {
    debug_assert_eq!(bytes.len(), 4 * words.len());
    for (four_bytes, word) in bytes.chunks_exact(4).zip(words) {
        *word = u32::from_le_bytes(four_bytes.try_into().unwrap());
    }
}

// Each chunk or parent node can produce either an 8-word chaining value or, by
// setting the ROOT flag, any number of final output bytes. The Output struct
// captures the state just prior to choosing between those two possibilities.
// It is also the input of the compression function.
#[derive(Clone, Copy)]
struct Output {
    input_chaining_value: [u32; 8],
    block_words: [u32; 16],
    counter: u64,
    block_len: u32,
    flags: u32,
}

impl Output {
    // The compression function. It stays out of line and takes its whole
    // input through one pointer, so every call site is a short call.
    #[inline(never)]
    fn compress(&self) -> [u32; 16] {
        let cv = &self.input_chaining_value;
        #[rustfmt::skip]
        let mut state = [
            cv[0], cv[1], cv[2], cv[3], cv[4], cv[5], cv[6], cv[7],
            IV[0], IV[1], IV[2], IV[3],
            self.counter as u32, (self.counter >> 32) as u32, self.block_len, self.flags,
        ];
        let mut m = self.block_words;
        for round in 0..7 {
            // Mix the columns.
            g(&mut state, 0, 4, 8, 12, m[0], m[1]);
            g(&mut state, 1, 5, 9, 13, m[2], m[3]);
            g(&mut state, 2, 6, 10, 14, m[4], m[5]);
            g(&mut state, 3, 7, 11, 15, m[6], m[7]);
            // Mix the diagonals.
            g(&mut state, 0, 5, 10, 15, m[8], m[9]);
            g(&mut state, 1, 6, 11, 12, m[10], m[11]);
            g(&mut state, 2, 7, 8, 13, m[12], m[13]);
            g(&mut state, 3, 4, 9, 14, m[14], m[15]);
            // Permute the message words for the next round.
            if round < 6 {
                m = MSG_PERMUTATION.map(|i| m[i]);
            }
        }

        for i in 0..8 {
            state[i] ^= state[i + 8];
            state[i + 8] ^= cv[i];
        }
        state
    }

    fn chaining_value(&self) -> [u32; 8] {
        first_8_words(self.compress())
    }

    fn root_output_bytes(&self, out_slice: &mut [u8]) {
        let mut output = *self;
        output.counter = 0;
        output.flags |= ROOT;
        for out_block in out_slice.chunks_mut(2 * OUT_LEN) {
            let words = output.compress();
            // The output length might not be a multiple of 4.
            for (word, out_word) in words.iter().zip(out_block.chunks_mut(4)) {
                out_word.copy_from_slice(&word.to_le_bytes()[..out_word.len()]);
            }
            output.counter += 1;
        }
    }
}

struct ChunkState {
    chaining_value: [u32; 8],
    chunk_counter: u64,
    block: [u8; BLOCK_LEN],
    block_len: u8,
    blocks_compressed: u8,
    flags: u32,
}

impl ChunkState {
    fn new(key_words: [u32; 8], chunk_counter: u64, flags: u32) -> Self {
        Self {
            chaining_value: key_words,
            chunk_counter,
            block: [0; BLOCK_LEN],
            block_len: 0,
            blocks_compressed: 0,
            flags,
        }
    }

    fn len(&self) -> usize {
        BLOCK_LEN * self.blocks_compressed as usize + self.block_len as usize
    }

    fn start_flag(&self) -> u32 {
        if self.blocks_compressed == 0 {
            CHUNK_START
        } else {
            0
        }
    }

    fn update(&mut self, mut input: &[u8]) {
        // The caller never passes more than the rest of the chunk, so its last
        // block always stays in the buffer for `output` to compress as
        // CHUNK_END.
        debug_assert!(input.len() <= CHUNK_LEN - self.len());
        while !input.is_empty() {
            let block: &[u8; BLOCK_LEN];
            if self.block_len == 0 && input.len() > BLOCK_LEN {
                // A whole block with more input after it skips the buffer.
                (block, input) = input.split_first_chunk().unwrap();
            } else if self.block_len as usize >= BLOCK_LEN {
                // The buffer is full and more input is coming, so this
                // compression is not CHUNK_END. (`>=` proves `block_len <
                // BLOCK_LEN` below, which removes the copy's panic path.)
                block = &self.block;
            } else {
                // Copy input bytes into the block buffer.
                let want = BLOCK_LEN - self.block_len as usize;
                let take = min(want, input.len());
                self.block[self.block_len as usize..][..take].copy_from_slice(&input[..take]);
                self.block_len += take as u8;
                input = &input[take..];
                continue;
            }
            self.chaining_value = self.block_output(block, BLOCK_LEN, 0).chaining_value();
            self.blocks_compressed += 1;
            self.block = [0; BLOCK_LEN];
            self.block_len = 0;
        }
    }

    // `block` as a compression input of `block_len` bytes (the rest of it is
    // zero), plus `end_flag`.
    fn block_output(&self, block: &[u8; BLOCK_LEN], block_len: usize, end_flag: u32) -> Output {
        let mut block_words = [0; 16];
        words_from_little_endian_bytes(block, &mut block_words);
        Output {
            input_chaining_value: self.chaining_value,
            block_words,
            counter: self.chunk_counter,
            block_len: block_len as u32,
            flags: self.flags | self.start_flag() | end_flag,
        }
    }

    fn output(&self) -> Output {
        self.block_output(&self.block, self.block_len as usize, CHUNK_END)
    }
}

fn parent_output(
    left_child_cv: [u32; 8],
    right_child_cv: [u32; 8],
    key_words: [u32; 8],
    flags: u32,
) -> Output {
    let mut block_words = [0; 16];
    block_words[..8].copy_from_slice(&left_child_cv);
    block_words[8..].copy_from_slice(&right_child_cv);
    Output {
        input_chaining_value: key_words,
        block_words,
        counter: 0,                  // Always 0 for parent nodes.
        block_len: BLOCK_LEN as u32, // Always BLOCK_LEN (64) for parent nodes.
        flags: PARENT | flags,
    }
}

fn parent_cv(
    left_child_cv: [u32; 8],
    right_child_cv: [u32; 8],
    key_words: [u32; 8],
    flags: u32,
) -> [u32; 8] {
    parent_output(left_child_cv, right_child_cv, key_words, flags).chaining_value()
}

/// An incremental hasher that can accept any number of writes.
pub struct Hasher {
    chunk_state: ChunkState,
    key_words: [u32; 8],
    cv_stack: [MaybeUninit<[u32; 8]>; 54], // Space for 54 subtree chaining values:
    cv_stack_len: u8,                      // 2^54 * CHUNK_LEN = 2^64
    flags: u32,
}

impl Hasher {
    fn new_internal(key_words: [u32; 8], flags: u32) -> Self {
        Self {
            chunk_state: ChunkState::new(key_words, 0, flags),
            key_words,
            cv_stack: [MaybeUninit::uninit(); 54],
            cv_stack_len: 0,
            flags,
        }
    }

    /// Construct a new `Hasher` for the regular hash function.
    pub fn new() -> Self {
        Self::new_internal(IV, 0)
    }

    /// Construct a new `Hasher` for the keyed hash function.
    pub fn new_keyed(key: &[u8; KEY_LEN]) -> Self {
        let mut key_words = [0; 8];
        words_from_little_endian_bytes(key, &mut key_words);
        Self::new_internal(key_words, KEYED_HASH)
    }

    /// Construct a new `Hasher` for the key derivation function. The context
    /// string should be hardcoded, globally unique, and application-specific.
    pub fn new_derive_key(context: &str) -> Self {
        let mut context_hasher = Self::new_internal(IV, DERIVE_KEY_CONTEXT);
        context_hasher.update(context.as_bytes());
        let mut context_key = [0; KEY_LEN];
        context_hasher.finalize(&mut context_key);
        let mut context_key_words = [0; 8];
        words_from_little_endian_bytes(&context_key, &mut context_key_words);
        Self::new_internal(context_key_words, DERIVE_KEY_MATERIAL)
    }

    // Section 5.1.2 of the BLAKE3 spec explains this algorithm in more detail.
    fn add_chunk_chaining_value(&mut self, mut new_cv: [u32; 8], mut total_chunks: u64) {
        // This chunk might complete some subtrees. For each completed subtree,
        // its left child will be the current top entry in the CV stack, and
        // its right child will be the current value of `new_cv`. Pop each left
        // child off the stack, merge it with `new_cv`, and overwrite `new_cv`
        // with the result. After all these merges, push the final value of
        // `new_cv` onto the stack. The number of completed subtrees is given
        // by the number of trailing 0-bits in the new total number of chunks.
        //
        // The stack is only reached through `get`/`get_mut`, so there is no
        // panic path. Neither `None` happens below the spec's 2^64-byte input
        // limit; past it the hash is undefined anyway, and a full stack just
        // drops the new entry.
        debug_assert!(total_chunks.trailing_zeros() <= self.cv_stack_len as u32);
        let mut len = self.cv_stack_len as usize;
        while total_chunks & 1 == 0 {
            len = len.wrapping_sub(1);
            let Some(left_cv) = self.cv_stack.get(len) else {
                break;
            };
            // SAFETY: entries below `cv_stack_len` are initialized.
            let left_cv = unsafe { left_cv.assume_init() };
            new_cv = parent_cv(left_cv, new_cv, self.key_words, self.flags);
            total_chunks >>= 1;
        }
        if let Some(top) = self.cv_stack.get_mut(len) {
            top.write(new_cv);
            self.cv_stack_len = len as u8 + 1;
        }
    }

    /// Add input to the hash state. This can be called any number of times.
    pub fn update(&mut self, mut input: &[u8]) {
        while !input.is_empty() {
            // If the current chunk is complete, finalize it and reset the
            // chunk state. More input is coming, so this chunk is not ROOT.
            if self.chunk_state.len() == CHUNK_LEN {
                let chunk_cv = self.chunk_state.output().chaining_value();
                let total_chunks = self.chunk_state.chunk_counter + 1;
                self.add_chunk_chaining_value(chunk_cv, total_chunks);
                self.chunk_state = ChunkState::new(self.key_words, total_chunks, self.flags);
            }

            // Compress input bytes into the current chunk state.
            let want = CHUNK_LEN - self.chunk_state.len();
            let take = min(want, input.len());
            self.chunk_state.update(&input[..take]);
            input = &input[take..];
        }
    }

    /// Finalize the hash and write any number of output bytes.
    pub fn finalize(&self, out_slice: &mut [u8]) {
        // Starting with the Output from the current chunk, compute all the
        // parent chaining values along the right edge of the tree, until we
        // have the root Output.
        let mut output = self.chunk_state.output();
        for left_cv in self.cv_stack.iter().take(self.cv_stack_len as usize).rev() {
            // SAFETY: entries below `cv_stack_len` are initialized.
            let left_cv = unsafe { left_cv.assume_init() };
            output = parent_output(left_cv, output.chaining_value(), self.key_words, self.flags);
        }
        output.root_output_bytes(out_slice);
    }
}
