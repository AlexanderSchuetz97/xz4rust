use std::io::Read;
use std::num::NonZeroUsize;
use xz4rust::{XzDecoder, XzError, XzReader};

#[test]
pub fn dict2big() {
    let input = include_bytes!("../test_files/java_native_utils_riscv64.so.xz");

    let mut r = XzReader::new_with_buffer_size_and_decoder(
        input.as_slice(),
        NonZeroUsize::new(4096).unwrap(),
        XzDecoder::in_heap_with_alloc_dict_size(4096, 4096),
    );
    let err = r.read_to_end(&mut Vec::new()).unwrap_err();
    let n: XzError = err.downcast().unwrap();

    assert_eq!(XzError::DictionaryTooLarge(8388608), n);
}

#[cfg(target_pointer_width = "64")] //This test needs 3gb heap. Won't work 'reliably' on 32 bit.
#[test]
pub fn dict_trunc() {
    let input = include_bytes!("../test_files/java_native_utils_riscv64.so.xz");
    let expect = include_bytes!("../test_files/java_native_utils_riscv64.so");

    let mut r = XzReader::new_with_buffer_size_and_decoder(
        input.as_slice(),
        NonZeroUsize::new(4096).unwrap(),
        XzDecoder::in_heap_with_alloc_dict(vec![0u8; xz4rust::DICT_SIZE_MAX + 1], 4096),
    );
    let mut m = Vec::new();
    _ = r.read_to_end(&mut m);
    assert_eq!(m.as_slice(), expect.as_slice());
}
