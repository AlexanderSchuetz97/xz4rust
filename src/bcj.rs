use crate::clamp::{clamp_u32_to_u8, clamp_u64_to_u32, clamp_u64_to_u8, clamp_us_to_u32};
use crate::decoder::{DecodeResult, XzDictBuffer, XzError, XzInOutBuffer};

/// State for decoding the bcj filter.
/// Only used when bcj is enabled in the stream header
#[derive(Clone, Default, Debug)]
pub struct BcjFilterState {
    ///Filter type that we are currently using.
    bcj_filter_type: BcjFilter,
    /// flag if the next filter (probably the lzma decoder) is done.
    pub next_filter_done: bool,
    /// offset to use when processing BCJ filters.
    offset: u32,
    /// position marker that bcj filters use during filtering.
    pub pos: u32,
    /// special mask marker by the x86 bcj filter. Unused by all other filters.
    pub x86_prev_mask: usize,
    /// amount of bytes still filtered in the buf.
    pub filtered: usize,
    /// amount of bytes in the buffer.
    pub size: usize,
    /// temporary buffer the filter runs on.
    pub buf: [u8; 16],
}

impl BcjFilterState {
    /// Creates a new `BcjFilterState` in its default configuration.
    pub const fn new() -> Self {
        Self {
            bcj_filter_type: BcjFilter::X86,
            next_filter_done: false,
            offset: 0,
            pos: 0,
            x86_prev_mask: 0,
            filtered: 0,
            size: 0,
            buf: [0; 16],
        }
    }

    /// Returns true if the filter is done.
    const fn is_done(&self) -> bool {
        self.next_filter_done
    }

    /// Marks the filter as done.
    const fn set_done(&mut self) {
        self.next_filter_done = true;
    }

    /// Resets/Initializes the filter for the given filter type directly from the xz stream header.
    /// # Errors
    /// if the filter type with the given id is not supported by the implementation.
    pub fn reset(&mut self, id: u8, offset: u32) -> Result<(), XzError> {
        self.bcj_filter_type = BcjFilter::try_from(id)?;
        self.next_filter_done = false;
        self.pos = 0;
        self.offset = offset;
        self.x86_prev_mask = 0;
        self.filtered = 0;
        self.size = 0;
        Ok(())
    }

    /// flush the filtered bytes to the output buffer.
    fn flush(&mut self, b: &mut XzInOutBuffer) {
        let copy_size = b.output_remaining().min(self.filtered);

        let source = &self.buf.as_slice()[..copy_size];
        b.copy_to_output(source);

        self.filtered = self.filtered.wrapping_sub(copy_size);
        self.size = self.size.wrapping_sub(copy_size);

        self.buf.copy_within(copy_size..copy_size + self.size, 0);
    }

    /// run the bcj filter. Will also internally call the lzma decoder to
    /// supply the data that will then be run through the bcj filter.
    pub(crate) fn run<
        T: FnMut(&mut XzInOutBuffer, &mut XzDictBuffer) -> Result<DecodeResult, XzError>,
    >(
        &mut self,
        mut next_filter: T,
        b: &mut XzInOutBuffer,
        d: &mut XzDictBuffer,
    ) -> Result<DecodeResult, XzError> {
        if self.filtered > 0 {
            self.flush(b);
            if self.filtered > 0 {
                //More output needed
                return Ok(DecodeResult::NeedMoreData);
            }
            if self.is_done() {
                return Ok(DecodeResult::EndOfDataStructure);
            }
        }

        if self.size == 0 || self.size < b.output_remaining() {
            let out_start = b.output_position();
            b.copy_to_output(&self.buf.as_slice()[..self.size]);
            let ret = next_filter(b, d)?;
            let out_now = b.output_position();
            let size = out_now - out_start;
            b.output_seek_set(out_start);
            let out_consumed = self.apply(b.output_slice_mut(), 0, size) + out_start;
            b.output_seek_set(out_now);
            //TODO unfuck this buffer magic here.
            if ret == DecodeResult::EndOfDataStructure {
                self.set_done();
                return Ok(DecodeResult::EndOfDataStructure);
            }

            self.size = b.output_position() - out_consumed;
            debug_assert!(self.size <= self.buf.len());
            b.output_seek_sub(self.size); //WTF, do we seriously abuse the out buffer as some temp buffer here?

            let target = &mut self.buf.as_mut_slice()[..self.size];
            let source = &b.output_slice()[..self.size];
            target.copy_from_slice(source);

            if self.size < b.output_remaining() {
                return Ok(DecodeResult::NeedMoreData);
            }
        }

        if b.output_remaining() > 0 {
            let mut temp_buffer = XzInOutBuffer::new(b.input_slice(), &mut self.buf[self.size..]);
            debug_assert!(b.output_position() <= b.output_len());
            let ret = next_filter(&mut temp_buffer, d)?;
            self.size += temp_buffer.output_position();
            b.input_seek_add(temp_buffer.input_position());
            debug_assert!(self.size <= self.buf.len());

            let mut data = self.buf;
            self.filtered = self.apply(data.as_mut_slice(), self.filtered, self.size);
            self.buf = data;

            if ret == DecodeResult::EndOfDataStructure {
                self.set_done();
                self.filtered = self.size;
            }

            self.flush(b);
            if self.filtered > 0 {
                return Ok(DecodeResult::NeedMoreData);
            }
        }

        if self.is_done() {
            return Ok(DecodeResult::EndOfDataStructure);
        }

        Ok(DecodeResult::NeedMoreData)
    }

