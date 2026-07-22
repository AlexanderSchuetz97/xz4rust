use xz4rust::{XzCheckType, XzDecoder, XzError, XzNextBlockResult};

fn run_test2(dd: &[u8], expected: &[u8]) {
    let mut decoder = XzDecoder::default();
    let mut out_vec: Vec<u8> = Vec::new();
    let mut buf = [0u8; 4096];
    let mut d2 = dd;
    loop {
        let result = decoder.decode(d2, buf.as_mut_slice()).unwrap();
        d2 = &d2[result.input_consumed()..];
        out_vec.extend_from_slice(&buf[..result.output_produced()]);
        if result.is_end_of_stream() {
            assert_eq!(out_vec.as_slice(), expected);
            break;
        }
    }

    return;
}
fn run_test(dd: &[u8], expected: &[u8]) {
    let mut decoder = XzDecoder::default();
    let mut out_vec: Vec<u8> = Vec::new();

    //if dd.len() > 10000 {
    let mut buf = [0u8; 4096];
    let mut d2 = dd;
    loop {
        let result = decoder.decode(d2, buf.as_mut_slice()).unwrap();
        d2 = &d2[result.input_consumed()..];
        out_vec.extend_from_slice(&buf[..result.output_produced()]);
        if result.is_end_of_stream() {
            assert_eq!(out_vec.as_slice(), expected);
            break;
        }
    }

    //    return;
    //}

    for y in 1..8192 {
        println!("{y}");
        let mut data = dd;
        out_vec.truncate(0);
        decoder.reset();
        let mut buf = vec![0u8; 16.max(y)];
        let cur_out = y;
        'outer: loop {
            let mut cur_in = 1usize;
            loop {
                //println!("{cur_in} {cur_out}");
                let result = decoder.decode(&data[..cur_in], &mut buf.as_mut_slice()[..cur_out]);
                if result.is_err() {
                    panic!("1= {y} {cur_in} {cur_out} {}", result.unwrap_err());
                }
                let result = result.unwrap();

                data = &data[result.input_consumed()..];
                out_vec.extend_from_slice(&buf[..result.output_produced()]);
                if result.made_progress() {
                    cur_in = 1;
                }

                match &result {
                    XzNextBlockResult::NeedMoreData(_, _) => {
                        if !result.made_progress() {
                            cur_in += 1;
                        }
                    }
                    XzNextBlockResult::EndOfStream(_, _) => {
                        assert_eq!(out_vec.as_slice(), expected);
                        break 'outer;
                    }
                }
            }
        }
    }

    for x in 1..dd.len().min(8192) {
        for y in 1..8192 {
            if y % 2000 == 1 {
                println!("{y}/{x}");
            }
            let mut data = dd;
            out_vec.truncate(0);
            decoder.reset();
            let mut buf = vec![0u8; y];
            loop {
                let rem = x.min(data.len());
                let result = decoder.decode(&data[..rem], buf.as_mut_slice());
                if result.is_err() {
                    panic!("2= {x} {y} {}", result.unwrap_err());
                }
                let result = result.unwrap();
                data = &data[result.input_consumed()..];
                out_vec.extend_from_slice(&buf[..result.output_produced()]);
                if result.is_end_of_stream() {
                    assert_eq!(out_vec.as_slice(), expected);
                    break;
                }
            }
        }
    }
}

fn run_test_expect_error<T: Fn(XzError)>(mut data: &[u8], checker: T) {
    let mut decoder = XzDecoder::default();
    let mut out_vec: Vec<u8> = Vec::new();

    let out_size = 4096;
    let mut out_buf = Vec::with_capacity(out_size);
    out_buf.resize(out_size, 0);
    let mut buf = [0u8; 4096];
    loop {
        match decoder.decode(data, buf.as_mut_slice()) {
            Ok(result) => {
                data = &data[result.input_consumed()..];
                out_vec.extend_from_slice(&buf[..result.output_produced()]);
                if result.is_end_of_stream() {
                    panic!("EOF")
                }
            }
            Err(err) => {
                assert_ne!("", format!("{}", &err).as_str());
                assert_ne!("", format!("{:?}", &err).as_str());
                checker(err);
                return;
            }
        }
    }
}

