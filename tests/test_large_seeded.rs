use sha2::{Digest, Sha256};
use std::fs;
use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::process::Command;
use std::sync::Mutex;

fn extend(seed: &mut [u8; 32]) {
    let mut sha = Sha256::new();
    Digest::update(&mut sha, &seed);
    seed.copy_from_slice(sha.finalize().as_slice());
}

static MTX: Mutex<()> = Mutex::new(());
fn gen_test_files(iter: u64, a: Vec<&str>) -> (String, String) {
    let mut sha = Sha256::new();
    sha.update(iter.to_le_bytes());
    for x in a.iter() {
        sha.update(x.as_bytes());
    }
    let hexi = hex::encode(sha.finalize().as_slice().to_vec());
    let base_file = "generated_testfiles/src.bin".to_string();
    let comp_file = format!("generated_testfiles/{}.bin.xz", &hexi);
    if fs::exists(&comp_file).unwrap() {
        return (base_file, comp_file);
    }

    if !fs::exists(&base_file).unwrap() {
        if !fs::exists("generated_testfiles").unwrap() {
            _ = fs::create_dir_all("generated_testfiles");
        }
        let mut cnt = 0u64;
        let mut seed = [69; 32];
        let mut file = BufWriter::new(File::create(&base_file).unwrap());
        while cnt < iter {
            cnt += 1;
            extend(&mut seed);
            file.write_all(seed.as_slice()).unwrap();
            if cnt % (iter / 100) == 0 {
                println!("GEN {}", cnt / (iter / 100))
            }
        }
        drop(file);
    }

    let temp = format!("generated_testfiles/{}.bin", &hexi);
    fs::rename(&base_file, &temp).unwrap();

    let mut args = Vec::new();
    args.push("xz");
    args.push("-k");
    args.extend_from_slice(a.as_slice());

    args.push(&temp);

    println!("Running xz... this will take a while!");
    let mut child = Command::new("/usr/bin/env").args(args).spawn().unwrap();
    let code = child.wait().unwrap();
    assert!(code.success());
    println!("xz is finished...");
    fs::rename(&temp, &base_file).unwrap();
    (base_file, comp_file)
}

const DESIRED_ITER: u64 = 150000000; //150000000;//312500000;
#[cfg(target_pointer_width = "32")]
pub fn test_large_seeded(a: Vec<&str>) {
    let env_var = std::env::var("TEST_LARGE_SEED").unwrap_or("false".to_string());
    if "true" != env_var.as_str() {
        return;
    }

    use xz4rust::XzReader;
    let guard = MTX.lock();
    let (base_file, comp_file) = gen_test_files(DESIRED_ITER, a);
    let compressed = File::open(&comp_file).unwrap();
    let mut raw = File::open(&base_file).unwrap();
    let mut r = XzReader::new(compressed);
    let mut buf = [0u8; 4096];
    let mut buf2 = [0u8; 4096];
    let mut total_read: u64 = 0;
    loop {
        let read = r.read(&mut buf).unwrap();
        total_read += read as u64;
        if read == 0 {
            assert_eq!(raw.read(&mut buf2).unwrap(), 0, "{}", total_read);
            break;
        }

        raw.read_exact(&mut buf2[0..read]).unwrap();
        assert_eq!(&buf[..read], &buf2[..read]);
    }
    drop(guard);
}

