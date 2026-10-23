//! Hashes a buffer the compiler can't see and publishes the digest, so the only
//! difference between builds is the BLAKE3 implementation picked by the
//! `impl-*` feature. Without one it copies the input (the baseline).

#![no_std]
#![no_main]

use core::hint::black_box;

const _: () = assert!(
    cfg!(feature = "impl-blakity") as u8
        + cfg!(feature = "impl-reference") as u8
        + cfg!(feature = "impl-blake3") as u8
        + cfg!(feature = "impl-c") as u8
        <= 1,
    "enable at most one impl-* feature"
);

cfg_select! {
    // `blakity::hash` is what bread calls.
    feature = "impl-blakity" => {
        fn hash(input: &[u8], out: &mut [u8; 32]) {
            *out = *blakity::hash(input).as_bytes();
        }
    }
    feature = "impl-reference" => {
        fn hash(input: &[u8], out: &mut [u8; 32]) {
            let mut hasher = upstream::reference::Hasher::new();
            hasher.update(input);
            hasher.finalize(out);
        }
    }
    // `blake3::hash` is what bread calls (impl-blake3-pure enables this too).
    feature = "impl-blake3" => {
        fn hash(input: &[u8], out: &mut [u8; 32]) {
            *out = *blake3::hash(input).as_bytes();
        }
    }
    feature = "impl-c" => {
        fn hash(input: &[u8], out: &mut [u8; 32]) {
            *out = upstream::c::hash(input);
        }
    }
    _ => {
        fn hash(input: &[u8], out: &mut [u8; 32]) {
            let n = input.len().min(32);
            out[..n].copy_from_slice(&input[..n]);
        }
    }
}

fn run(ptr: *const u8) -> usize {
    // The length is opaque too, so every size path (multi-chunk trees
    // included) stays in the binary.
    let input = unsafe { core::slice::from_raw_parts(black_box(ptr), black_box(4096)) };
    let mut out = [0u8; 32];
    hash(input, &mut out);
    black_box(&out);
    out[0] as usize
}

#[cfg(target_os = "uefi")]
#[unsafe(no_mangle)]
extern "efiapi" fn efi_main(_image: *mut u8, system_table: *const u8) -> usize {
    run(system_table)
}

#[cfg(not(target_os = "uefi"))]
#[unsafe(no_mangle)]
extern "C" fn _start() -> ! {
    run(0x10_0000 as *const u8);
    loop {}
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