#[test]
fn t1() {
    run_test(
        include_bytes!("../test_files/good-0cat-empty.xz"),
        include_bytes!("../test_files/good-0cat-empty"),
    );
}

#[test]
fn t2() {
    run_test(
        include_bytes!("../test_files/good-0catpad-empty.xz"),
        include_bytes!("../test_files/good-0catpad-empty"),
    );
}

#[test]
fn t3() {
    run_test(
        include_bytes!("../test_files/good-0-empty.xz"),
        include_bytes!("../test_files/good-0-empty"),
    );
}

#[test]
fn t4() {
    run_test(
        include_bytes!("../test_files/good-0-empty.xz"),
        include_bytes!("../test_files/good-0-empty"),
    );
}

#[test]
fn t5() {
    run_test(
        include_bytes!("../test_files/good-0pad-empty.xz"),
        include_bytes!("../test_files/good-0pad-empty"),
    );
}

#[test]
fn t6() {
    run_test2(
        include_bytes!("../test_files/good-1-3delta-lzma2.xz"),
        include_bytes!("../test_files/good-1-3delta-lzma2"),
    );
}

#[test]
fn t7() {
    run_test2(
        include_bytes!("../test_files/good-1-arm64-lzma2-1.xz"),
        include_bytes!("../test_files/good-1-arm64-lzma2-1"),
    );
}

#[test]
fn t8() {
    run_test2(
        include_bytes!("../test_files/good-1-arm64-lzma2-2.xz"),
        include_bytes!("../test_files/good-1-arm64-lzma2-2"),
    );
}

#[test]
fn t9() {
    run_test(
        include_bytes!("../test_files/good-1-block_header-1.xz"),
        include_bytes!("../test_files/good-1-block_header-1"),
    );
}

#[test]
fn t10() {
    run_test(
        include_bytes!("../test_files/good-1-block_header-2.xz"),
        include_bytes!("../test_files/good-1-block_header-2"),
    );
}

#[test]
fn t11() {
    run_test(
        include_bytes!("../test_files/good-1-block_header-3.xz"),
        include_bytes!("../test_files/good-1-block_header-3"),
    );
}

#[test]
fn t12() {
    run_test(
        include_bytes!("../test_files/good-1-check-crc32.xz"),
        include_bytes!("../test_files/good-1-check-crc32"),
    );
}

#[test]
fn t13() {
    run_test(
        include_bytes!("../test_files/good-1-check-crc64.xz"),
        include_bytes!("../test_files/good-1-check-crc64"),
    );
}

#[test]
fn t14() {
    run_test(
        include_bytes!("../test_files/good-1-check-none.xz"),
        include_bytes!("../test_files/good-1-check-none"),
    );
}

#[test]
fn t15() {
    run_test(
        include_bytes!("../test_files/good-1-check-sha256.xz"),
        include_bytes!("../test_files/good-1-check-sha256"),
    );
}

#[test]
fn t16() {
    run_test2(
        include_bytes!("../test_files/good-1-delta-lzma2.tiff.xz"),
        include_bytes!("../test_files/good-1-delta-lzma2.tiff"),
    );
}

#[test]
fn t17() {
    run_test(
        include_bytes!("../test_files/good-1-empty-bcj-lzma2.xz"),
        include_bytes!("../test_files/good-1-empty-bcj-lzma2"),
    );
}

#[test]
fn t18() {
    run_test2(
        include_bytes!("../test_files/good-1-lzma2-1.xz"),
        include_bytes!("../test_files/good-1-lzma2-1"),
    );
}

#[test]
fn t19() {
    run_test2(
        include_bytes!("../test_files/good-1-lzma2-2.xz"),
        include_bytes!("../test_files/good-1-lzma2-2"),
    );
}

