use xz4rust::{XzDecoder, XzError, XzNextBlockResult, XzStaticDecoder, DICT_SIZE_PROFILE_0};

const OUTPUT_LEN: usize = 16_384;
const FRAGMENTS: [&[u8]; 6] = [
    include_bytes!("../test_files/lzma2-chunk-boundary/fragment-0.bin"),
    include_bytes!("../test_files/lzma2-chunk-boundary/fragment-1.bin"),
    include_bytes!("../test_files/lzma2-chunk-boundary/fragment-2.bin"),
    include_bytes!("../test_files/lzma2-chunk-boundary/fragment-3.bin"),
    include_bytes!("../test_files/lzma2-chunk-boundary/fragment-4.bin"),
    include_bytes!("../test_files/lzma2-chunk-boundary/fragment-5.bin"),
];
const EXPECTED: &[u8; OUTPUT_LEN * 6] =
    include_bytes!("../test_files/lzma2-chunk-boundary/expected-output.bin");

fn assert_need_more(result: &XzNextBlockResult) {
    assert!(matches!(result, XzNextBlockResult::NeedMoreData(_, _)));
}

fn seed(decoder: &mut XzDecoder<'_>) {
    for window in 0..2 {
        let mut output = [0; OUTPUT_LEN];
        let result = decoder.decode(FRAGMENTS[window], &mut output).unwrap();
        assert_need_more(&result);
        assert_eq!(result.input_consumed(), FRAGMENTS[window].len());
        assert_eq!(result.output_produced(), OUTPUT_LEN);
        assert!(decoder.is_lzma2_chunk_boundary());
        assert_eq!(
            output,
            EXPECTED[window * OUTPUT_LEN..(window + 1) * OUTPUT_LEN]
        );
    }
}

#[test]
fn boundary_state_is_conservative() {
    let mut dictionary = [0; DICT_SIZE_PROFILE_0];
    let mut decoder = XzDecoder::with_fixed_size_dict(&mut dictionary);
    let mut output = [0; OUTPUT_LEN];

    assert!(!decoder.is_lzma2_chunk_boundary());
    decoder.reset();
    assert!(!decoder.is_lzma2_chunk_boundary());

    let result = decoder.decode(&FRAGMENTS[0][..6], &mut output).unwrap();
    assert_need_more(&result);
    assert!(!decoder.is_lzma2_chunk_boundary());

    decoder.reset();
    let result = decoder.decode(&FRAGMENTS[0][..24], &mut output).unwrap();
    assert_need_more(&result);
    assert_eq!(result.input_consumed(), 24);
    assert_eq!(result.output_produced(), 0);
    assert!(!decoder.is_lzma2_chunk_boundary());

    decoder.reset();
    let result = decoder.decode(&FRAGMENTS[0][..25], &mut output).unwrap();
    assert_need_more(&result);
    assert_eq!(result.input_consumed(), 25);
    assert_eq!(result.output_produced(), 0);
    assert!(!decoder.is_lzma2_chunk_boundary());

    decoder.reset();
    let result = decoder.decode(FRAGMENTS[0], &mut output).unwrap();
    assert_need_more(&result);
    assert_eq!(result.input_consumed(), FRAGMENTS[0].len());
    assert_eq!(result.output_produced(), OUTPUT_LEN);
    assert!(decoder.is_lzma2_chunk_boundary());

    let error = decoder.decode(&[0x03], &mut output[..1]).unwrap_err();
    assert_eq!(error, XzError::CorruptedDataInLzma);
    assert!(!decoder.is_lzma2_chunk_boundary());

    decoder.reset();
    assert!(!decoder.is_lzma2_chunk_boundary());
    let stream = include_bytes!("../test_files/good-1-check-none.xz");
    let result = decoder.decode(stream, &mut output).unwrap();
    assert!(result.is_end_of_stream());
    assert_eq!(result.input_consumed(), stream.len());
    assert!(result.output_produced() > 0);
    assert!(!decoder.is_lzma2_chunk_boundary());
}

