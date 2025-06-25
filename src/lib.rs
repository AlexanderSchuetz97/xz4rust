//! # xz4rust
//! Memory safe pure Rust no-std & no alloc XZ decoder
#![no_std]
#![cfg_attr(feature = "no_unsafe", deny(unsafe_code))]
#![deny(
    clippy::correctness,
    clippy::perf,
    clippy::complexity,
    clippy::style,
    clippy::nursery,
    clippy::pedantic,
    clippy::clone_on_ref_ptr,
    clippy::decimal_literal_representation,
    clippy::float_cmp_const,
    clippy::missing_docs_in_private_items,
    clippy::multiple_inherent_impl,
    clippy::unwrap_used,
    clippy::cargo_common_metadata,
    clippy::used_underscore_binding
)]

#[cfg(target_pointer_width = "16")]
compile_error!("This crate does not work with 16 bit targets");

#[cfg(feature = "alloc")]
extern crate alloc;

/// bcj filtering
#[cfg(feature = "bcj")]
mod bcj;

/// Crc32 validation
mod crc32;

/// Crc64 validation
#[cfg(feature = "crc64")]
mod crc64xz;

/// LZMA and XZ stream decoder
mod decoder;

/// SHA256 validation. Mostly wraps the sha2 crate.
#[cfg(feature = "sha256")]
mod sha256;

/// Features for the Rust Standard Library. (`io::Read` support)
#[cfg(feature = "std")]
mod stl;

/// utility for clamping integers.
mod clamp;

/// variable length integer decoding.
mod vli;

/// xz delta filter decoder
#[cfg(feature = "delta")]
mod delta;
mod encoder;

// These are all types that are needed to use this crate to decode some xz files.
#[cfg(feature = "std")]
pub use stl::XzReader;
pub use {
    decoder::XzCheckType, decoder::XzDecoder, decoder::XzError, decoder::XzNextBlockResult,
    decoder::XzStaticDecoder,
};

/// Minimum possible dictionary size.
pub const DICT_SIZE_MIN: usize = 4096;

/// Maximum possible dictionary size.
pub const DICT_SIZE_MAX: usize = 3_221_225_472; //3Gib

/// Dictionary size of files created with "xz -0 <filename>"
pub const DICT_SIZE_PROFILE_0: usize = 256 * 1024;

/// Dictionary size of files created with "xz -1 <filename>"
pub const DICT_SIZE_PROFILE_1: usize = 1024 * 1024;

/// Dictionary size of files created with "xz -2 <filename>"
pub const DICT_SIZE_PROFILE_2: usize = 2 * 1024 * 1024;

/// Dictionary size of files created with "xz -3 <filename>"
pub const DICT_SIZE_PROFILE_3: usize = 4 * 1024 * 1024;

/// Dictionary size of files created with "xz -4 <filename>"
pub const DICT_SIZE_PROFILE_4: usize = 4 * 1024 * 1024;

/// Dictionary size of files created with "xz -5 <filename>"
pub const DICT_SIZE_PROFILE_5: usize = 8 * 1024 * 1024;

/// Dictionary size of files created with "xz -6 <filename>"
pub const DICT_SIZE_PROFILE_6: usize = 8 * 1024 * 1024;

/// Dictionary size of files created with "xz -7 <filename>"
pub const DICT_SIZE_PROFILE_7: usize = 16 * 1024 * 1024;

/// Dictionary size of files created with "xz -8 <filename>"
pub const DICT_SIZE_PROFILE_8: usize = 32 * 1024 * 1024;

/// Dictionary size of files created with "xz -9 <filename>"
pub const DICT_SIZE_PROFILE_9: usize = 64 * 1024 * 1024;