#[test]
fn t20() {
    run_test2(
        include_bytes!("../test_files/good-1-lzma2-3.xz"),
        include_bytes!("../test_files/good-1-lzma2-3"),
    );
}

#[test]
fn t21() {
    run_test2(
        include_bytes!("../test_files/good-1-lzma2-4.xz"),
        include_bytes!("../test_files/good-1-lzma2-4"),
    );
}

#[test]
fn t22() {
    run_test2(
        include_bytes!("../test_files/good-1-lzma2-4.xz"),
        include_bytes!("../test_files/good-1-lzma2-4"),
    );
}

#[test]
fn t23() {
    run_test(
        include_bytes!("../test_files/good-1-check-crc64.xz"),
        include_bytes!("../test_files/good-1-check-crc64"),
    );
}

#[test]
fn t24() {
    run_test_expect_error(
        include_bytes!("../test_files/bad-0-backward_size.xz"),
        |err| assert!(matches!(err, XzError::FooterDecoderIndexMismatch(_, _))),
    );
}

#[test]
fn t25() {
    //TODO why?
    run_test(include_bytes!("../test_files/bad-0cat-alone.xz"), &[]);
}

#[test]
fn t26() {
    //TODO why?
    run_test(
        include_bytes!("../test_files/bad-0cat-header_magic.xz"),
        &[],
    );
}

#[test]
fn t27() {
    //TODO why?
    run_test(include_bytes!("../test_files/bad-0catpad-empty.xz"), &[]);
}

#[test]
fn t28() {
    run_test_expect_error(
        include_bytes!("../test_files/bad-0-empty-truncated.xz"),
        |err| assert_eq!(err, XzError::NeedsLargerInputBuffer),
    );
}

#[test]
fn t29() {
    run_test_expect_error(include_bytes!("../test_files/bad-0-footer_magic.xz"), |e| {
        assert_eq!(e, XzError::FooterMagicNumberMismatch)
    });
}

#[test]
fn t30() {
    run_test_expect_error(include_bytes!("../test_files/bad-0-header_magic.xz"), |e| {
        assert_eq!(e, XzError::StreamHeaderMagicNumberMismatch)
    });
}

#[test]
fn t31() {
    run_test_expect_error(
        include_bytes!("../test_files/bad-0-nonempty_index.xz"),
        |e| assert_eq!(e, XzError::CorruptedDataInBlockIndex),
    );
}

#[test]
fn t32() {
    //TODO why?
    run_test(include_bytes!("../test_files/bad-0pad-empty.xz"), &[]);
}

#[test]
fn t33() {
    run_test_expect_error(
        include_bytes!("../test_files/bad-1-block_header-1.xz"),
        |e| assert_eq!(e, XzError::BlockHeaderTooSmall),
    );
    run_test_expect_error(
        include_bytes!("../test_files/bad-1-block_header-2.xz"),
        |e| assert_eq!(e, XzError::BlockHeaderTooSmall),
    );
    run_test_expect_error(
        include_bytes!("../test_files/bad-1-block_header-3.xz"),
        |e| assert_eq!(e, XzError::BlockHeaderCrc32Mismatch(321064920, 857935832)),
    );
    run_test_expect_error(
        include_bytes!("../test_files/bad-1-block_header-4.xz"),
        |e| assert_eq!(e, XzError::LessDataInBlockBodyThanHeaderIndicated),
    );
    run_test_expect_error(
        include_bytes!("../test_files/bad-1-block_header-5.xz"),
        |e| assert_eq!(e, XzError::MoreDataInBlockBodyThanHeaderIndicated),
    );
    run_test_expect_error(
        include_bytes!("../test_files/bad-1-block_header-6.xz"),
        |e| assert_eq!(e, XzError::CorruptedDataInBlockIndex),
    );
}