#[cfg(feature = "bcj")]
#[test]
fn eos_with_bcj_output_pending_is_not_a_boundary() {
    let stream = include_bytes!("../test_files/good-1-empty-bcj-lzma2.xz");
    let header = &stream[..24];
    let body = [0x01, 0x00, 0x04, 0xe8, 0, 0, 0, 0, 0];
    let mut dictionary = [0; DICT_SIZE_PROFILE_0];
    let mut decoder = XzDecoder::with_fixed_size_dict(&mut dictionary);
    let mut output = [0; 1];

    let result = decoder.decode(header, &mut output).unwrap();
    assert_need_more(&result);
    assert_eq!(result.input_consumed(), header.len());
    assert_eq!(result.output_produced(), 0);
    assert!(!decoder.is_lzma2_chunk_boundary());

    let mut input_pos = 0;
    let mut output_produced = 0;
    while input_pos < body.len() {
        let result = decoder.decode(&body[input_pos..], &mut output).unwrap();
        assert_need_more(&result);
        assert!(result.made_progress());
        input_pos += result.input_consumed();
        output_produced += result.output_produced();
    }

    assert_eq!(input_pos, body.len());
    assert!(output_produced < 5);
    assert!(!decoder.is_lzma2_chunk_boundary());
}

#[test]
fn static_decoder_delegates_boundary_query() {
    static DECODER: std::sync::Mutex<XzStaticDecoder<DICT_SIZE_PROFILE_0>> =
        std::sync::Mutex::new(XzStaticDecoder::new());

    let mut decoder = DECODER.lock().unwrap();
    decoder.reset();
    let mut output = [0; OUTPUT_LEN];
    let result = decoder.decode(FRAGMENTS[0], &mut output).unwrap();
    assert_need_more(&result);
    assert_eq!(result.input_consumed(), FRAGMENTS[0].len());
    assert_eq!(result.output_produced(), OUTPUT_LEN);
    assert!(decoder.is_lzma2_chunk_boundary());
    assert_eq!(output, EXPECTED[..OUTPUT_LEN]);
}

#[test]
fn persistent_fragments_decode_at_boundaries() {
    let mut dictionary = [0; DICT_SIZE_PROFILE_0];
    let mut decoder = XzDecoder::with_fixed_size_dict(&mut dictionary);

    for window in 0..FRAGMENTS.len() {
        let mut output = [0; OUTPUT_LEN];
        let result = decoder.decode(FRAGMENTS[window], &mut output).unwrap();
        assert_need_more(&result);
        assert_eq!(result.input_consumed(), FRAGMENTS[window].len());
        assert_eq!(result.output_produced(), OUTPUT_LEN);
        assert!(decoder.is_lzma2_chunk_boundary());
        assert_eq!(
            output,
            EXPECTED[window * OUTPUT_LEN..(window + 1) * OUTPUT_LEN]
        );
    }
}

fn decode_chunked(input_chunk: usize, output_chunk: usize) {
    let mut dictionary = [0; DICT_SIZE_PROFILE_0];
    let mut decoder = XzDecoder::with_fixed_size_dict(&mut dictionary);

    for window in 0..FRAGMENTS.len() {
        let fragment = FRAGMENTS[window];
        let mut output = [0; OUTPUT_LEN];
        let mut input_pos = 0;
        let mut output_pos = 0;

        while input_pos < fragment.len() || output_pos < OUTPUT_LEN {
            let input_end = (input_pos + input_chunk).min(fragment.len());
            let output_end = (output_pos + output_chunk).min(OUTPUT_LEN);
            let result = decoder
                .decode(
                    &fragment[input_pos..input_end],
                    &mut output[output_pos..output_end],
                )
                .unwrap();
            assert_need_more(&result);
            assert!(result.made_progress());
            input_pos += result.input_consumed();
            output_pos += result.output_produced();
        }

        assert_eq!(input_pos, fragment.len());
        assert_eq!(output_pos, OUTPUT_LEN);
        assert!(decoder.is_lzma2_chunk_boundary());
        assert_eq!(
            output,
            EXPECTED[window * OUTPUT_LEN..(window + 1) * OUTPUT_LEN]
        );
    }
}

#[test]
fn persistent_fragments_decode_with_chunked_buffers() {
    for (input_chunk, output_chunk) in [(256, OUTPUT_LEN), (257, 1024), (1024, 257)] {
        decode_chunked(input_chunk, output_chunk);
    }
}

