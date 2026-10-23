//! Upstream BLAKE3 (the `../../upstream` submodule) as a no_std crate. It is a
//! crate of its own so that bread's profile can give it opt-level 3, as bread
//! does its hash crate.

#![no_std]

/// `reference_impl.rs`, included as a module because its crate isn't
/// `#![no_std]`.
#[cfg(feature = "reference")]
#[path = "../../upstream/reference_impl/reference_impl.rs"]
pub mod reference;

/// FFI to the portable C implementation (built by build.rs).
#[cfg(feature = "c")]
pub mod c {
    use core::mem::MaybeUninit;

    /// Room for `blake3_hasher` (1912 bytes on 64-bit targets), 8-byte aligned.
    #[repr(C, align(8))]
    struct RawHasher([u8; 2048]);

    unsafe extern "C" {
        fn blake3_hasher_init(h: *mut RawHasher);
        fn blake3_hasher_update(h: *mut RawHasher, input: *const u8, len: usize);
        fn blake3_hasher_finalize(h: *const RawHasher, out: *mut u8, len: usize);
    }

    pub fn hash(input: &[u8]) -> [u8; 32] {
        let mut h = MaybeUninit::<RawHasher>::uninit();
        let mut out = [0u8; 32];
        unsafe {
            blake3_hasher_init(h.as_mut_ptr());
            blake3_hasher_update(h.as_mut_ptr(), input.as_ptr(), input.len());
            blake3_hasher_finalize(h.as_ptr(), out.as_mut_ptr(), out.len());
        }
        out
    }
}