#[test]
fn t34() {
    run_test_expect_error(
        include_bytes!("../test_files/bad-1-check-crc32-2.xz"),
        |e| assert_eq!(e, XzError::ContentCrc32Mismatch(362980163, 4288848707)),
    );
    run_test_expect_error(include_bytes!("../test_files/bad-1-check-crc32.xz"), |e| {
        assert_eq!(e, XzError::ContentCrc32Mismatch(362980163, 346202947))
    });
    run_test_expect_error(include_bytes!("../test_files/bad-1-check-crc64.xz"), |e| {
        assert_eq!(
            e,
            XzError::ContentCrc64Mismatch(14597925186004594415, 14669982780042522351)
        )
    });
    run_test_expect_error(include_bytes!("../test_files/bad-1-check-sha256.xz"), |e| {
        assert_eq!(
            e,
            XzError::ContentSha256Mismatch(
                [
                    142, 89, 53, 231, 225, 51, 104, 205, 150, 136, 254, 143, 72, 160, 149, 82, 147,
                    103, 106, 2, 21, 98, 88, 44, 126, 132, 141, 175, 225, 63, 176, 70
                ],
                [
                    142, 89, 53, 231, 225, 51, 104, 205, 150, 136, 254, 143, 72, 160, 149, 82, 147,
                    103, 106, 2, 21, 98, 88, 44, 126, 132, 141, 175, 225, 63, 176, 71
                ]
            )
        )
    });
}

#[test]
fn t35() {
    run_test_expect_error(include_bytes!("../test_files/bad-1-lzma2-1.xz"), |e| {
        assert_eq!(e, XzError::LzmaDictionaryResetExcepted)
    });
    run_test_expect_error(include_bytes!("../test_files/bad-1-lzma2-2.xz"), |e| {
        assert_eq!(e, XzError::DictionaryOverflow)
    });
    run_test_expect_error(include_bytes!("../test_files/bad-1-lzma2-3.xz"), |e| {
        assert_eq!(e, XzError::LzmaPropertiesInvalid)
    });
    run_test_expect_error(include_bytes!("../test_files/bad-1-lzma2-4.xz"), |e| {
        assert_eq!(e, XzError::LzmaPropertiesMissing)
    });
    run_test_expect_error(include_bytes!("../test_files/bad-1-lzma2-5.xz"), |e| {
        assert_eq!(e, XzError::LzmaPropertiesMissing)
    });
    run_test_expect_error(include_bytes!("../test_files/bad-1-lzma2-6.xz"), |e| {
        assert_eq!(e, XzError::CorruptedDataInLzma)
    });
    run_test_expect_error(include_bytes!("../test_files/bad-1-lzma2-7.xz"), |e| {
        assert_eq!(e, XzError::CorruptedDataInLzma)
    });
    run_test_expect_error(include_bytes!("../test_files/bad-1-lzma2-8.xz"), |e| {
        assert_eq!(e, XzError::LzmaPropertiesMissing)
    });
    run_test_expect_error(include_bytes!("../test_files/bad-1-lzma2-9.xz"), |e| {
        assert_eq!(e, XzError::MoreDataInBlockBodyThanHeaderIndicated)
    });
    run_test_expect_error(include_bytes!("../test_files/bad-1-lzma2-10.xz"), |e| {
        assert_eq!(e, XzError::MoreDataInBlockBodyThanHeaderIndicated)
    });
    run_test_expect_error(include_bytes!("../test_files/bad-1-lzma2-11.xz"), |e| {
        assert_eq!(e, XzError::MoreDataInBlockBodyThanHeaderIndicated)
    });
}

#[test]
fn t36() {
    run_test_expect_error(
        include_bytes!("../test_files/bad-1-stream_flags-1.xz"),
        |e| assert_eq!(e, XzError::FooterCheckTypeMismatch(512, XzCheckType::Crc32)),
    );
    run_test_expect_error(
        include_bytes!("../test_files/bad-1-stream_flags-2.xz"),
        |e| assert_eq!(e, XzError::StreamHeaderCrc32Mismatch(920527465, 1994269289)),
    );
    run_test_expect_error(
        include_bytes!("../test_files/bad-1-stream_flags-3.xz"),
        |e| assert_eq!(e, XzError::FooterCrc32Mismatch(228147856, 228082320)),
    );
}