#[cfg(target_pointer_width = "64")]
pub fn test_large_seeded(a: Vec<&str>) {
    let env_var = std::env::var("TEST_LARGE_SEED").unwrap_or("false".to_string());
    if "true" != env_var.as_str() {
        return;
    }

    let guard = MTX.lock();
    let (base_file, comp_file) = gen_test_files(DESIRED_ITER, a);

    let mut compressed = File::open(&comp_file).unwrap();
    let mut raw = File::open(&base_file).unwrap();
    let compressed_size = compressed.metadata().unwrap().len();

    //assert!(compressed_size > u32::MAX as u64, "{}", compressed_size); //That is the point of this test.

    let raw_size = raw.metadata().unwrap().len();

    //assert!(raw_size > u32::MAX as u64, "{}", raw_size); //That is the point of this test.

    let mut compressed_buf = Vec::with_capacity(compressed_size as usize);
    compressed_buf.resize(compressed_size as usize, 0);

    let mut raw_buf = Vec::with_capacity(raw_size as usize);
    raw_buf.resize(raw_size as usize, 0);

    compressed.read_exact(&mut compressed_buf).unwrap();

    let mut decoder = xz4rust::XzDecoder::default();
    let res = decoder
        .decode(compressed_buf.as_slice(), raw_buf.as_mut_slice())
        .unwrap();

    assert_eq!(res.is_end_of_stream(), true);
    assert_eq!(res.input_consumed(), compressed_buf.len());
    assert_eq!(res.output_produced(), raw_buf.len());
    drop(compressed_buf);
    let mut expected_buf = Vec::with_capacity(raw_size as usize);
    expected_buf.resize(raw_size as usize, 0);

    raw.read_exact(&mut expected_buf).unwrap();

    assert_eq!(expected_buf.as_slice(), raw_buf.as_slice());
    drop(guard)
}

#[test]
pub fn test_default() {
    test_large_seeded(Vec::new());
}

#[test]
pub fn test_profile_1() {
    test_large_seeded(vec!["-1"]);
}

#[test]
pub fn test_profile_2() {
    test_large_seeded(vec!["-2"]);
}

#[test]
pub fn test_profile_3() {
    test_large_seeded(vec!["-3"]);
}

#[test]
pub fn test_profile_4() {
    test_large_seeded(vec!["-4"]);
}

#[test]
pub fn test_profile_5() {
    test_large_seeded(vec!["-5"]);
}

#[test]
pub fn test_profile_6() {
    test_large_seeded(vec!["-6"]);
}

#[test]
pub fn test_profile_7() {
    test_large_seeded(vec!["-7"]);
}

#[test]
pub fn test_profile_8() {
    test_large_seeded(vec!["-8"]);
}

#[test]
pub fn test_profile_9() {
    test_large_seeded(vec!["-9"]);
}

#[test]
pub fn test_arm() {
    test_large_seeded(vec!["--arm", "--lzma2"]);
}
#[test]
pub fn test_sparc() {
    test_large_seeded(vec!["--sparc", "--lzma2"]);
}

#[test]
pub fn test_ia64() {
    test_large_seeded(vec!["--ia64", "--lzma2"]);
}

#[test]
pub fn test_x86() {
    test_large_seeded(vec!["--x86", "--lzma2"]);
}

#[test]
pub fn test_armthumb() {
    test_large_seeded(vec!["--armthumb", "--lzma2"]);
}

#[test]
pub fn test_arm64() {
    test_large_seeded(vec!["--arm64", "--lzma2"]);
}

#[test]
pub fn test_powerpc() {
    test_large_seeded(vec!["--powerpc", "--lzma2"]);
}

#[test]
pub fn test_riscv() {
    test_large_seeded(vec!["--riscv", "--lzma2"]);
}

#[test]
pub fn test_arm_offset() {
    test_large_seeded(vec!["--arm=start=124", "--lzma2"]);
}
#[test]
pub fn test_sparc_offset() {
    test_large_seeded(vec!["--sparc=start=124", "--lzma2"]);
}

#[test]
pub fn test_ia64_offset() {
    test_large_seeded(vec!["--ia64=start=64", "--lzma2"]);
}

#[test]
pub fn test_x86_offset() {
    test_large_seeded(vec!["--x86=start=124", "--lzma2"]);
}

#[test]
pub fn test_armthumb_offset() {
    test_large_seeded(vec!["--armthumb=start=124", "--lzma2"]);
}

#[test]
pub fn test_arm64_offset() {
    test_large_seeded(vec!["--arm64=start=124", "--lzma2"]);
}

#[test]
pub fn test_powerpc_offset() {
    test_large_seeded(vec!["--powerpc=start=124", "--lzma2"]);
}

#[test]
pub fn test_riscv_offset() {
    test_large_seeded(vec!["--riscv=start=124", "--lzma2"]);
}