    /// apply the bcj filter to some bytes that the lzma decoder returned.
    fn apply(&mut self, mut buf: &mut [u8], mut pos: usize, mut size: usize) -> usize {
        buf = &mut buf[pos..];
        size = size.wrapping_sub(pos);
        let sl_buf = &mut buf[..size];

        let filtered = match self.bcj_filter_type {
            BcjFilter::X86 => {
                let (flt, mask) = bcj_x86(self.pos, self.offset, sl_buf, self.x86_prev_mask);
                self.x86_prev_mask = mask;
                flt
            }
            BcjFilter::PowerPc => bcj_powerpc(self.pos, self.offset, sl_buf),
            BcjFilter::IntelIthanium64 => bcj_ia64(self.pos, self.offset, sl_buf),
            BcjFilter::Arm => bcj_arm(self.pos, self.offset, sl_buf),
            BcjFilter::ArmThumb => bcj_armthumb(self.pos, self.offset, sl_buf),
            BcjFilter::Sparc => bcj_sparc(self.pos, self.offset, sl_buf),
            BcjFilter::Arm64 => bcj_arm64(self.pos, self.offset, sl_buf),
            BcjFilter::RiscV => bcj_riscv(self.pos, self.offset, sl_buf),
        };
        pos = pos.wrapping_add(filtered);

        //This wrap is needed and tests hit it.
        self.pos = self.pos.wrapping_add(clamp_us_to_u32(filtered));

        pos
    }
}

/// ?
const fn bcj_x86_test_msbyte(b: u8) -> bool {
    b == 0 || b == 0xff
}

/// runs the x86 bcj filter (filtered, mask)
fn bcj_x86(s_pos: u32, offset: u32, buf: &mut [u8], mask: usize) -> (usize, usize) {
    static MASK_TO_ALLOWED_STATUS: [bool; 8] = [true, true, true, false, true, false, false, false];
    static MASK_TO_BIT_NUM: [usize; 8] = [0, 1, 2, 2, 3, 3, 3, 3];
    let mut position: usize = 0;
    let mut prev_pos = usize::MAX;
    let mut prev_mask = mask;
    let mut src: u32;
    let mut dest: u32;
    let mut size = buf.len();
    if size <= 4 {
        return (0, mask);
    }
    size -= 4;
    while position < size {
        if buf[position] & 0xfe != 0xe8 {
            position = position.wrapping_add(1);
            continue;
        }
        prev_pos = position.wrapping_sub(prev_pos);
        if prev_pos <= 3 {
            prev_mask = prev_mask << prev_pos.wrapping_sub(1) & 7;
            if prev_mask != 0 {
                let b = buf[position
                    .wrapping_add(4)
                    .wrapping_sub(MASK_TO_BIT_NUM[prev_mask])];

                if !MASK_TO_ALLOWED_STATUS[prev_mask] || bcj_x86_test_msbyte(b) {
                    prev_pos = position;
                    prev_mask = prev_mask << 1 | 1;
                    position = position.wrapping_add(1);
                    continue;
                }
            }
        } else {
            prev_mask = 0;
        }

        prev_pos = position;
        if bcj_x86_test_msbyte(buf[position.wrapping_add(4)]) {
            src = u32::from_le_bytes([
                buf[position + 1],
                buf[position + 2],
                buf[position + 3],
                buf[position + 4],
            ]);
            loop {
                dest = src.wrapping_sub(
                    s_pos
                        .wrapping_add(offset)
                        .wrapping_add(clamp_us_to_u32(position))
                        .wrapping_add(5),
                );
                if prev_mask == 0 {
                    break;
                }
                let j = clamp_us_to_u32(MASK_TO_BIT_NUM[prev_mask] * 8);
                let b = clamp_u32_to_u8(dest >> (24i32 as u32).wrapping_sub(j));
                if !bcj_x86_test_msbyte(b) {
                    break;
                }
                src = dest
                    ^ ((1i32 as u32) << (32i32 as u32).wrapping_sub(j)).wrapping_sub(1i32 as u32);
            }
            dest &= 0x01ff_ffff;
            dest |= 0u32.wrapping_sub(dest & 0x0100_0000);

            buf[position + 1..position + 5].copy_from_slice(&dest.to_le_bytes());

            position = position.wrapping_add(4);
        } else {
            prev_mask = prev_mask << 1i32 | 1;
        }

        position = position.wrapping_add(1);
    }
    prev_pos = position.wrapping_sub(prev_pos);
    let new_mask = if prev_pos > 3 {
        0usize
    } else {
        prev_mask << prev_pos.wrapping_sub(1)
    };
    (position, new_mask)
}

