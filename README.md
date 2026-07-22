# xz4rust
Memory safe pure Rust no-std & no alloc XZ decoder

## Usage
### With standard library
```rust
use std::fs::File;
use std::io::Read;
use xz4rust::XzReader;

fn main() -> std::io::Result<()> {
    //This file contains Hello\nWorld!
    let file = File::open("../test_files/good-1-block_header-1.xz")?;
    let mut reader = XzReader::new(file);
    
    let mut result = Vec::new();
    reader.read_to_end(&mut result)?;
    
    println!("{}", String::from_utf8_lossy(&result));
    Ok(())
}
```

### `no_std` + no allocator
Note: XzDecoder itself uses about 32k memory on the stack. This example needs about 100k stack.
See below for an alternative that uses much less stack.
```rust
use xz4rust::{XzDecoder, XzNextBlockResult};

/// I am aware that the print! macro is not available in no_std, but other than that everything
/// here should work in no_std environments.
fn main() {
  //This file contains Hello\nWorld!
  let compressed_data = include_bytes!("../test_files/good-1-block_header-1.xz");
  // The size of the dictionary depends on the input file, this one has a 64kib dictionary.
  // If you use xz-utils you can set the dict size when encoding.
  // The largest preset in xz-utils (-9) uses a 65mb dictionary.
  // Using a dictionary that is too small will cause an err when decoding.
  // Note: You may have to move this off the stack to the heap or static memory if this becomes too large for your stack.
  let mut dictionary_buffer = [0u8; 65536];
  let mut decompressed_data_buffer = [0u8; 16];

  let mut decoder = XzDecoder::with_fixed_size_dict(&mut dictionary_buffer);
  let mut input_position = 0usize;
  loop {
    match decoder.decode(
      &compressed_data[input_position..],
      &mut decompressed_data_buffer,
    ) {
      Ok(XzNextBlockResult::NeedMoreData(input_consumed, output_produced)) => {
        input_position += input_consumed;
        if output_produced > 0 {
          // Note: We know this input file contains only ascii characters
          // and no multi byte which might be split at the edge of a buffer!
          print!(
            "{}",
            std::str::from_utf8(&decompressed_data_buffer[..output_produced]).unwrap()
          );
        }
      }
      Ok(XzNextBlockResult::EndOfStream(_, output_produced)) => {
        if output_produced > 0 {
          // Note: We know this input file contains only ascii characters
          // and no multi byte which might be split at the edge of a buffer!
          print!(
            "{}",
            std::str::from_utf8(&decompressed_data_buffer[..output_produced]).unwrap()
          );
        }
        println!();
        println!("Finished!");
        break;
      }
      Err(err) => panic!("Decompression failed {}", err),
    };
  }
}

```
### `no_std` + no allocator + minimal stack space
```rust
use spin::mutex::SpinMutex;
use xz4rust::{XzNextBlockResult, XzStaticDecoder};


// DICT_SIZE_PROFILE_0 is the size of the dictionary in bytes! It has a direct impact on the size of the variable.
// In this configuration this entire variable is about 300k in size, which will be placed in your binary.
// If you are willing to use unsafe code then you should also be able to place it in zeroed memory. (if the linker permits)
// In this example we use no unsafe code and just accept that the binary gets bigger.
static DECODER: SpinMutex<XzStaticDecoder<{ xz4rust::DICT_SIZE_PROFILE_0 }>> = 
  SpinMutex::new(XzStaticDecoder::new());

fn main() {
  //This file contains Hello\nWorld!
  let compressed_data = include_bytes!("../test_files/good-1-block_header-1.xz");

  let mut decompressed_data_buffer = [0u8; 16];

  let mut decoder = DECODER.lock();
  let mut input_position = 0usize;
  loop {
    match decoder.decode(
      &compressed_data[input_position..],
      &mut decompressed_data_buffer,
    ) {
      Ok(XzNextBlockResult::NeedMoreData(input_consumed, output_produced)) => {
        input_position += input_consumed;
        if output_produced > 0 {
          // Note: We know this input file contains only ascii characters
          // and no multibyte which might be split at the edge of a buffer!
          print!(
            "{}",
            std::str::from_utf8(&decompressed_data_buffer[..output_produced]).unwrap()
          );
        }
      }
      Ok(XzNextBlockResult::EndOfStream(_, output_produced)) => {
        if output_produced > 0 {
          // Note: We know this input file contains only ascii characters
          // and no multibyte which might be split at the edge of a buffer!
          print!(
            "{}",
            std::str::from_utf8(&decompressed_data_buffer[..output_produced]).unwrap()
          );
        }
        println!();
        println!("Finished!");
        break;
      }
      Err(err) => panic!("Decompression failed {}", err),
    };
  }
}
```
### `no_std` + alloc
```rust
use xz4rust::{XzDecoder, XzNextBlockResult};

fn main() {
  //This file contains Hello\nWorld!
  let compressed_data = include_bytes!("../test_files/good-1-block_header-1.xz");
  let mut decompressed_data = Vec::new();

  let initial_alloc_size = xz4rust::DICT_SIZE_MIN;
  // Note: This is 3GB, decide yourself if you want the decoder 
  // to allocate this much memory if the possibly untrustworthy input file requires it.
  let max_alloc_size = xz4rust::DICT_SIZE_MAX;
  let mut decoder = XzDecoder::in_heap_with_alloc_dict_size(initial_alloc_size, max_alloc_size);

  let mut input_position = 0usize;
  loop {
    let mut temp_buffer = [0u8; 4096];
    match decoder.decode(&compressed_data[input_position..], &mut temp_buffer) {
      Ok(XzNextBlockResult::NeedMoreData(input_consumed, output_produced)) => {
        input_position += input_consumed;
        decompressed_data.extend_from_slice(&temp_buffer[..output_produced]);
      }
      Ok(XzNextBlockResult::EndOfStream(_, output_produced)) => {
        decompressed_data.extend_from_slice(&temp_buffer[..output_produced]);
        break;
      }
      Err(err) => panic!("Decompression failed {}", err),
    };
  }

  //This obviously requires std, but is just for illustrative purposes, you can do something else with the data...
  println!(
    "Decompressed contents: {}",
    String::from_utf8_lossy(&decompressed_data)
  );
  println!("Finished!");
}
```