#[test]
fn t37() {
    run_test_expect_error(include_bytes!("../test_files/bad-1-vli-1.xz"), |e| {
        assert_eq!(e, XzError::CorruptedUncompressedLengthVliInBlockHeader)
    });
    run_test_expect_error(include_bytes!("../test_files/bad-1-vli-2.xz"), |e| {
        assert_eq!(e, XzError::CorruptedUncompressedLengthVliInBlockHeader)
    });
}

#[test]
fn t38() {
    run_test_expect_error(
        include_bytes!("../test_files/bad-2-compressed_data_padding.xz"),
        |e| assert_eq!(e, XzError::CorruptedData),
    );
}

#[test]
fn t39() {
    run_test_expect_error(include_bytes!("../test_files/bad-2-index-1.xz"), |e| {
        assert_eq!(e, XzError::CorruptedData)
    });
    run_test_expect_error(include_bytes!("../test_files/bad-2-index-2.xz"), |e| {
        assert_eq!(e, XzError::CorruptedData)
    });
    run_test_expect_error(include_bytes!("../test_files/bad-2-index-3.xz"), |e| {
        assert_eq!(e, XzError::CorruptedData)
    });
    run_test_expect_error(include_bytes!("../test_files/bad-2-index-4.xz"), |e| {
        assert_eq!(e, XzError::IndexCrc32Mismatch(1575476230, 1558699014))
    });
    run_test_expect_error(include_bytes!("../test_files/bad-2-index-5.xz"), |e| {
        assert_eq!(e, XzError::CorruptedData)
    });
}

#[test]
fn t40() {
    run_test_expect_error(
        include_bytes!("../test_files/bad-3-index-uncomp-overflow.xz"),
        |e| assert_eq!(e, XzError::CorruptedData),
    );
}

#[test]
fn t41() {
    run_test_expect_error(
        include_bytes!("../test_files/unsupported-block_header.xz"),
        |e| assert_eq!(e, XzError::UnsupportedBlockHeaderOption),
    );
    run_test_expect_error(include_bytes!("../test_files/unsupported-check.xz"), |e| {
        assert_eq!(e, XzError::UnsupportedCheckType(2))
    });
    run_test_expect_error(
        include_bytes!("../test_files/unsupported-filter_flags-1.xz"),
        |e| assert_eq!(e, XzError::UnsupportedBlockHeaderOption),
    );
    run_test_expect_error(
        include_bytes!("../test_files/unsupported-filter_flags-2.xz"),
        |e| assert_eq!(e, XzError::UnsupportedBlockHeaderOption),
    );
    run_test_expect_error(
        include_bytes!("../test_files/unsupported-filter_flags-3.xz"),
        |e| assert_eq!(e, XzError::UnsupportedBcjFilter(33)),
    );
}

#[test]
fn t42() {
    run_test2(
        include_bytes!("../test_files/java_native_utils_amd64.so.xz"),
        include_bytes!("../test_files/java_native_utils_amd64.so"),
    );
}

#[test]
fn t43() {
    run_test2(
        include_bytes!("../test_files/java_native_utils_riscv64.so.xz"),
        include_bytes!("../test_files/java_native_utils_riscv64.so"),
    );
}

#[test]
fn t44() {
    run_test2(
        include_bytes!("../test_files/java_native_utils_armel.so.xz"),
        include_bytes!("../test_files/java_native_utils_armel.so"),
    );
}

#[test]
fn t45() {
    run_test2(
        include_bytes!("../test_files/java_native_utils_amd64.so2.xz"),
        include_bytes!("../test_files/java_native_utils_amd64.so"),
    );
}