/// runs the powerpc bcj filter
fn bcj_powerpc(s_pos: u32, offset: u32, buf: &mut [u8]) -> usize {
    let mut i: usize = 0;
    let size = buf.len() as u64 & !(3i32 as u64);
    while (i as u64) < size {
        let mut instr = u32::from_be_bytes([buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]);
        if instr & 0xfc00_0003 == 0x4800_0001 {
            instr &= 0x03ff_fffc;
            instr = instr.wrapping_sub(s_pos.wrapping_add(offset).wrapping_add(clamp_us_to_u32(i)));
            instr &= 0x03ff_fffc;
            instr |= 0x4800_0001;
            let as_be = u32::to_be_bytes(instr);
            buf[i] = as_be[0];
            buf[i + 1] = as_be[1];
            buf[i + 2] = as_be[2];
            buf[i + 3] = as_be[3];
        }
        i = i.wrapping_add(4);
    }
    i
}

/// runs the ia64 bcj filter
fn bcj_ia64(s_pos: u32, offset: u32, buf: &mut [u8]) -> usize {
    static BRANCH_TABLE: [u8; 32] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 4, 6, 6, 0, 0, 7, 7, 4, 4, 0, 0, 4, 4,
        0, 0,
    ];
    let mut i: usize = 0;
    let size = buf.len() & !15;
    while i < size {
        let mask = u32::from(BRANCH_TABLE[(buf[i] & 0x1f) as usize]);
        let mut slot = 0;
        let mut bit_pos = 5u32;
        while slot < 3 {
            if mask >> slot & 1 == 0 {
                slot += 1;
                bit_pos += 41;
                continue;
            }

            let byte_pos = bit_pos >> 3;
            let bit_res = bit_pos & 7;
            let mut instr = 0;
            let mut j = 0;
            while j < 6 {
                instr |= u64::from(buf[i.wrapping_add(j).wrapping_add(byte_pos as usize)])
                    << (8i32 as u64).wrapping_mul(j as u64);
                j += 1;
            }
            let mut norm = instr >> bit_res;
            if norm >> 37 & 0xf != 0x5 || (norm >> 9).trailing_zeros() < 3 {
                slot += 1;
                bit_pos += 41;
                continue;
            }

            let mut addr = clamp_u64_to_u32(norm >> 13 & 0xfffff);
            addr |= ((norm >> 36) as u32 & 1) << 20;
            addr <<= 4;
            addr = addr.wrapping_sub(s_pos.wrapping_add(offset).wrapping_add(clamp_us_to_u32(i)));
            addr >>= 4;
            norm &= !((0x008f_ffff) << 13);
            norm |= u64::from(addr & 0x000f_ffff) << 13;
            norm |= u64::from(addr & 0x0010_0000) << 16;
            instr &= ((1u64) << bit_res) - 1;
            instr |= norm << bit_res;
            let mut j = 0;
            while j < 6 {
                buf[i.wrapping_add(j).wrapping_add(byte_pos as usize)] =
                    clamp_u64_to_u8(instr >> (8i32 as u64).wrapping_mul(j as u64));
                j += 1;
            }
            slot += 1;
            bit_pos += 41;
        }
        i += 16;
    }
    i
}