## Comparison to other XZ decoders available for Rust
| Crate                | Can Decode | Can Encode | Can Decode BCJ | No C-Compiler/Unsafe | no-std       | no-alloc |
|----------------------|------------|------------|----------------|----------------------|--------------|----------|
| xz4rust (this crate) | &check;    | &cross;    | &check;        | &check;              | &check;      | &check;  |
| xz2                  | &check;    | &check;    | &cross; (*1)   | &cross;              | &cross;      | &cross;  |
| xz-embedded-sys      | &check;    | &cross;    | &cross; (*1)   | &cross;              | &cross; (*1) | &cross;  |

(*1)
It would probably be trivial to patch the crate.

### Benchmarks
Speed values are based on the `b2` benchmark in the benches' directory.
This benchmark decodes a ~1mb amd64 `.so` file. IO delay is not included
as the entire 1mb xz file is loaded into memory before the benchmark begins.

The benchmarks uses rust 1.86

The "Steam Deck" column refers to a baseline Steam Deck so it should be reasonably reproducible.

The "I7 8700k" Column refers to an ordinary desktop computer with a non overclocked Intel I7 8700k CPU running debian linux.
This benchmark is probably not reproducible on a different computer.

| Crate                | Steam Deck    | Intel I7 8700k |
|----------------------|---------------|----------------|
| xz4rust (this crate) | 13.7ms (136%) | 9.6ms  (133%)  |
| xz2                  | 10.1ms (100%) | 7.2ms  (100%)  |
| xz-embedded-sys      | 11.8ms (117%) | 9.4ms  (130%)  | 


# Features
The default features assume you are using the rust standard library. 
For no_std disable the default features and enable them as needed!

- `bcj` - enables support for decoding BCJ xz files. 
  - Enabled by default
  - BCJ improves the compression of compiled executable code. This is usually present in .xz packages bundled by some linux distributions.
  - If you only need to decode .xz files that you create yourself then you probably do not need this feature unless you explicitly enable it during compression.
  - If this feature is disabled, then upon decoding of the header of a xz file with bcj the implementation will return an Err.
- `delta` - enables support for decoding xz files that use the delta filter.
  - Enabled by default
  - delta is rarely used. It can be useful in improving the compression ratio in bitmaps or tiff images.
  - If this feature is disabled, then upon decoding of the header of a xz file with the delta filter the implementation will return an Err.
- `crc64`
  - Enabled by default
  - Support for crc64 checksums in xz files
  - Note: the xz command line application will use crc64 checksums by default, disabling this feature will prevent you from decoding those
  - If this feature is disabled then upon decoding of the header of a xz file with crc64 the implementation will return an Err.
- `sha256`
  - Enabled by default
  - Adds a dependency to the `sha2` crate
  - If this feature is disabled then upon decoding of the header of a xz file with sha256 the implementation will return an Err.
- `alloc`
  - Enabled by default
  - Requires you to have an allocator present in your binary. (If you use the stdlib then you have an allocator)
  - When creating the decoder you will have to decide how the decoder allocates the dictionary. 
    - If you disable this feature then you cannot choose the option to let the decoder allocate the dictionary on the heap.