#[test]
fn t46() {
    run_test_expect_error(
        include_bytes!("../test_files/bad-unsupported-stream-option.xz"),
        |e| assert_eq!(e, XzError::UnsupportedStreamHeaderOption),
    );

    run_test_expect_error(
        include_bytes!("../test_files/bad-unsupported-stream-option-2.xz"),
        |e| assert_eq!(e, XzError::UnsupportedStreamHeaderOption),
    );
}

#[test]
fn t47() {
    //This file has some history.
    //Basically I was running `xzcheck` on all .xz files on my nas.
    //This one failed. xz -t however succeeded!
    //Turns out this file found an integer underflow porting bug in dict_get
    //as well as me mistakenly using the wrong variable in dict_uncompressed.
    //2 bugs found by one file. Great success.
    //I do not precisely know where this file came from, all I know is that
    //Its a tar.bzip2.xz. Yes I dont know why I had that in this format either.
    //The bzip archive should contain a power pc 64 little endian openjdk 8,
    //but for this test case this is irrelevant, we obviously do not decode the bzip archive in this test.
    //I vaguely remember using this to develop and test a Java JNI C library in the past.
    //The extracted version of the archive was simply extracted with debian 12 "xz -d -k"
    run_test2(
        include_bytes!("../test_files/openjdk_8_bzip.xz"),
        include_bytes!("../test_files/openjdk_8_bzip"),
    );
}

#[test]
fn t48() {
    run_test2(
        include_bytes!("../test_files/concat_amd64_armel_blob.xz"),
        include_bytes!("../test_files/concat_amd64_armel_blob"),
    );
}

#[test]
fn t49() {
    run_test2(
        include_bytes!("../test_files/concat_amd64_armel_riscv_blob.xz"),
        include_bytes!("../test_files/concat_amd64_armel_riscv_blob"),
    );
}

#[test]
fn t50() {
    run_test2(
        include_bytes!("../test_files/rand4096.xz"),
        include_bytes!("../test_files/rand4096"),
    );
}

#[test]
fn t51() {
    run_test(
        include_bytes!("../test_files/rand128.xz"),
        include_bytes!("../test_files/rand128"),
    );
}

#[test]
fn t52() {
    run_test2(
        include_bytes!("../test_files/rand312.xz"),
        include_bytes!("../test_files/rand312"),
    );
}

#[test]
fn t53() {
    run_test_expect_error(include_bytes!("../test_files/bad-dict-props.xz"), |e| {
        assert_eq!(e, XzError::UnsupportedLzmaProperties(105))
    });
}

#[test]
fn t54() {
    run_test_expect_error(
        include_bytes!("../test_files/bad-2-lzma2-bad-vli-in-index.xz"),
        |e| assert_eq!(e, XzError::CorruptedDataInBlockIndex),
    );
}

#[test]
fn t55() {
    run_test_expect_error(
        include_bytes!("../test_files/bad-byte7-stream-header.xz"),
        |e| assert_eq!(e, XzError::UnsupportedStreamHeaderOption),
    );
}

#[test]
fn t56() {
    run_test_expect_error(
        include_bytes!("../test_files/bad-lzma-properties-too-large.xz"),
        |e| assert_eq!(e, XzError::LzmaPropertiesTooLarge),
    );
}

#[test]
fn t57() {
    run_test2(
        include_bytes!("../test_files/java_native_utils_amd64.so3.xz"),
        include_bytes!("../test_files/java_native_utils_amd64.so"),
    );
}

#[test]
fn t58() {
    run_test2(
        include_bytes!("../test_files/java_native_utils_armel.so2.xz"),
        include_bytes!("../test_files/java_native_utils_armel.so"),
    );
}

#[test]
fn t59() {
    run_test2(
        include_bytes!("../test_files/java_native_utils_riscv64.so2.xz"),
        include_bytes!("../test_files/java_native_utils_riscv64.so"),
    );
}