/// runs the arm bcj filter
fn bcj_arm(s_pos: u32, offset: u32, buf: &mut [u8]) -> usize {
    let mut i = 0;
    let size = buf.len() & !3;
    while i < size {
        if buf[i.wrapping_add(3)] == 0xeb {
            let mut addr = u32::from(buf[i])
                | u32::from(buf[i.wrapping_add(1)]) << 8
                | u32::from(buf[i.wrapping_add(2)]) << 16;

            addr <<= 2;
            addr = addr.wrapping_sub(
                s_pos
                    .wrapping_add(offset)
                    .wrapping_add(clamp_us_to_u32(i))
                    .wrapping_add(8),
            );
            addr >>= 2;
            buf[i] = clamp_u32_to_u8(addr);
            buf[i.wrapping_add(1)] = clamp_u32_to_u8(addr >> 8);
            buf[i.wrapping_add(2)] = clamp_u32_to_u8(addr >> 16);
        }
        i += 4;
    }
    i
}

/// runs the armthumb bcj filter
fn bcj_armthumb(s_pos: u32, offset: u32, buf: &mut [u8]) -> usize {
    let mut i: usize = 0;
    if buf.len() < 4 {
        return 0;
    }
    let size = buf.len() - 4;
    while i <= size {
        if buf[i + 1] & 0xf8 != 0xf0 || buf[i + 3] & 0xf8 != 0xf8 {
            i += 2;
            continue;
        }

        let mut addr = (u32::from(buf[i + 1]) & 0x7) << 19
            | u32::from(buf[i]) << 11
            | (u32::from(buf[i + 3]) & 0x7) << 8
            | u32::from(buf[i + 2]);

        addr <<= 1;
        addr = addr.wrapping_sub(
            s_pos
                .wrapping_add(offset)
                .wrapping_add(clamp_us_to_u32(i))
                .wrapping_add(4),
        );
        addr >>= 1;
        buf[i + 1] = clamp_u32_to_u8(0xf0 | addr >> 19 & 0x7);
        buf[i] = clamp_u32_to_u8(addr >> 11);
        buf[i + 3] = clamp_u32_to_u8(0xf8 | addr >> 8 & 0x7);
        buf[i + 2] = clamp_u32_to_u8(addr);
        i += 4;
    }
    i
}

/// runs the sparc bcj filter
fn bcj_sparc(s_pos: u32, offset: u32, buf: &mut [u8]) -> usize {
    let mut i: usize = 0;
    let size = buf.len() & !3;
    while i < size {
        let mut instr = u32::from_be_bytes([buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]);
        if instr >> 22 == 0x100 || instr >> 22 == 0x1ff {
            instr <<= 2;
            instr = instr.wrapping_sub(s_pos.wrapping_add(offset).wrapping_add(clamp_us_to_u32(i)));
            instr >>= 2;
            instr = 0x4000_0000u32.wrapping_sub(instr & 0x0040_0000)
                | 0x4000_0000
                | instr & 0x003f_ffff;
            buf[i..i + 4].copy_from_slice(&instr.to_be_bytes());
        }
        i += 4;
    }
    i
}