- `std`
  - Enabled by default
  - Requires the standard library
  - Adds support for decoding transparently from a std::io::Read
- `no_unsafe`
  - Not enabled by default
  - Disables all unsafe code in this crate.
  - Read below for more info.

## How was this crate implemented?
This implementation is a port of the C library xz-embedded to rust.

This implementation has the same limitations as xz-embedded (3GiB dictionary size)

A memory allocator is optional for this implementation.

The C code of xz-embedded has been translated using c2rust and
then manually refactored until no unsafe code remained and the rust code looked sane.

After porting to Rust some features present in other xz decoder implementations have been implemented on top of
the ported rust code. Such as support for filter chains and the delta filter.

## License
The rust source code in this project is released under the MIT License.
The rust source code is a port/translation of xz-embedded as permitted by the license of xz-embedded.
For more information regarding xz-embedded see here:
https://github.com/tukaani-project/xz-embedded

The MIT License does NOT apply to the test files in the test_files directory of this repository.
It contains compressed binaries that are released under different licenses (such as LGPLv3).
This makes any binary builds of the test code non trivially redistributable. 
As should be obvious, this has no effect on any non-test builds of this library.

Some of the files in the test_files directory appear to also be in the public domain. 
They are sourced from the xz-utils git repo.
If you are looking to re-use or redistribute only those test files then
I recommend sourcing them from the xz-utils git repo directly.

## Tests
This implementation can decode all test files from the xz-repo.
In addition to those test files various tests on random data as well as some 
"real" files are also conducted. 

Every code branch that is not hit by a test is marked with a "TODO unreached" comment in the code.
There are not many, and they are in rather obscure paths of the code.
Examples of such unreached code paths:
* Non trivially reachable error handling when block/file headers contain invalid data
* Parts of the riscv bcj filter

If you have a test file (and or in/out buffer sizes) that causes those to be hit then sharing those files via GitHub issue 
would be very much appreciated.

## Unsafe code
This crate features two optional unsafe blocks. Both are only related to allocation of the memory for the
decoder. Once the decoder is allocated, no unsafe code is needed to perform the actual decoding.

One in the `alloc` feature to
allocate a 32kb large struct in the heap using `Box::new_uninit`.
This unsafe block is trivially verifiable and has been tested with miri.

One function to allocate the decoder at an arbitrary address.
This can be useful if you want to place the decoder in a union.
You can just ignore this function if you don't need it.
This unsafe block is trivially verifiable and has been tested with miri.

#### Why?
This is unfortunately needed because rust has no other guaranteed way to allocate a structure on the heap.
You can move a struct to the heap, but you cant reliably allocate it there without unsafe.
On small stack sizes stack allocation of 32k (like with libc-musl which has only 90k stack) may blow the stack.
There is a test that ensures that .xz files can be decoded with as little as 8k of stack. Depending
on the optimization level and cpu architecture less than 4k of stack are also sufficient.

#### Disabling unsafe code

If your target system has enough stack (at least ~40k) you can disable all unsafe code in this crate
(enforced by deny(unsafe_code)) if you enable the `no_unsafe` feature without having to worry about any side effects.
I do not recommend enabling the `no_unsafe` feature on libc-musl targets 
due to stack overflows that might occur then.

## Is this library panic free?
Probably not. There are a lot of operations that could potentially panic given a malicious input file.
Until the entire codebase is fully fuzzed I cannot rule out that this implementation panics. Since this
implementation has effectively no unsafe code as mentioned above a panic is the worst that could happen tho.
There is also uncertainty about some places regarding unsigned integer overflows. 
In debug mode these panic, in release mode they just act like they do in C. 
If you have any test files that cause panics or overflows (and therefore panics in debug mode) 
then those would be greatly appreciated.

## xzcheck program
I use this library to decode patches/releases for a different application as part an installer application. 
To reduce risk and ensure that patches can be decoded without issue by the installer, 
I have created a small program called xzcheck which verifies that a .xz file is valid 
and can be decoded by this library. I run xzcheck as part of my build process for my patches.
If your use-case allows for this then I recommend you to do the same if you are considering this library.

You can install it normally via `cargo install xzcheck`
It fully decodes the entire .xz file and checks the content hash. 
If it succeeds in decoding it exits with code 0 otherwise it exits with code 255 and
prints an error to stderr. This is useful if you wish to automatically test created 
xz files to see if your application will later be able to decode them.

## Unsupported targets
Any rust target where usize/pointer size is 16 bit is not supported.
Currently, rust only has one tier-3 target for a microcontroller where this is the case.
This crate will emit a compiler error for such targets.

## Future work
* Finish refactoring existing code.
* Optimize the current implementation using perf.
* Port/Implement an XZ Encoder. (A lot of work that I currently do not need myself...)