#[test]
fn later_fragment_needs_persistent_state() {
    let mut dictionary = [0; DICT_SIZE_PROFILE_0];
    let mut decoder = XzDecoder::with_fixed_size_dict(&mut dictionary);
    let mut output = [0; OUTPUT_LEN];

    let error = decoder.decode(FRAGMENTS[2], &mut output).unwrap_err();
    assert_eq!(error, XzError::StreamHeaderMagicNumberMismatch);
    assert!(!decoder.is_lzma2_chunk_boundary());
}

#[test]
fn truncated_fragment_is_not_a_boundary() {
    let mut dictionary = [0; DICT_SIZE_PROFILE_0];
    let mut decoder = XzDecoder::with_fixed_size_dict(&mut dictionary);
    seed(&mut decoder);
    let mut output = [0; OUTPUT_LEN];
    let fragment = &FRAGMENTS[2][..FRAGMENTS[2].len() - 1];

    let result = decoder.decode(fragment, &mut output).unwrap();
    assert_need_more(&result);
    assert_eq!(result.input_consumed(), fragment.len());
    assert!(result.output_produced() < OUTPUT_LEN);
    assert!(!decoder.is_lzma2_chunk_boundary());
}

#[test]
fn appended_byte_is_counted_and_not_a_boundary() {
    let mut dictionary = [0; DICT_SIZE_PROFILE_0];
    let mut decoder = XzDecoder::with_fixed_size_dict(&mut dictionary);
    seed(&mut decoder);
    let mut input = [0; 2744];
    input[..FRAGMENTS[2].len()].copy_from_slice(FRAGMENTS[2]);
    let mut output = [0; OUTPUT_LEN];

    let result = decoder.decode(&input, &mut output).unwrap();
    assert_need_more(&result);
    assert_eq!(result.input_consumed(), input.len());
    assert_eq!(result.output_produced(), OUTPUT_LEN);
    assert!(!decoder.is_lzma2_chunk_boundary());
}

#[test]
fn mutated_fragment_errors_and_is_not_a_boundary() {
    let mut dictionary = [0; DICT_SIZE_PROFILE_0];
    let mut decoder = XzDecoder::with_fixed_size_dict(&mut dictionary);
    seed(&mut decoder);
    let mut input = *include_bytes!("../test_files/lzma2-chunk-boundary/fragment-2.bin");
    let mutation = input.len() / 2;
    input[mutation] ^= 0x01;
    let mut output = [0; OUTPUT_LEN];

    decoder.decode(&input, &mut output).unwrap_err();
    assert!(!decoder.is_lzma2_chunk_boundary());
}

#[test]
fn short_output_capacity_is_not_a_boundary() {
    let mut dictionary = [0; DICT_SIZE_PROFILE_0];
    let mut decoder = XzDecoder::with_fixed_size_dict(&mut dictionary);
    seed(&mut decoder);
    let mut output = [0; OUTPUT_LEN - 1];

    let result = decoder.decode(FRAGMENTS[2], &mut output).unwrap();
    assert_need_more(&result);
    assert_eq!(result.input_consumed(), FRAGMENTS[2].len());
    assert_eq!(result.output_produced(), output.len());
    assert!(!decoder.is_lzma2_chunk_boundary());
}

#[test]
fn large_output_capacity_still_reports_exact_output() {
    let mut dictionary = [0; DICT_SIZE_PROFILE_0];
    let mut decoder = XzDecoder::with_fixed_size_dict(&mut dictionary);
    seed(&mut decoder);
    let mut output = [0xa5; OUTPUT_LEN + 1];

    let result = decoder.decode(FRAGMENTS[2], &mut output).unwrap();
    assert_need_more(&result);
    assert_eq!(result.input_consumed(), FRAGMENTS[2].len());
    assert_eq!(result.output_produced(), OUTPUT_LEN);
    assert!(decoder.is_lzma2_chunk_boundary());
    assert_eq!(output[OUTPUT_LEN], 0xa5);
    assert_eq!(
        output[..OUTPUT_LEN],
        EXPECTED[2 * OUTPUT_LEN..3 * OUTPUT_LEN]
    );
}