/// runs the arm64 bcj filter
fn bcj_arm64(s_pos: u32, offset: u32, buf: &mut [u8]) -> usize {
    let mut i: usize = 0;
    let size = buf.len() & !15;
    while i < size {
        let mut instr = u32::from_le_bytes([buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]);
        if instr >> 26i32 == 0x25i32 as u32 {
            let addr = instr
                .wrapping_sub(s_pos.wrapping_add(offset).wrapping_add(clamp_us_to_u32(i)) >> 2i32);
            instr = 0x9400_0000 | addr & 0x03ff_ffff;
            buf[i..i + 4].copy_from_slice(instr.to_le_bytes().as_slice());
            i += 4;
            continue;
        }

        if instr & 0x9f00_0000 != 0x9000_0000 {
            i += 4;
            continue;
        }

        let mut addr = instr >> 29 & 3 | instr >> 3i32 & 0x001f_fffc;
        if addr.wrapping_add(0x20000) & 0x001c_0000 == 0 {
            addr = addr
                .wrapping_sub(s_pos.wrapping_add(offset).wrapping_add(clamp_us_to_u32(i)) >> 12i32);
            instr &= 0x9000_001f;
            instr |= (addr & 3) << 29i32;
            instr |= (addr & 0x3fffc) << 3i32;
            instr |= 0u32.wrapping_sub(addr & 0x20000) & 0x00e0_0000;
            buf[i..i + 4].copy_from_slice(instr.to_le_bytes().as_slice());
        }

        i += 4;
    }
    i
}
/// runs the riscv bcj filter
fn bcj_riscv(s_pos: u32, offset: u32, buf: &mut [u8]) -> usize {
    let mut i: usize = 0;
    let mut instr2: u32;
    let mut instr2_rs1: u32;
    let mut addr: u32;

    let Some(size) = buf.len().checked_sub(8) else {
        return 0;
    };

    while i <= size {
        let mut instr = u32::from(buf[i]);

        if instr == 0xefi32 as u32 {
            let b1 = u32::from(buf[i + 1]);
            if b1 & 0xd != 0 {
                i += 2;
                continue;
            }

            let b2 = u32::from(buf[i + 2]);
            let b3 = u32::from(buf[i + 3]);
            addr = (b1 & 0xf0i32 as u32) << 13i32 | b2 << 9i32 | b3 << 1i32;
            addr = addr.wrapping_sub(s_pos.wrapping_add(offset).wrapping_add(clamp_us_to_u32(i)));
            buf[i + 1] = clamp_u32_to_u8(b1 & 0xf | addr >> 8 & 0xf0);

            buf[i + 2] = clamp_u32_to_u8(addr >> 16 & 0xf | addr >> 7 & 0x10 | addr << 4 & 0xe0);

            buf[i + 3] = clamp_u32_to_u8(addr >> 4 & 0x7f | addr >> 13 & 0x80);
            i += 4;
            continue;
        }

        if instr & 0x7fi32 as u32 != 0x17i32 as u32 {
            i += 2;
            continue;
        }

        instr |= u32::from_le_bytes([0, buf[i + 1], buf[i + 2], buf[i + 3]]);

        if instr & 0xe80 != 0 {
            instr2 = u32::from_le_bytes([buf[i + 4], buf[i + 5], buf[i + 6], buf[i + 7]]);
            if (instr << 8 ^ instr2.wrapping_sub(3)) & 0xf8003 != 0 {
                i += 6;
                continue;
            }

            //TODO UNREACHED BY TEST
            addr = (instr & 0xffff_f000).wrapping_add(instr2 >> 20);
            instr = (0x17 | 2 << 7) | instr2 << 12;
            instr2 = addr;

            buf[i..i + 4].copy_from_slice(&instr.to_le_bytes());
            buf[i + 4..i + 8].copy_from_slice(&instr2.to_le_bytes());
            i += 8;
            continue;
        }

        instr2_rs1 = instr >> 27;
        if instr.wrapping_sub(0x3117) << 18 >= instr2_rs1 & 0x1d {
            i += 4;
            continue;
        }

        addr = u32::from_be_bytes([buf[i + 4], buf[i + 5], buf[i + 6], buf[i + 7]]);
        addr = addr.wrapping_sub(s_pos.wrapping_add(offset).wrapping_add(clamp_us_to_u32(i)));
        instr2 = instr >> 12 | addr << 20;
        instr = 0x17 | instr2_rs1 << 7 | addr.wrapping_add(0x800) & 0xffff_f000;

        buf[i..i + 4].copy_from_slice(&instr.to_le_bytes());
        buf[i + 4..i + 8].copy_from_slice(&instr2.to_le_bytes());
        i += 8;
    }
    i
}

/// All supported bcj filters.
#[derive(Debug, Eq, PartialEq, Default, Copy, Clone)]
#[repr(u8)]
enum BcjFilter {
    #[default]
    /// filter for RISC-V 32-bit and 64-bit architecture.
    RiscV = 0, //only so zero alloc is guaranteed to be valid!

    /// filter for 64-bit arm architecture. Debian calls this aarch64.
    Arm64,

    /// filter for Sun sparc architecture. Big or Little endian.
    Sparc,

    /// filter for Arm thumb architecture. Little endian only. Mostly used by microcontrollers.
    ArmThumb,

    /// filter for 32-bit Arm architecture. Little endian only.
    Arm,

    /// Intel Ithanium/ia64. Big and little endian.
    IntelIthanium64,

    /// 32-bit power pc architecture. big endian only
    PowerPc,

    /// filter for 32-bit and 64 bit x86/i386/i686/amd64 architecture.
    X86,
}

impl TryFrom<u8> for BcjFilter {
    type Error = XzError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            4 => Self::X86,
            5 => Self::PowerPc,
            6 => Self::IntelIthanium64,
            7 => Self::Arm,
            8 => Self::ArmThumb,
            9 => Self::Sparc,
            10 => Self::Arm64,
            11 => Self::RiscV,
            _ => return Err(XzError::UnsupportedBcjFilter(u32::from(value))),
        })
    }
}
