#[cfg(feature = "bcj")]
use crate::bcj::BcjFilterState;
use crate::clamp::{clamp_u32_to_u16, clamp_u32_to_u8, clamp_u64_to_u32, clamp_us_to_u32};
use crate::crc32::crc32;
#[cfg(feature = "delta")]
use crate::delta::DeltaDecoder;
#[cfg(feature = "sha256")]
use crate::sha256::XzSha256;
use crate::vli::{VliDecoder, VliResult};
use crate::DICT_SIZE_MAX;
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
#[cfg(feature = "alloc")]
use alloc::vec;
use core::fmt::{Debug, Display, Formatter};
use core::mem;
#[cfg(feature = "delta")]
use core::num::NonZeroUsize;
use core::ops::{Deref, DerefMut, Sub};

/// Input Output Buffer
#[derive(Debug)]
pub struct XzInOutBuffer<'a> {
    /// Input slice
    input: &'a [u8],
    /// Input position
    input_pos: usize,
    /// Output slice
    out: &'a mut [u8],
    /// Output position
    out_pos: usize,
}

impl<'a> XzInOutBuffer<'a> {
    /// Constructor
    pub const fn new(input: &'a [u8], output: &'a mut [u8]) -> Self {
        Self {
            input,
            input_pos: 0,
            out: output,
            out_pos: 0,
        }
    }

    /// get the input position
    pub const fn input_position(&self) -> usize {
        self.input_pos
    }

    /// Set the input position
    pub const fn input_seek_set(&mut self, position: usize) {
        debug_assert!(position <= self.input.len(),);
        self.input_pos = position;
    }

    /// Copy input to output at current position.
    /// # Panics
    /// if `copy_size` is larger than the remaining bytes in the output or input.
    pub fn copy_in_to_out(&mut self, copy_size: usize) {
        let new_in = self.input_pos + copy_size;
        let new_out = self.out_pos + copy_size;
        let src = &self.input[self.input_pos..new_in];
        let dst = &mut self.out[self.out_pos..new_out];
        dst.copy_from_slice(src);
        self.out_pos = new_out;
        self.input_pos = new_in;
    }

    /// Copy a slice to the output.
    pub fn copy_to_output(&mut self, data: impl AsRef<[u8]>) {
        let data = data.as_ref();
        let new_out = self.out_pos + data.len();
        let dst = &mut self.out[self.out_pos..new_out];
        dst.copy_from_slice(data);
        self.out_pos = new_out;
    }

    /// Add the given amount to the position.
    /// # Panics
    /// On debug builds if the `input_pos` would be out of bounds or if it would wrap usize.
    pub const fn input_seek_add(&mut self, amount: usize) {
        self.input_pos += amount;
        debug_assert!(self.input_pos <= self.input.len(),);
    }

    /// reads one byte from the input without changing the position and transforms it into a numeric type like u32.
    /// returns none if no input is available.
    pub fn input_peek_byte<T: From<u8>>(&self) -> Option<T> {
        self.input.get(self.input_pos).copied().map(T::from)
    }

    /// Reads one byte from the input, advance the position by one and transform it into a numeric type like u32.
    /// returns none if no input is available.
    pub fn input_read_byte<T: From<u8>>(&mut self) -> Option<T> {
        let data = self.input.get(self.input_pos).copied()?;
        self.input_pos += 1;
        Some(T::from(data))
    }

    /// Returns the input slice starting at the current input pos containing all remaining input bytes.
    pub fn input_slice(&self) -> &[u8] {
        &self.input[self.input_pos..]
    }

    /// Returns the amount of bytes that can still be read. equal to `input_slice().len()`
    pub const fn input_remaining(&self) -> usize {
        debug_assert!(self.input_pos <= self.input.len());
        self.input.len() - self.input_pos
    }

    /// Returns the full size of the input.
    pub const fn in_size(&self) -> usize {
        self.input.len()
    }

    /// Returns the full size of the output
    pub const fn output_len(&self) -> usize {
        self.out.len()
    }

    /// Returns the output slice starting at `out_pos`.
    /// This slice would contain all bytes that can and should still be filled with data.
    /// TODO refactor this, the bcj filter decrements the `out_pos` and then calls this...
    #[cfg(feature = "bcj")] //only used by this feature for now
    pub fn output_slice(&self) -> &[u8] {
        &self.out[self.out_pos..]
    }

    /// Returns an output slice that starts at `start_idx` and goes until the current output buffer position.
    /// This can be used to re-inspect bytes in the output buffer.
    /// #Panics
    /// if `start_idx` is larger than the current position.
    pub fn output_slice_look_back(&self, start_idx: usize) -> &[u8] {
        &self.out[start_idx..self.out_pos]
    }

    /// Returns a mutable output slice that starts at `start_idx` and goes until the current output buffer position.
    /// This can be used to re-inspect bytes in the output buffer.
    /// #Panics
    /// if `start_idx` is larger than the current position.
    #[cfg(feature = "delta")] //Currently used to apply the delta filter.
    pub fn output_slice_look_back_mut(&mut self, start_idx: usize) -> &mut [u8] {
        &mut self.out[start_idx..self.out_pos]
    }

    /// Returns the mutable output slice
    #[cfg(feature = "bcj")] //Only used by bcj filters.
    pub fn output_slice_mut(&mut self) -> &mut [u8] {
        &mut self.out[self.out_pos..]
    }

    /// Returns the output position
    pub const fn output_position(&self) -> usize {
        self.out_pos
    }

    /// Returns the remaining bytes that can still be filled with output.
    pub const fn output_remaining(&self) -> usize {
        self.output_len() - self.output_position()
    }

    //pub fn output_seek_add(&mut self, amount: usize) {
    //    self.out_pos += amount;
    //    debug_assert!(self.out_pos <= self.out.len());
    //}

    /// TODO get rid of this.
    #[cfg(feature = "bcj")] //only used by this feature for now
    pub const fn output_seek_sub(&mut self, amount: usize) {
        self.out_pos -= amount;
    }

    //pub fn output_seek_add(&mut self, amount: usize) {
    //     debug_assert!(self.out_pos + amount <= self.out.len());
    //     self.out_pos += amount;
    // }

    /// Set the output position to an absolute value
    /// TODO do we really need this?
    #[cfg(feature = "bcj")] //only used by this feature for now
    pub const fn output_seek_set(&mut self, amount: usize) {
        debug_assert!(amount <= self.out.len());
        self.out_pos = amount;
    }
}

/// The lzma2 decoder
#[derive(Debug)]
pub struct XzLzma2Decoder {
    /// Range decoder
    rc: RcDecoder,
    /// State machine of the decoder.
    sequence: LzmaStreamState,
    /// State machine of the decoder.
    next_sequence: LzmaStreamState,
    /// Uncompressed amount of data
    uncompressed: usize,
    /// Compressed amount of data.
    compressed: usize,
    /// Does the dictionary need to be reset next?
    need_dict_reset: bool,
    /// Do we need lzma properties next?
    need_props: bool,
    /// low level lzma specific decoding state
    lzma: LzmaDecoderState,
    /// temp buffer size (i.e. amount of bytes in `temp_buf`)
    temp_size: usize,
    /// small temporary buffer
    temp_buf: [u8; 63],
}

impl XzLzma2Decoder {
    /// Constructor.
    const fn new() -> Self {
        Self {
            rc: RcDecoder::new(),
            sequence: LzmaStreamState::Control,
            next_sequence: LzmaStreamState::Control,
            uncompressed: 0,
            compressed: 0,
            need_dict_reset: false,
            need_props: false,
            lzma: LzmaDecoderState::new(),
            temp_size: 0,
            temp_buf: [0; 63],
        }
    }

    /// reset the state back to the default state.
    fn reset(&mut self) {
        self.lzma.reset();
        self.rc.reset();
    }

    /// reset lzma decoder.
    fn xz_dec_lzma2_reset(&mut self, props: u8, d: &mut XzDictBuffer) -> Result<(), XzError> {
        if props > 39 {
            return Err(XzError::UnsupportedLzmaProperties(u32::from(props)));
        }
        let mut dict_size = 2 + usize::from(props & 1);
        dict_size <<= (props >> 1) + 11;
        if dict_size > d.max_size() {
            return Err(XzError::DictionaryTooLarge(dict_size as u64));
        }

        d.alloc_dict(dict_size)?;

        self.sequence = LzmaStreamState::Control;
        self.need_dict_reset = true;
        self.temp_size = 0;
        Ok(())
    }

    /// main lzma2 decoding loop
    fn lzma_main(&mut self, rcb: &mut RcBuf, d: &mut XzDictBuffer) -> Result<(), XzError> {
        if d.dict_has_space() && self.lzma.len > 0 {
            // The C code did not check for failure here, for some reason? No test case reaches Err here!
            if let Ok(count) = d.dict_repeat(self.lzma.rep0 as usize, self.lzma.len) {
                self.lzma.len -= count;
            }
        }

        while d.dict_has_space() && !rcb.limit_exceeded() {
            let pos_state = d.dict_pos() & self.lzma.pos_mask;
            let index = (16 * self.lzma.state.num()) + pos_state;

            if self.rc.rc_bit(&mut self.lzma.is_match[index], rcb) {
                self.lzma_literal(rcb, d);
                continue;
            }

            if self
                .rc
                .rc_bit(&mut self.lzma.is_rep[self.lzma.state as usize], rcb)
            {
                self.lzma_match(clamp_us_to_u32(pos_state), rcb);
            } else {
                self.lzma_rep_match(clamp_us_to_u32(pos_state), rcb);
            }

            self.lzma.len -= d.dict_repeat(self.lzma.rep0 as usize, self.lzma.len)?;
        }
        self.rc.normalize(rcb);
        Ok(())
    }

    /// call the lzma2 decoder.
    fn lzma2_lzma(&mut self, b: &mut XzInOutBuffer, d: &mut XzDictBuffer) -> Result<(), XzError> {
        if self.temp_size > 0 || self.compressed == 0 {
            let mut amount_of_data_to_process = (42 - self.temp_size).min(b.input_remaining());

            if let Some(sub) = self.compressed.checked_sub(self.temp_size) {
                amount_of_data_to_process = amount_of_data_to_process.min(sub);
            }

            let target = &mut self.temp_buf.as_mut_slice()
                [self.temp_size..self.temp_size + amount_of_data_to_process];
            let source = &b.input_slice()[..amount_of_data_to_process];

            target.copy_from_slice(source);
            let new_len = self.temp_size + amount_of_data_to_process;

            if new_len < 21 && new_len != self.compressed {
                //Not enough data to make progress.
                self.temp_size += amount_of_data_to_process;
                b.input_seek_add(amount_of_data_to_process);
                return Ok(());
            }

            let limit = if new_len == self.compressed {
                let len = self.temp_buf.len() - self.temp_size - amount_of_data_to_process;

                debug_assert!(len <= 63);
                debug_assert!(new_len < 63);
                debug_assert!(new_len + len <= 63);

                self.temp_buf[new_len..new_len + len].fill(0);
                new_len
            } else {
                new_len - 21
            };

            let cl = self.temp_buf; //TODO get rid of this copy by outsmarting borrow checker at some point.
            let mut rcb = RcBuf {
                input: cl.as_slice(),
                in_pos: 0,
                in_limit: limit,
            };

            self.lzma_main(&mut rcb, d)?;
            if rcb.in_pos > new_len {
                //TODO unreached
                return Err(XzError::CorruptedDataInLzma);
            }
            self.compressed -= rcb.in_pos;
            if rcb.in_pos < self.temp_size {
                self.temp_size -= rcb.in_pos;
                self.temp_buf.copy_within(rcb.in_pos.., 0);
                return Ok(());
            }

            b.input_seek_add(rcb.in_pos.wrapping_sub(self.temp_size));
            self.temp_size = 0;
        }

        let mut in_avail = b.in_size().wrapping_sub(b.input_pos);
        if in_avail >= 21 {
            let mut rcb = RcBuf {
                input: b.input,
                in_pos: b.input_pos,
                in_limit: 0,
            };

            if in_avail >= self.compressed + 21 {
                rcb.in_limit = b.input_pos + self.compressed;
            } else {
                rcb.in_limit = b.in_size() - 21;
            }

            self.lzma_main(&mut rcb, d)?;

            in_avail = rcb.in_pos - b.input_pos;
            if in_avail > self.compressed {
                //TODO unreached
                return Err(XzError::CorruptedDataInLzma);
            }
            //TODO doesnt wrap!
            self.compressed = self.compressed.wrapping_sub(in_avail);
            b.input_pos = rcb.in_pos;
        }
        in_avail = b.input_remaining();
        if in_avail < 21 {
            if in_avail > self.compressed {
                in_avail = self.compressed;
            }

            let source = &b.input_slice()[..in_avail];
            self.temp_buf[..in_avail].copy_from_slice(source);

            self.temp_size = in_avail;
            b.input_pos = b.input_pos.wrapping_add(in_avail);
        }
        Ok(())
    }

    /// process the lzma decoders state machine.
    #[allow(clippy::too_many_lines)]
    pub fn xz_dec_lzma2_run(
        &mut self,
        b: &mut XzInOutBuffer,
        d: &mut XzDictBuffer,
    ) -> Result<DecodeResult, XzError> {
        loop {
            match self.sequence {
                LzmaStreamState::Control => {
                    let Some(tmp) = b.input_read_byte::<u8>() else {
                        return Ok(DecodeResult::NeedMoreData);
                    };

                    if tmp == 0 {
                        self.sequence = LzmaStreamState::Finished;
                        return Ok(DecodeResult::EndOfDataStructure);
                    }

                    if tmp >= 0xe0 || tmp == 0x1 {
                        self.need_props = true;
                        self.need_dict_reset = false;
                        d.dict_reset();
                    } else if self.need_dict_reset {
                        return Err(XzError::LzmaDictionaryResetExcepted);
                    }

                    if tmp < 0x80 {
                        if tmp > 0x2 {
                            return Err(XzError::CorruptedDataInLzma);
                        }
                        self.sequence = LzmaStreamState::Compressed0;
                        self.next_sequence = LzmaStreamState::Copy;
                        continue;
                    }

                    self.uncompressed = ((tmp as usize) & 0x1f) << 16;
                    self.sequence = LzmaStreamState::Uncompressed1;
                    if tmp >= 0xc0 {
                        self.need_props = false;
                        self.next_sequence = LzmaStreamState::Properties;
                        continue;
                    }
                    if self.need_props {
                        return Err(XzError::LzmaPropertiesMissing);
                    }
                    self.next_sequence = LzmaStreamState::LzmaPrepare;
                    if tmp >= 0xa0 {
                        self.reset();
                    }
                }
                LzmaStreamState::Uncompressed1 => {
                    let Some(next_byte) = b.input_read_byte::<usize>() else {
                        return Ok(DecodeResult::NeedMoreData);
                    };

                    self.uncompressed = self.uncompressed.wrapping_add(next_byte << 8);
                    self.sequence = LzmaStreamState::Uncompressed2;
                }
                LzmaStreamState::Uncompressed2 => {
                    let Some(next_byte) = b.input_read_byte::<usize>() else {
                        return Ok(DecodeResult::NeedMoreData);
                    };

                    self.uncompressed += next_byte + 1;
                    self.sequence = LzmaStreamState::Compressed0;
                }
                LzmaStreamState::Compressed0 => {
                    let Some(next_byte) = b.input_read_byte::<usize>() else {
                        return Ok(DecodeResult::NeedMoreData);
                    };
                    self.compressed = next_byte << 8;
                    self.sequence = LzmaStreamState::Compressed1;
                }
                LzmaStreamState::Compressed1 => {
                    let Some(next_byte) = b.input_read_byte::<usize>() else {
                        return Ok(DecodeResult::NeedMoreData);
                    };

                    self.compressed += next_byte + 1;
                    self.sequence = self.next_sequence;
                }
                LzmaStreamState::Properties => {
                    let Some(next_byte) = b.input_read_byte() else {
                        return Ok(DecodeResult::NeedMoreData);
                    };
                    self.lzma_props(next_byte)?;

                    self.sequence = LzmaStreamState::LzmaPrepare;
                }
                LzmaStreamState::LzmaPrepare => {
                    //INFO not sure if relevant but c impl did check bounds of input here.
                    if self.compressed < 5 {
                        //TODO unreached
                        return Err(XzError::CorruptedDataInLzma);
                    }
                    if !self.rc.read_init(b) {
                        return Ok(DecodeResult::NeedMoreData);
                    }

                    self.compressed -= 5;
                    self.sequence = LzmaStreamState::LzmaRun;
                }
                LzmaStreamState::LzmaRun => {
                    let remaining = b.output_remaining();
                    let out_max = if remaining < self.uncompressed {
                        remaining
                    } else {
                        self.uncompressed
                    };

                    d.dict_limit(out_max);

                    self.lzma2_lzma(b, d)?;

                    self.uncompressed -= d.dict_flush(b);

                    if self.uncompressed == 0 {
                        if self.compressed > 0 || self.lzma.len > 0 || !self.rc.is_finished() {
                            return Err(XzError::CorruptedDataInLzma);
                        }
                        self.rc.reset();
                        self.sequence = LzmaStreamState::Control;
                        continue;
                    }
                    if b.output_remaining() == 0 {
                        return Ok(DecodeResult::NeedMoreData);
                    }
                    if b.input_pos == b.in_size() && self.temp_size < self.compressed {
                        return Ok(DecodeResult::NeedMoreData);
                    }

                    //TODO UNREACHED, Why?
                }
                LzmaStreamState::Copy => {
                    if b.input_pos >= b.in_size() {
                        return Ok(DecodeResult::NeedMoreData);
                    }

                    self.compressed = d.dict_uncompressed(b, self.compressed);
                    if self.compressed > 0 {
                        return Ok(DecodeResult::NeedMoreData);
                    }
                    self.sequence = LzmaStreamState::Control;
                }
                LzmaStreamState::Finished => {
                    return Ok(DecodeResult::EndOfDataStructure);
                }
            }
        }
    }

    /// Decode the length of the match into self.lzma.len.
    fn lzma_len(&mut self, is_rep: bool, pos_state: u32, rcb: &mut RcBuf) {
        let l = if is_rep {
            &mut self.lzma.rep_len_dec
        } else {
            &mut self.lzma.match_len_dec
        };

        let probs = if self.rc.rc_bit(&mut l.choice, rcb) {
            let probs = l.low[pos_state as usize].as_mut_slice();
            self.lzma.len = 2;
            probs
        } else if self.rc.rc_bit(&mut l.choice2, rcb) {
            let probs = l.mid[pos_state as usize].as_mut_slice();
            self.lzma.len = 2 + ((1) << 3);
            probs
        } else {
            let probs = l.high.as_mut_slice();
            self.lzma.len = 2 + ((1) << 3) + ((1) << 3);
            probs
        };

        self.lzma.len = self.lzma.len.wrapping_add(
            self.rc
                .rc_bittree(probs, rcb)
                .wrapping_sub(clamp_us_to_u32(probs.len())) as usize,
        );
    }

    /// Decode a match. The distance will be stored in self.lzma.rep0.
    fn lzma_match(&mut self, pos_state: u32, rcb: &mut RcBuf) {
        self.lzma.state = self.lzma.state.u32_match();
        self.lzma.rep3 = self.lzma.rep2;
        self.lzma.rep2 = self.lzma.rep1;
        self.lzma.rep1 = self.lzma.rep0;
        self.lzma_len(false, pos_state, rcb);

        let slot = match self.lzma.len {
            0..2 => unreachable!("dist_slot should not be less than 2 after lzma_len"),
            2 => 0..64,
            3 => 64..128,
            4 => 128..192,
            _ => 192..256,
        };

        let probs = &mut self.lzma.dist_slot[slot];
        let dist_slot = self
            .rc
            .rc_bittree(probs, rcb)
            .wrapping_sub(((1i32) << 6i32) as u32);

        if dist_slot < 4i32 as u32 {
            self.lzma.rep0 = dist_slot;
            return;
        }

        let limit = (dist_slot >> 1i32).wrapping_sub(1i32 as u32);
        self.lzma.rep0 = 2u32.wrapping_add(dist_slot & 1i32 as u32);

        if dist_slot < 14 {
            self.lzma.rep0 <<= limit;

            let total_offset = ((256 + self.lzma.rep0 as usize) - dist_slot as usize) - 1;

            let probs = &mut self.lzma.dist_slot.as_mut_slice()[total_offset..];

            self.lzma.rep0 = self.rc.bittree_reverse(probs, self.lzma.rep0, limit, rcb);
            return;
        }

        self.lzma.rep0 = self.rc.direct(self.lzma.rep0, limit - 4, rcb) << 4;

        let probs = &mut self.lzma.dist_slot.as_mut_slice()[370..];

        self.lzma.rep0 = self
            .rc
            .bittree_reverse(probs, self.lzma.rep0, 4i32 as u32, rcb);
    }

    /// Get index to the literal coder probability array.
    fn lzma_literal_probs(&self, d: &mut XzDictBuffer) -> usize {
        // Should always hold true.
        debug_assert!(self.lzma.lc <= 8);

        let prev_byte: u32 = u32::from(d.dict_get(0));
        let low: u32 = prev_byte >> (8 - self.lzma.lc);
        let high: u32 =
            clamp_us_to_u32((d.dict_pos() & self.lzma.literal_pos_mask as usize) << self.lzma.lc);

        low.wrapping_add(high) as usize
    }
    /// Decode a literal (one 8-bit byte)
    fn lzma_literal(&mut self, rcb: &mut RcBuf, d: &mut XzDictBuffer) {
        let probs = self.lzma_literal_probs(d);
        if self.lzma.state.u32_is_literal() {
            let n = &mut self.lzma.literal[probs][..0x100];
            let symbol = self.rc.rc_bittree(n, rcb);
            d.dict_put(clamp_u32_to_u8(symbol));
            self.lzma.state = self.lzma.state.u32_literal();
            return;
        }

        let mut symbol = 1;
        let mut match_byte = u32::from(d.dict_get(self.lzma.rep0 as usize)) << 1;
        let mut offset = 0x100;
        loop {
            let match_bit = match_byte & offset;
            match_byte <<= 1;
            let i = offset.wrapping_add(match_bit).wrapping_add(symbol);
            let probs = &mut self.lzma.literal[probs];

            if self.rc.rc_bit(&mut probs[i as usize], rcb) {
                symbol <<= 1;
                offset &= !match_bit;
            } else {
                symbol = (symbol << 1) | 1;
                offset &= match_bit;
            }

            if symbol >= 0x100 {
                break;
            }
        }
        d.dict_put(clamp_u32_to_u8(symbol));
        self.lzma.state = self.lzma.state.u32_literal();
    }

    /// Decode a repeated match. The distance is one of the four most recently
    /// seen matches. The distance will be stored in self.lzma.rep0.
    fn lzma_rep_match(&mut self, pos_state: u32, rcb: &mut RcBuf) {
        let index = self.lzma.state.num() + 12;
        if self
            .rc
            .rc_bit(&mut self.lzma.is_rep.as_mut_slice()[index], rcb)
        {
            let index = (16 * self.lzma.state.num()) + pos_state as usize;
            if self.rc.rc_bit(&mut self.lzma.is_rep0_long[index], rcb) {
                self.lzma.state = self.lzma.state.u32_short_rep();
                self.lzma.len = 1;
                return;
            }

            self.lzma.state = self.lzma.state.u32_long_rep();
            self.lzma_len(true, pos_state, rcb);
            return;
        }

        let index = self.lzma.state.num() + 24;
        if self.rc.rc_bit(&mut self.lzma.is_rep[index], rcb) {
            mem::swap(&mut self.lzma.rep1, &mut self.lzma.rep0);

            self.lzma.state = self.lzma.state.u32_long_rep();
            self.lzma_len(true, pos_state, rcb);
            return;
        }

        let index = self.lzma.state.num() + 36;
        if self.rc.rc_bit(&mut self.lzma.is_rep[index], rcb) {
            let tmp = self.lzma.rep2;
            self.lzma.rep2 = self.lzma.rep1;
            self.lzma.rep1 = self.lzma.rep0;
            self.lzma.rep0 = tmp;

            self.lzma.state = self.lzma.state.u32_long_rep();
            self.lzma_len(true, pos_state, rcb);
            return;
        }

        let tmp = self.lzma.rep3;
        self.lzma.rep3 = self.lzma.rep2;
        self.lzma.rep2 = self.lzma.rep1;
        self.lzma.rep1 = self.lzma.rep0;
        self.lzma.rep0 = tmp;

        self.lzma.state = self.lzma.state.u32_long_rep();
        self.lzma_len(true, pos_state, rcb);
    }

    /// Decode and validate LZMA properties (lc/lp/pb) and calculate the bit masks
    /// from the decoded lp and pb values. On success, the LZMA decoder state is
    /// reset and Ok is returned.
    fn lzma_props(&mut self, mut props: u8) -> Result<(), XzError> {
        if props > 224 {
            return Err(XzError::LzmaPropertiesTooLarge);
        }

        let mut cnt = 0;
        while props >= 45 {
            props -= 45;
            cnt += 1;
        }
        self.lzma.pos_mask = (1 << cnt) - 1;
        let mut cnt = 0;
        while props >= 9 {
            props -= 9;
            cnt += 1;
        }
        self.lzma.lc = u32::from(props);
        if self.lzma.lc + cnt > 4 {
            return Err(XzError::LzmaPropertiesInvalid);
        }
        self.lzma.literal_pos_mask = (1u32 << cnt) - 1;
        self.reset();
        Ok(())
    }
}

/// .
#[derive(Clone, Debug)]
struct LzmaDecoderState {
    /// Distance of latest match
    rep0: u32,
    /// Distance of 2nd least match
    rep1: u32,
    /// Distance of 3rd latest match
    rep2: u32,
    /// Distance of 4th latest match
    rep3: u32,
    /// Length of a match
    len: usize,
    /// Types of the most recently seen LZMA symbols
    state: LzmaState,
    /// LZMA Properties
    lc: u32,
    /// LZMA Bitmask
    literal_pos_mask: u32, // max possible value of this field is 15!
    /// LZMA Bitmask
    pos_mask: usize, // max possible value of this field is 15!
    /// If 1, it's a match. Otherwise, it's a single 8-bit literal.
    is_match: [u16; 192],
    /// 0 -> 11: If 1, it's a repeated match. The distance is one of rep0 to rep3.
    /// 12 -> 23: If 0, distance of a repeated match is rep0. Otherwise, check is rep1.
    /// 24 -> 35: If 0, distance of a repeated match is rep1. Otherwise, check is rep2.
    /// 36 -> 47: If 0, distance of a repeated match is rep2. Otherwise, it is rep3.
    is_rep: [u16; 48],

    /// If 1, the repeated match has length of one byte.
    /// Otherwise, the length is decoded from rep len decoder.
    is_rep0_long: [u16; 192],

    /// Probability tree for the highest two bits of the match distance.
    /// Elements are grouped in 4 groups of 64 each.
    ///
    /// Element 256 to 369
    /// Probability trees for additional bits for match distance
    /// when the distance is in the range [4, 127].
    ///
    /// Element 370 to 385:
    /// Probability tree for the lowest four bits of a match
    /// distance that is equal to or greater than 128.
    dist_slot: [u16; 386],

    /// length of a normal match
    match_len_dec: LzmaLenDecoder,
    /// length of a repeated match
    rep_len_dec: LzmaLenDecoder,
    /// probabilities of literals.
    literal: [[u16; 768]; 16],
}

impl LzmaDecoderState {
    /// Constructor
    const fn new() -> Self {
        Self {
            rep0: 0,
            rep1: 0,
            rep2: 0,
            rep3: 0,
            state: LzmaState::LitLit,
            len: 0,
            lc: 0,
            literal_pos_mask: 0,
            pos_mask: 0,
            is_match: [0; 192],
            is_rep: [0; 48],
            is_rep0_long: [0; 192],
            dist_slot: [0; 386],
            match_len_dec: LzmaLenDecoder::new(),
            rep_len_dec: LzmaLenDecoder::new(),
            //TODO future maybe depending on feature gate alloc we should alloc this on the heap.
            #[allow(clippy::large_stack_arrays)]
            literal: [[1024; 768]; 16],
        }
    }

    /// Resets the decoder to its intal state.
    pub fn reset(&mut self) {
        self.state = LzmaState::LitLit;
        self.rep0 = 0;
        self.rep1 = 0;
        self.rep2 = 0;
        self.rep3 = 0;
        self.len = 0;
        self.is_match = [1024; 192];
        self.is_rep = [1024; 48];
        self.is_rep0_long = [1024; 192];
        self.dist_slot = [1024; 386];
        self.match_len_dec.reset();
        self.rep_len_dec.reset();
        self.literal
            .iter_mut()
            .flat_map(|x| x.iter_mut())
            .for_each(|x| *x = 1024);
        //self.literal = [[1024; 768]; 16];
    }
}

impl Default for LzmaDecoderState {
    fn default() -> Self {
        Self::new()
    }
}

/// LZMA Length decoder
#[derive(Clone, Debug)]
struct LzmaLenDecoder {
    /// Probability of match length being at least 10
    choice: u16,
    /// Probability of match length being at least 18
    choice2: u16,
    /// Probabilities for match lengths 2-9
    low: [[u16; 8]; 16],
    /// Probabilities for match lengths 10-17
    mid: [[u16; 8]; 16],
    /// Probabilities for match lengths 18-273
    high: [u16; 256],
}

impl LzmaLenDecoder {
    /// create a new decoder
    const fn new() -> Self {
        Self {
            choice: 1024,
            choice2: 1024,
            low: [[1024; 8]; 16],
            mid: [[1024; 8]; 16],
            high: [1024; 256],
        }
    }

    /// reset the decoder to the inital state.
    const fn reset(&mut self) {
        self.choice = 1024;
        self.choice2 = 1024;
        self.low = [[1024; 8]; 16];
        self.mid = [[1024; 8]; 16];
        self.high = [1024; 256];
    }
}

impl Default for LzmaLenDecoder {
    fn default() -> Self {
        Self::new()
    }
}

///
/// This enum is used to track which LZMA symbols have occurred most recently
/// and in which order. This information is used to predict the next symbol.
///
/// Symbols:
///  - Literal: One 8-bit byte
///  - Match: Repeat a chunk of data at some distance
///  - Long repeat: Multi-byte match at a recently seen distance
///  - Short repeat: One-byte repeat at a recently seen distance
///
/// The symbol names are in from `OldestOlderPrevious`. REP means
/// either short or long repeated match, and `NonLit` means any non-literal.
#[derive(Clone, Debug, Copy, Default, Ord, PartialEq, Eq, PartialOrd, Hash)]
#[repr(u8)]
enum LzmaState {
    #[default]
    LitLit = 0,
    ///TODO
    MatchLitLit,
    ///TODO
    RepLitLit,
    ///TODO
    ShortRepLitLit,
    ///TODO
    MatchLit,
    ///TODO
    RepLit,
    ///TODO
    ShortRepLit,
    ///TODO
    LitMatch,
    ///TODO
    LitLongRep,
    ///TODO
    LitShortRep,
    ///TODO
    NonLitMatch,
    ///TODO
    NonLitRep,
}

impl LzmaState {
    /// numeric value of the state, used in some computations.
    const fn num(self) -> usize {
        match self {
            Self::LitLit => 0,
            Self::MatchLitLit => 1,
            Self::RepLitLit => 2,
            Self::ShortRepLitLit => 3,
            Self::MatchLit => 4,
            Self::RepLit => 5,
            Self::ShortRepLit => 6,
            Self::LitMatch => 7,
            Self::LitLongRep => 8,
            Self::LitShortRep => 9,
            Self::NonLitMatch => 10,
            Self::NonLitRep => 11,
        }
    }

    /// State transition
    #[allow(clippy::match_same_arms)]
    const fn u32_literal(self) -> Self {
        match self {
            Self::LitLit => Self::LitLit,
            Self::MatchLitLit => Self::LitLit,
            Self::RepLitLit => Self::LitLit,
            Self::ShortRepLitLit => Self::LitLit,
            Self::MatchLit => Self::MatchLitLit,
            Self::RepLit => Self::RepLitLit,
            Self::ShortRepLit => Self::ShortRepLitLit,
            Self::LitMatch => Self::MatchLit,
            Self::LitLongRep => Self::RepLit,
            Self::LitShortRep => Self::ShortRepLit,
            Self::NonLitMatch => Self::MatchLit,
            Self::NonLitRep => Self::RepLit,
        }
    }

    /// State transition
    #[allow(clippy::match_same_arms)]
    const fn u32_match(self) -> Self {
        match self {
            Self::LitLit => Self::LitMatch,
            Self::MatchLitLit => Self::LitMatch,
            Self::RepLitLit => Self::LitMatch,
            Self::ShortRepLitLit => Self::LitMatch,
            Self::MatchLit => Self::LitMatch,
            Self::RepLit => Self::LitMatch,
            Self::ShortRepLit => Self::LitMatch,
            Self::LitMatch => Self::NonLitMatch,
            Self::LitLongRep => Self::NonLitMatch,
            Self::LitShortRep => Self::NonLitMatch,
            Self::NonLitMatch => Self::NonLitMatch,
            Self::NonLitRep => Self::NonLitMatch,
        }
    }

    /// State transition
    #[allow(clippy::match_same_arms)]
    const fn u32_long_rep(self) -> Self {
        match self {
            Self::LitLit => Self::LitLongRep,
            Self::MatchLitLit => Self::LitLongRep,
            Self::RepLitLit => Self::LitLongRep,
            Self::ShortRepLitLit => Self::LitLongRep,
            Self::MatchLit => Self::LitLongRep,
            Self::RepLit => Self::LitLongRep,
            Self::ShortRepLit => Self::LitLongRep,
            Self::LitMatch => Self::NonLitRep,
            Self::LitLongRep => Self::NonLitRep,
            Self::LitShortRep => Self::NonLitRep,
            Self::NonLitMatch => Self::NonLitRep,
            Self::NonLitRep => Self::NonLitRep,
        }
    }

    /// State transition
    #[allow(clippy::match_same_arms)]
    const fn u32_short_rep(self) -> Self {
        match self {
            Self::LitLit => Self::LitShortRep,
            Self::MatchLitLit => Self::LitShortRep,
            Self::RepLitLit => Self::LitShortRep,
            Self::ShortRepLitLit => Self::LitShortRep,
            Self::MatchLit => Self::LitShortRep,
            Self::RepLit => Self::LitShortRep,
            Self::ShortRepLit => Self::LitShortRep,
            Self::LitMatch => Self::NonLitRep,
            Self::LitLongRep => Self::NonLitRep,
            Self::LitShortRep => Self::NonLitRep,
            Self::NonLitMatch => Self::NonLitRep,
            Self::NonLitRep => Self::NonLitRep,
        }
    }

    /// is the state literal.
    const fn u32_is_literal(self) -> bool {
        matches!(
            self,
            Self::LitLit
                | Self::MatchLitLit
                | Self::RepLitLit
                | Self::ShortRepLitLit
                | Self::MatchLit
                | Self::RepLit
                | Self::ShortRepLit
        )
    }
}

/// State of the lzma stream, what do we expect next in the stream?
#[derive(Clone, Debug, Copy, Default, Ord, PartialEq, Eq, PartialOrd, Hash)]
#[repr(u8)]
enum LzmaStreamState {
    /// LZMA2 control byte
    ///
    /// Exact values:
    ///   0x00   End marker
    ///   0x01   Dictionary reset followed by
    ///          an uncompressed chunk
    ///   0x02   Uncompressed chunk (no dictionary reset)
    ///
    /// Highest three bits (s->control & 0xE0):
    ///   0xE0   Dictionary reset, new properties and state
    ///          reset, followed by LZMA compressed chunk
    ///   0xC0   New properties and state reset, followed
    ///          by LZMA compressed chunk (no dictionary
    ///          reset)
    ///   0xA0   State reset using old properties,
    ///          followed by LZMA compressed chunk (no
    ///          dictionary reset)
    ///   0x80   LZMA chunk (no dictionary or state reset)
    ///
    /// For LZMA compressed chunks, the lowest five bits
    /// (s->control & 1F) are the highest bits of the
    /// uncompressed size (bits 16-20).
    ///
    /// A new LZMA2 stream must begin with a dictionary
    /// reset. The first LZMA chunk must set new
    /// properties and reset the LZMA state.
    ///
    /// Values that don't match anything described above
    /// are invalid and we return an error.
    #[default]
    Control = 0,
    Uncompressed1 = 1,
    Uncompressed2 = 2,
    Compressed0 = 3,
    Compressed1 = 4,
    Properties = 5,
    LzmaPrepare = 6,
    LzmaRun = 7,
    Copy = 8,
    /// LZMA2 end marker was consumed.
    Finished = 9,
}

/// Buffer used by the range decoder.
/// This buffer is borrowed from some other buffer.
pub struct RcBuf<'a> {
    /// the buffer slice
    input: &'a [u8],
    /// position in the input slice
    in_pos: usize,
    /// maximum position in the input slice. TODO this can be refactored away using slices later...
    in_limit: usize,
}

impl RcBuf<'_> {
    /// returns the next byte from the `RcBuf`.
    const fn next(&mut self) -> u8 {
        let r = self.input[self.in_pos];
        self.in_pos += 1;
        r
    }

    /// Return true if there may not be enough input for the next decoding loop.
    const fn limit_exceeded(&self) -> bool {
        self.in_pos > self.in_limit
    }
}

/// Range Decoder
#[derive(Clone, Debug)]
pub struct RcDecoder {
    ///TODO
    range: u32,
    /// TODO
    code: u32,
    /// Number of bytes we still have to read from the buffer to be able to intalize the range decoder.
    init_bytes_left: u8, //valid values: 0->5
}

impl RcDecoder {
    /// Constructor
    const fn new() -> Self {
        Self {
            range: 0,
            code: 0,
            init_bytes_left: 0,
        }
    }

    /// reset the Rc back to its initial state.
    const fn reset(&mut self) {
        self.range = u32::MAX;
        self.code = 0;
        self.init_bytes_left = 5;
    }

    /// initializes the rc.
    fn read_init(&mut self, b: &mut XzInOutBuffer) -> bool {
        while self.init_bytes_left > 0 {
            if b.input_pos == b.in_size() {
                return false;
            }
            let x = u32::from(b.input_slice()[0]);
            b.input_pos = b.input_pos.wrapping_add(1);
            self.code = (self.code << 8i32).wrapping_add(x);
            self.init_bytes_left -= 1;
        }
        true
    }

    /// Is the rc decoder finished?
    const fn is_finished(&self) -> bool {
        self.code == 0
    }

    /// Read the next input byte if needed.
    fn normalize(&mut self, rcb: &mut RcBuf) {
        if self.range >= (1 << 24) {
            return;
        }
        self.range <<= 8;
        self.code = (self.code << 8) | u32::from(rcb.next());
    }

    /// Decode one bit.
    fn rc_bit(&mut self, prob: &mut u16, rcb: &mut RcBuf) -> bool {
        self.normalize(rcb);
        let p = u32::from(*prob);
        // Info from Mr Collin: "The 16-bit probability variables stay within the range [31, 2017]"
        debug_assert!(p >= 31);
        debug_assert!(p <= 2017);

        // as long as the debug_assert's are true, this cannot wrap.
        // (4,294,967,295 >> 11) * 2017 = 4,229,953,567 which is less than 4,294,967,295 (u32::MAX)
        let bound = (self.range >> 11) * p;

        if self.code < bound {
            self.range = bound;
            *prob = clamp_u32_to_u16(p + ((((1u32) << 11) - p) >> 5));
            return true;
        }

        //TODO unsure if wrapping needed.
        self.range = self.range.wrapping_sub(bound);
        self.code -= bound;

        *prob = clamp_u32_to_u16(p - (p >> 5));
        false
    }

    /// Decode a bittree starting from the most significant bit.
    fn rc_bittree(&mut self, probs: &mut [u16], rcb: &mut RcBuf) -> u32 {
        let mut symbol = 1;
        loop {
            if self.rc_bit(&mut probs[symbol], rcb) {
                symbol <<= 1;
            } else {
                symbol = (symbol << 1) | 1;
            }

            if symbol >= probs.len() {
                return clamp_us_to_u32(symbol);
            }
        }
    }

    /// Decode a bittree starting from the least significant bit.
    fn bittree_reverse(
        &mut self,
        probs: &mut [u16],
        mut dest: u32,
        limit: u32,
        rcb: &mut RcBuf,
    ) -> u32 {
        //Info: Control flow shows that limit is always at least 1 and never more than 13.
        debug_assert!(limit > 0);
        let mut symbol = 1u32;
        for i in 0..limit {
            if self.rc_bit(&mut probs[symbol as usize], rcb) {
                symbol <<= 1;
                continue;
            }

            symbol = (symbol << 1) | 1;
            dest = dest.wrapping_add(1 << i);
        }

        dest
    }

    /// Decode direct bits (fixed fifty-fifty probability)
    fn direct(&mut self, mut dest: u32, limit: u32, rcb: &mut RcBuf) -> u32 {
        //INFO: Control flow shows that the smallest possible value this is actually called with is limit=10.
        debug_assert!(limit > 0);
        for _ in 0..limit {
            self.normalize(rcb);
            self.range >>= 1;
            let new_code = self.code.wrapping_sub(self.range);
            if new_code & (1 << 31) != 0 {
                dest <<= 1;
                continue;
            }
            self.code = new_code;
            dest = (dest << 1) | 1;
        }

        dest
    }
}

/// Holds the actual buffer allocation.
#[derive(Debug)]
enum XzDictBufferAllocation<'a> {
    /// dynamic allocation in the heap. We use Vec for this.
    #[cfg(feature = "alloc")]
    Alloc(vec::Vec<u8>, usize),
    /// fixed size allocation.
    Fixed(&'a mut [u8]),
}

impl Deref for XzDictBufferAllocation<'_> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        match self {
            #[cfg(feature = "alloc")]
            XzDictBufferAllocation::Alloc(alc, _) => alc.as_slice(),
            XzDictBufferAllocation::Fixed(fix) => fix,
        }
    }
}

impl DerefMut for XzDictBufferAllocation<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            #[cfg(feature = "alloc")]
            XzDictBufferAllocation::Alloc(alc, _) => alc.as_mut_slice(),
            XzDictBufferAllocation::Fixed(fix) => fix,
        }
    }
}

/// Holds the buffer and some dictionary state.
#[derive(Debug)]
pub struct XzDictBuffer<'a> {
    /// the actual buffer allocation
    buffer: XzDictBufferAllocation<'a>,
    ///TODO CONSOLIDATE
    dict_start: usize,
    ///TODO CONSOLIDATE
    dict_pos: usize,
    ///TODO CONSOLIDATE
    dict_size: usize,
    ///TODO CONSOLIDATE
    dict_full: usize,
    ///TODO CONSOLIDATE
    dict_limit: usize,
}

impl<'a> XzDictBuffer<'a> {
    /// Constructor
    const fn new(buffer: XzDictBufferAllocation<'a>) -> Self {
        Self {
            buffer,
            dict_start: 0,
            dict_pos: 0,
            dict_size: 0,
            dict_full: 0,
            dict_limit: 0,
        }
    }

    /// returns the maximum possible size of the dictionary. This is controlled by the use of the library.
    #[allow(clippy::missing_const_for_fn)] //Without alloc clippy will assume this can be const.
    fn max_size(&self) -> usize {
        match &self.buffer {
            #[cfg(feature = "alloc")]
            XzDictBufferAllocation::Alloc(cur, max) => cur.len().max(*max),
            XzDictBufferAllocation::Fixed(buf) => buf.len(),
        }
    }

    //fn allocated(&self) -> usize {
    //    match &self.buffer {
    //        #[cfg(feature = "alloc")]
    //        XzDictBuffer::Alloc(cur, _) => cur.len(),
    //        XzDictBuffer::Fixed(buf) => buf.len(),
    //    }
    //}

    /// mutable dictionary buffer limited to `dict_size`.
    fn buffer_mut(&mut self) -> &mut [u8] {
        &mut self.buffer.deref_mut()[..self.dict_size]
    }

    /// gets a byte from the dictionary at the given index.
    fn buffer_get(&self, index: usize) -> u8 {
        debug_assert!(index < self.dict_size);
        self.buffer[index]
    }

    /// pushes a byte into the dictionary buffer at the current position.
    fn push(&mut self, byte: u8) {
        debug_assert!(self.dict_pos < self.dict_size);
        self.buffer[self.dict_pos] = byte;
        self.dict_pos += 1;
    }

    /// immutable dictionary buffer limited to `dict_size`.
    fn buffer(&self) -> &[u8] {
        //Optimization that saves about 0.2ms in b2
        //unsafe {core::slice::from_raw_parts(self.buffer.as_ptr(), self.dict_size)}
        &self.buffer[..self.dict_size]
    }

    /// Ensures that at least `needed_size` bytes are allocated in the dictionary or return an error
    /// if this is not possible due to memory limits or if we use a fixed size dictionary that is smaller.
    #[allow(clippy::missing_const_for_fn)] //not possible const with alloc feature, without alloc clippy will tell that this "could" be const.
    fn alloc_dict(&mut self, needed_size: usize) -> Result<(), XzError> {
        match &mut self.buffer {
            #[cfg(feature = "alloc")]
            XzDictBufferAllocation::Alloc(buf, max) => {
                if buf.len() >= needed_size {
                    self.dict_size = needed_size;
                    return Ok(());
                }

                if needed_size > *max {
                    return Err(XzError::DictionaryTooLarge(needed_size as u64));
                }

                *buf = vec![0u8; needed_size];
                self.dict_size = needed_size;
                Ok(())
            }
            XzDictBufferAllocation::Fixed(sl) => {
                if sl.len() < needed_size {
                    return Err(XzError::DictionaryTooLarge(needed_size as u64));
                }
                self.dict_size = needed_size;
                Ok(())
            }
        }
    }

    /// repeats a lzma rep in the dictionary.
    pub fn dict_repeat(&mut self, rep0: usize, len: usize) -> Result<usize, XzError> {
        if rep0 >= self.dict_full() || rep0 >= self.dict_size() {
            return Err(XzError::DictionaryOverflow);
        }

        let count = self.get_dict_remaining_until_limit().min(len);

        //TODO further examine wrapping logic
        let mut back = self.dict_pos().wrapping_sub(rep0).wrapping_sub(1);

        if rep0 >= self.dict_pos() {
            //TODO UNREACHED
            back = back.wrapping_add(self.buffer().len());
        }

        //TODO unfuck this loop
        let mut remaining = count;
        loop {
            self.push(self.buffer()[back]);
            back += 1;

            if back == self.buffer().len() {
                //TODO unreached.
                back = 0;
            }
            remaining = remaining.wrapping_sub(1);
            if remaining == 0 {
                break;
            }
        }

        if (self.dict_full()) < self.dict_pos() {
            self.set_dict_full();
        }
        Ok(count)
    }

    /// Copies some uncompressed bytes from the dictionary to the out buffer.
    fn dict_uncompressed(&mut self, b: &mut XzInOutBuffer, mut left: usize) -> usize {
        while left > 0 && b.input_pos < b.in_size() {
            let remaining_out = b.output_remaining();
            if remaining_out == 0 {
                break;
            }
            let remaining_input = b.input_remaining();
            let remaining_dict = self.buffer().len().wrapping_sub(self.dict_pos()); //TODO wrapping needed?
            debug_assert!(remaining_dict > 0);

            let copy_size = remaining_input
                .min(remaining_out)
                .min(remaining_dict)
                .min(left);

            left -= copy_size;

            let buf_pos = self.dict_pos();
            let target = &mut self.buffer_mut()[buf_pos..(buf_pos + copy_size)];
            let src = &b.input_slice()[..copy_size];
            target.copy_from_slice(src);

            self.set_dict_pos(self.dict_pos().wrapping_add(copy_size));
            if (self.dict_full()) < self.dict_pos() {
                self.set_dict_full();
            }

            if self.dict_pos() == self.buffer().len() {
                self.set_dict_pos(0);
            }

            b.copy_in_to_out(copy_size);
            self.set_dict_start();
        }

        left
    }

    /// returns the byte for the given lzma dist.
    /// # Panics
    /// may panic if dist is not a valid lzma dist
    fn dict_get(&self, dist: usize) -> u8 {
        debug_assert!(self.dict_size() > 0);
        if self.dict_full() == 0 {
            return 0;
        }

        if dist >= self.dict_pos() {
            // Info: Concerning underflow.
            // This term is (A) - (B),
            // A cannot wrap internally because dict_size of 0 is not valid.
            // B cannot wrap internally due to the IF above ensuring that it doesn't.
            // A - B must produce a valid index that is less than A+1
            // The only way for this to hold true with wrapping is if B is usize::MAX.
            // The only way that happen is if dist is usize::MAX and dict_pos is 0.
            // This is impossible on 64 bit targets considering dist was numerically bounded to u32::MAX.
            // Conclusion is therefore that for a valid dist underflow is not possible, even on 32 bit targets.

            // Note: This may underflow for invalid dist in debug mode!
            let offset = (self.dict_size() - 1) - (dist - self.dict_pos());
            // Note: This may panic for invalid dist in release mode!
            return self.buffer_get(offset);
        }

        //This cant underflow.
        let offset = self.dict_pos() - (dist) - 1;
        self.buffer_get(offset)
    }

    /// Writes some compressed bytes to the output buffer.
    fn dict_flush(&mut self, b: &mut XzInOutBuffer) -> usize {
        let copy_size = self.dict_pos().wrapping_sub(self.dict_start());

        if self.dict_pos() == self.dict_size() {
            //TODO unreached
            self.set_dict_pos(0);
        }

        let dict_start = self.dict_start();
        let source = &self.buffer()[dict_start..dict_start + copy_size];
        b.copy_to_output(source);

        self.set_dict_start();
        copy_size
    }

    /// sets the dictionary size.
    const fn dict_size(&self) -> usize {
        self.dict_size
    }

    /// gets the dictionary position
    const fn dict_pos(&self) -> usize {
        debug_assert!(self.dict_pos <= self.dict_size);
        self.dict_pos
    }

    /// sets the dictionary position.
    const fn set_dict_pos(&mut self, pos: usize) {
        debug_assert!(pos <= self.dict_size);
        self.dict_pos = pos;
    }

    /// sets the dictionary start to the current position.
    const fn set_dict_start(&mut self) {
        debug_assert!(self.dict_pos <= self.dict_size);
        self.dict_start = self.dict_pos;
    }

    /// sets the dictionary to be full at the current position.
    const fn set_dict_full(&mut self) {
        debug_assert!(self.dict_pos <= self.dict_size);
        self.dict_full = self.dict_pos;
    }

    /// returns the dictionary fill count.
    const fn dict_full(&self) -> usize {
        debug_assert!(self.dict_full <= self.dict_size);
        self.dict_full
    }

    /// returns the dictionary start.
    const fn dict_start(&self) -> usize {
        debug_assert!(self.dict_start <= self.dict_size);
        self.dict_start
    }

    /// returns the dictionary limit
    const fn get_dict_limit(&self) -> usize {
        debug_assert!(self.dict_limit <= self.dict_size);
        self.dict_limit
    }

    /// returns the amount of bytes remaining until the dictionary limit.
    const fn get_dict_remaining_until_limit(&self) -> usize {
        debug_assert!(self.dict_limit <= self.dict_size);
        debug_assert!(self.dict_pos <= self.dict_limit);
        self.dict_limit - self.dict_pos
    }

    /// returns true if the dictionary still has space.
    const fn dict_has_space(&self) -> bool {
        self.dict_pos() < self.get_dict_limit()
    }

    /// inserts a byte into the dictionary.
    fn dict_put(&mut self, byte: u8) {
        self.push(byte);
        if self.dict_full() < self.dict_pos() {
            self.set_dict_full();
        }
    }

    /// resets the dictionary counters to its inital state.
    const fn dict_reset(&mut self) {
        self.set_dict_pos(0);
        self.set_dict_start();
        self.set_dict_full();
        self.dict_limit = 0;
    }

    /// sets the dict limit to either dict size or `dict_pos+out_max` whichever is smaller.
    const fn dict_limit(&mut self, out_max: usize) {
        let remaining = self.dict_size - self.dict_pos;

        if remaining <= out_max {
            //TODO unreached.
            self.dict_limit = self.dict_size;
            return;
        }

        self.dict_limit = self.dict_pos + out_max;
    }
}

/// State of the xz decoder state machine.
#[derive(Debug)]
#[repr(u8)]
enum XzDecoderState {
    /// the initial state of the decoder
    StreamHeader = 0,
    /// TODO
    StreamStart,
    /// TODO
    BlockHeader,
    /// TODO
    BlockUncompress,
    /// TODO
    BlockPadding,
    /// TODO
    BlockCheck,
    /// TODO
    Index,
    /// TODO
    IndexPadding,
    /// TODO
    IndexCrc32,
    /// TODO
    StreamFooter,
}

#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum XzCheckType {
    #[default]
    None = 0,
    Crc32,
    #[cfg(feature = "sha256")]
    Sha256,
    #[cfg(feature = "crc64")]
    Crc64,
}

impl Display for XzCheckType {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            #[cfg(feature = "sha256")]
            Self::Sha256 => f.write_str("sha256"),
            #[cfg(feature = "crc64")]
            Self::Crc64 => f.write_str("crc64"),
            Self::Crc32 => f.write_str("crc32"),
            Self::None => f.write_str("none"),
        }
    }
}

impl XzCheckType {
    /// returns the size of the check in bytes.
    const fn check_size(self) -> usize {
        match self {
            #[cfg(feature = "sha256")]
            Self::Sha256 => 32,
            #[cfg(feature = "crc64")]
            Self::Crc64 => 8,
            Self::Crc32 => 4,
            Self::None => 0,
        }
    }
}

impl PartialEq<u8> for XzCheckType {
    fn eq(&self, other: &u8) -> bool {
        *other == (*self).into()
    }
}

impl From<XzCheckType> for u8 {
    fn from(value: XzCheckType) -> Self {
        match value {
            #[cfg(feature = "sha256")]
            XzCheckType::Sha256 => 10,
            #[cfg(feature = "crc64")]
            XzCheckType::Crc64 => 4,
            XzCheckType::Crc32 => 1,
            XzCheckType::None => 0,
        }
    }
}

impl TryFrom<u8> for XzCheckType {
    type Error = XzError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::None,
            1 => Self::Crc32,
            #[cfg(feature = "crc64")]
            4 => Self::Crc64,
            #[cfg(not(feature = "crc64"))]
            4 => return Err(XzError::Crc64NotSupported),
            #[cfg(feature = "sha256")]
            10 => Self::Sha256,
            #[cfg(not(feature = "sha256"))]
            10 => return Err(XzError::Sha256NotSupported),
            _ => return Err(XzError::UnsupportedCheckType(u32::from(value))),
        })
    }
}

#[derive(Debug)]
pub enum XzNextBlockResult {
    NeedMoreData(usize, usize),
    EndOfStream(usize, usize),
}

impl XzNextBlockResult {
    #[must_use]
    pub const fn input_consumed(&self) -> usize {
        match self {
            Self::EndOfStream(inp, _) | Self::NeedMoreData(inp, _) => *inp,
        }
    }

    #[must_use]
    pub const fn output_produced(&self) -> usize {
        match self {
            Self::NeedMoreData(_, out) | Self::EndOfStream(_, out) => *out,
        }
    }

    #[must_use]
    pub const fn made_progress(&self) -> bool {
        self.input_consumed() != 0 || self.output_produced() != 0
    }

    #[must_use]
    pub const fn is_end_of_stream(&self) -> bool {
        matches!(self, Self::EndOfStream(_, _))
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[non_exhaustive]
pub enum XzError {
    NeedsReset,
    NeedsLargerInputBuffer,
    CorruptedData,
    CorruptedDataInLzma,
    DictionaryOverflow,
    LzmaPropertiesTooLarge,
    LzmaPropertiesInvalid,
    LzmaPropertiesMissing,
    LzmaDictionaryResetExcepted,
    MoreDataInBlockBodyThanHeaderIndicated,
    LessDataInBlockBodyThanHeaderIndicated,
    CorruptedDataInBlockIndex,
    BlockHeaderTooSmall,
    CorruptedCompressedLengthVliInBlockHeader,
    CorruptedUncompressedLengthVliInBlockHeader,
    UnsupportedStreamHeaderOption,
    UnsupportedBlockHeaderOption,
    #[cfg(not(feature = "bcj"))]
    BcjFilterNotSupported,
    #[cfg(not(feature = "crc64"))]
    Crc64NotSupported,
    #[cfg(not(feature = "sha256"))]
    Sha256NotSupported,
    UnsupportedLzmaProperties(u32),
    DictionaryTooLarge(u64),
    UnsupportedCheckType(u32),
    /// Will never be returned anymore because
    /// BCJ filters with offset are now supported. Will be removed in the next major release
    #[cfg(feature = "bcj")]
    #[deprecated]
    BcjFilterWithOffsetNotSupported,
    #[cfg(feature = "bcj")]
    UnsupportedBcjFilter(u32),
    #[cfg(not(feature = "delta"))]
    DeltaFilterUnsupported,

    ContentCrc32Mismatch(u32, u32), //Actual, Excepted
    IndexCrc32Mismatch(u32, u32),   //Actual, Excepted
    #[cfg(feature = "crc64")]
    ContentCrc64Mismatch(u64, u64), //Actual, Excepted

    #[cfg(feature = "sha256")]
    ContentSha256Mismatch([u8; 32], [u8; 32]), //Actual, Expected

    StreamHeaderMagicNumberMismatch,
    StreamHeaderCrc32Mismatch(u32, u32), //Actual, Expected

    BlockHeaderCrc32Mismatch(u32, u32), //Actual, Expected

    FooterMagicNumberMismatch,
    FooterCheckTypeMismatch(u32, XzCheckType), //Actual, Expected
    FooterCrc32Mismatch(u32, u32),             //Actual, Expected
    FooterDecoderIndexMismatch(u64, u64),      //Actual, Expected
}

impl Display for XzError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NeedsReset => f.write_str("NeedsReset"),
            Self::NeedsLargerInputBuffer => f.write_str("NeedsLargerInputBuffer"),
            Self::CorruptedData => f.write_str("CorruptedData"),
            Self::CorruptedDataInLzma => f.write_str("CorruptedDataInLzma"),
            Self::DictionaryOverflow => f.write_str("DictionaryOverflow"),
            Self::LzmaPropertiesTooLarge => f.write_str("LzmaPropertiesTooLarge"),
            Self::LzmaPropertiesInvalid => f.write_str("LzmaPropertiesInvalid"),
            Self::LzmaPropertiesMissing => f.write_str("LzmaPropertiesMissing"),
            Self::LzmaDictionaryResetExcepted => f.write_str("LzmaDictionaryResetExcepted"),
            Self::MoreDataInBlockBodyThanHeaderIndicated => {
                f.write_str("MoreDataInBlockBodyThanHeaderIndicated")
            }
            Self::LessDataInBlockBodyThanHeaderIndicated => {
                f.write_str("LessDataInBlockBodyThanHeaderIndicated")
            }
            Self::CorruptedDataInBlockIndex => f.write_str("CorruptedDataInBlockIndex"),
            Self::BlockHeaderTooSmall => f.write_str("CorruptedDataInBlockHeader"),
            Self::UnsupportedStreamHeaderOption => f.write_str("UnsupportedOption"),
            #[cfg(not(feature = "bcj"))]
            Self::BcjFilterNotSupported => f.write_str("BcjFilterNotSupported"),
            #[cfg(not(feature = "crc64"))]
            Self::Crc64NotSupported => f.write_str("Crc64NotSupported"),
            #[cfg(not(feature = "sha256"))]
            Self::Sha256NotSupported => f.write_str("Sha256NotSupported"),
            Self::UnsupportedLzmaProperties(prp) => {
                f.write_fmt(format_args!("UnsupportedLzmaProperties(property={prp})"))
            }
            Self::DictionaryTooLarge(size) => {
                f.write_fmt(format_args!("UnsupportedLzmaProperties(size={size} bytes)"))
            }
            Self::UnsupportedCheckType(typ) => {
                f.write_fmt(format_args!("UnsupportedCheckType(type={typ})"))
            }
            #[cfg(feature = "bcj")]
            #[expect(deprecated)]
            Self::BcjFilterWithOffsetNotSupported => f.write_str("BcjFilterWithOffsetNotSupported"),
            #[cfg(feature = "bcj")]
            Self::UnsupportedBcjFilter(flt) => {
                f.write_fmt(format_args!("UnsupportedBcjFilter(type={flt})"))
            }
            #[cfg(not(feature = "delta"))]
            Self::DeltaFilterUnsupported => f.write_str("DeltaFilterUnsupported"),
            Self::ContentCrc32Mismatch(actual, expected) => f.write_fmt(format_args!(
                "ContentCrc32Mismatch(actual={actual}, expected={expected})"
            )),
            Self::IndexCrc32Mismatch(actual, expected) => f.write_fmt(format_args!(
                "IndexCrc32Mismatch(actual={actual}, expected={expected})"
            )),
            #[cfg(feature = "crc64")]
            Self::ContentCrc64Mismatch(actual, expected) => f.write_fmt(format_args!(
                "ContentCrc64Mismatch(actual={actual}, expected={expected})"
            )),
            #[cfg(feature = "sha256")]
            Self::ContentSha256Mismatch(actual, expected) => f.write_fmt(format_args!(
                "ContentSha256Mismatch(actual={actual:?}, expected={expected:?})"
            )),
            Self::StreamHeaderMagicNumberMismatch => f.write_str("StreamHeaderMagicNumberMismatch"),
            Self::StreamHeaderCrc32Mismatch(actual, expected) => f.write_fmt(format_args!(
                "StreamHeaderCrc32Mismatch(actual={actual}, expected={expected})"
            )),
            Self::BlockHeaderCrc32Mismatch(actual, expected) => f.write_fmt(format_args!(
                "BlockHeaderCrc32Mismatch(actual={actual}, expected={expected})"
            )),
            Self::FooterMagicNumberMismatch => f.write_str("FooterMagicNumberMismatch"),
            Self::FooterCheckTypeMismatch(actual, expected) => f.write_fmt(format_args!(
                "FooterCheckTypeMismatch(actual={actual}, expected={expected})"
            )),
            Self::FooterCrc32Mismatch(actual, expected) => f.write_fmt(format_args!(
                "FooterCrc32Mismatch(actual={actual}, expected={expected})"
            )),
            Self::FooterDecoderIndexMismatch(actual, expected) => f.write_fmt(format_args!(
                "FooterCrc32Mismatch(actual={actual}, expected={expected})"
            )),
            Self::CorruptedCompressedLengthVliInBlockHeader => {
                f.write_str("CorruptedCompressedLengthVliInBlockHeader")
            }
            Self::CorruptedUncompressedLengthVliInBlockHeader => {
                f.write_str("CorruptedUncompressedLengthVliInBlockHeader")
            }
            Self::UnsupportedBlockHeaderOption => f.write_str("UnsupportedBlockHeaderOption"),
        }
    }
}

/// Xz decoder that can be placed in static memory.
/// The Xz decoder is not a small data structure,
/// so putting it in static memory may be a good idea
/// if stack space is not guaranteed to be available.
///
/// Note: Allocating this decoder on the stack will almost always blow the stack!
/// Use `XzDecoder` or `XzReader` instead!
#[derive(Debug)]
pub struct XzStaticDecoder<const T: usize> {
    /// the fixed size dictionary
    dict_buf: [u8; T],

    /// first index in the dict.
    dict_start: usize,

    /// current position in the dict.
    dict_pos: usize,

    /// required size of the dictionary based on the xz header.
    dict_size: usize,

    /// mutable counter how many bytes in the `dict_buf` are filled.
    dict_full: usize,

    /// set if dictionary was limited to a certain size.
    dict_limit: usize,

    /// The rest of the decoder.
    inner: XzInnerDecoder,
}

impl<const T: usize> Default for XzStaticDecoder<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const T: usize> XzStaticDecoder<T> {
    /// Static constructor of the `XzDecoder`.
    /// # Panics
    /// if T is less than `DICT_SIZE_MIN`
    #[must_use]
    pub const fn new() -> Self {
        assert!(T >= crate::DICT_SIZE_MIN, "Dictionary too small");

        Self {
            dict_buf: [0; T],
            dict_start: 0,
            dict_pos: 0,
            dict_size: 0,
            dict_full: 0,
            dict_limit: 0,
            inner: XzInnerDecoder::new(),
        }
    }

    /// This function inits or resets a static `XzDecoder` at a given location in memory.
    /// The returned pointers lifetime is equal to that of the input address.
    /// Its numeric value is identical.
    ///
    /// This function will initialize the memory to a valid state for decoding.
    /// This function does not read the memory.
    ///
    /// This function can also be used to "reset" an existing static Decoder
    /// if it already exists at the given address.
    ///
    /// # Arguments
    /// address: raw address where the decoder should be placed.
    /// size: size of the memory in bytes at address that can be used.
    ///
    /// # Panics
    /// if size is less than sizeof `XzStaticDecoder<T>`
    ///
    /// # Safety
    /// address must point to a valid address with sufficient lifetime and at least size bytes in size.
    /// alignment of address must be suitable for `XzStaticDecoder<T>`
    ///
    #[cfg(not(feature = "no_unsafe"))]
    pub const unsafe fn init_or_reset_at_address(
        address: *mut core::ffi::c_void,
        size: usize,
    ) -> *mut Self {
        assert!(
            size >= size_of::<Self>(),
            "XzStaticDecoder::init_or_reset_at_address() given address/buffer is too small"
        );

        //Not const.
        //assert_eq!(address.align_offset(align_of::<Self>()), 0, "XzStaticDecoder::init_or_reset_at_address() given address/buffer is not aligned properly");

        unsafe {
            let decoder_ptr: *mut Self = address.cast();
            decoder_ptr.write_bytes(0, 1);

            //We cannot guarantee that a 0 alloc is valid for this field as it contains 1 member residing in a different crate.
            #[cfg(feature = "sha256")]
            core::ptr::addr_of_mut!((*decoder_ptr).inner.sha256).write(XzSha256::new());

            decoder_ptr.as_mut().unwrap_unchecked().reset();

            decoder_ptr
        }
    }

    /// Processes the next block of input data and possibly produces output.
    ///
    /// The recommended minimum size of the output buffer is 256 bytes, the larger, the better.
    /// The input buffer should also be at least that size, the larger, the better.
    /// However, obviously, at the end of the stream this is not possible/needed.
    ///
    /// The decoder can be reused to decode multiple xz streams if "reset"
    /// is called after `XzNextBlockResult::EndOfStream` was returned.
    /// Failure to do so will lead to Err `XzError::NeedsReset` upon future calls.
    ///
    /// This implementation will NOT parse the padding 0 bytes
    /// mentioned in the XZ documentation that occur between concatenated streams of two xz files.
    /// The caller will have to skip all 0 bytes between such streams.
    ///
    /// # Errors
    /// Most errors returned by this fn are fatal and the decoder must be reset afterward.
    /// If `decode` is called again when the decoder had a fatal error then it will cause an Err with `XzError::NeedsReset`.
    /// The only errors that are not fatal are:
    /// - `XzError::NeedsLargerInputBuffer`
    ///     - Input buffer does not contain enough data to make progress
    ///
    pub fn decode(
        &mut self,
        input_data: &[u8],
        output_data: &mut [u8],
    ) -> Result<XzNextBlockResult, XzError> {
        let mut dict_buf = self.dict_buf.as_mut_slice();
        if T > DICT_SIZE_MAX {
            dict_buf = &mut dict_buf[..DICT_SIZE_MAX];
        }

        let mut dict_buf_borrow = XzDictBuffer {
            buffer: XzDictBufferAllocation::Fixed(dict_buf),
            //TODO shared struct dict offsets which is just copy?
            dict_start: self.dict_start,
            dict_pos: self.dict_pos,
            dict_size: self.dict_size,
            dict_full: self.dict_full,
            dict_limit: self.dict_limit,
        };
        let result = self
            .inner
            .decode(input_data, output_data, &mut dict_buf_borrow);
        self.dict_pos = dict_buf_borrow.dict_pos;
        self.dict_size = dict_buf_borrow.dict_size;
        self.dict_start = dict_buf_borrow.dict_start;
        self.dict_full = dict_buf_borrow.dict_full;
        self.dict_limit = dict_buf_borrow.dict_limit;
        result
    }

    /// Returns whether a complete LZMA2 chunk has ended.
    ///
    /// When true, the decoder is before the next LZMA2 control byte
    /// and has no compressed input buffered internally.
    ///
    /// This does not indicate why the chunk ended, XZ stream
    /// completion or check validation, or that all filtered output
    /// has been returned.
    #[must_use]
    pub const fn is_lzma2_chunk_boundary(&self) -> bool {
        self.inner.is_lzma2_chunk_boundary()
    }

    /// Reset the decoder
    pub const fn reset(&mut self) {
        self.inner.reset();
        self.dict_pos = 0;
        self.dict_size = 0;
        self.dict_start = 0;
        self.dict_full = 0;
        self.dict_limit = 0;
    }
}

#[derive(Debug)]
pub struct XzDecoder<'a> {
    /// Dictionary buffer
    dictionary_buffer: XzDictBuffer<'a>,
    /// The rest of the decoder
    inner: XzInnerDecoder,
}

impl<'a> XzDecoder<'a> {
    /// Creates a new xz decoder that uses a fixed size dictionary.
    /// The content in the dict slice is irrelevant and will be overwritten.
    pub fn with_fixed_size_dict(mut dict: &'a mut [u8]) -> Self {
        if dict.len() > DICT_SIZE_MAX {
            dict = &mut dict[..DICT_SIZE_MAX];
        }

        Self {
            dictionary_buffer: XzDictBuffer::new(XzDictBufferAllocation::Fixed(dict)),
            inner: XzInnerDecoder::default(),
        }
    }

    #[cfg(feature = "alloc")]
    #[must_use]
    pub fn with_alloc_dict_size(initial_dict: usize, max_dict: usize) -> XzDecoder<'static> {
        Self::with_alloc_dict(vec![0; initial_dict.min(DICT_SIZE_MAX)], max_dict)
    }

    #[cfg(feature = "alloc")]
    #[must_use]
    pub fn with_alloc_dict(mut initial_dict: vec::Vec<u8>, max_dict: usize) -> XzDecoder<'static> {
        initial_dict.truncate(DICT_SIZE_MAX);

        XzDecoder {
            dictionary_buffer: XzDictBuffer::new(XzDictBufferAllocation::Alloc(
                initial_dict,
                max_dict.min(DICT_SIZE_MAX),
            )),
            inner: XzInnerDecoder::default(),
        }
    }

    /// This allocates a `XzDecoder` in heap.
    /// The default dictionary buffer of 8MB will be allocated additionally.
    /// 8MB is the default value used by lzma-utils to create
    /// .xz files if no other option is passed to the xz program.
    /// The maximum dictionary size the decoder will allocate (should the input file require it) is 3GB.
    #[cfg(feature = "alloc")]
    #[must_use]
    pub fn in_heap() -> Box<XzDecoder<'static>> {
        Self::in_heap_with_alloc_dict_size(crate::DICT_SIZE_PROFILE_7, DICT_SIZE_MAX)
    }

    #[cfg(feature = "alloc")]
    #[must_use]
    pub fn in_heap_with_alloc_dict_size(
        initial_dict: usize,
        max_dict: usize,
    ) -> Box<XzDecoder<'static>> {
        Self::in_heap_with_alloc_dict(vec![0; initial_dict.min(DICT_SIZE_MAX)], max_dict)
    }

    #[cfg(feature = "alloc")]
    #[cfg(feature = "no_unsafe")]
    #[must_use]
    pub fn in_heap_with_alloc_dict(
        mut initial_dict: vec::Vec<u8>,
        max_dict: usize,
    ) -> Box<XzDecoder<'static>> {
        initial_dict.truncate(DICT_SIZE_MAX);

        //This may blow the stack due to possible stack allocation of XzDecoder before it is moved to heap.
        //It needs a 32k-40k stack to succeed.
        let mut result = Box::new(XzDecoder::with_alloc_dict(
            initial_dict,
            max_dict.min(DICT_SIZE_MAX),
        ));
        result.reset();
        result
    }

    #[cfg(feature = "alloc")]
    #[cfg(not(feature = "no_unsafe"))]
    #[must_use]
    pub fn in_heap_with_alloc_dict(
        mut initial_dict: vec::Vec<u8>,
        max_dict: usize,
    ) -> Box<XzDecoder<'static>> {
        use core::ptr::addr_of_mut;

        if initial_dict.len() > DICT_SIZE_MAX {
            initial_dict.truncate(DICT_SIZE_MAX);
        }

        //The decoder is big. It will blow the stack on small stack sizes.
        //This fn doesn't stack allocate it.

        let mut decoder = unsafe {
            let mut uninit = Box::<XzDecoder>::new_uninit();
            let ptr = uninit.as_mut_ptr();
            // Zero the memory.
            addr_of_mut!((*ptr).inner).write_bytes(0, 1);

            //We cannot guarantee that a 0 alloc is valid for this field as it contains 1 member residing in a different crate.
            #[cfg(feature = "sha256")]
            addr_of_mut!((*ptr).inner.sha256).write(XzSha256::new());

            // This field is not a valid 0 alloc.
            addr_of_mut!((*ptr).dictionary_buffer).write(XzDictBuffer::new(
                XzDictBufferAllocation::Alloc(initial_dict, max_dict.min(DICT_SIZE_MAX)),
            ));
            uninit.assume_init()
        };
        // Actually init all fields properly.
        decoder.reset();
        decoder
    }

    /// Processes the next block of input data and possibly produces output.
    ///
    /// The recommended minimum size of the output buffer is 256 bytes, the larger, the better.
    /// The input buffer should also be at least that size, the larger, the better.
    /// However, at the end of the stream this is not possible/needed.
    ///
    /// The decoder can be reused to decode multiple xz streams if "reset"
    /// is called after `XzNextBlockResult::EndOfStream` was returned.
    /// Failure to do so will lead to Err `XzError::NeedsReset` upon future calls.
    ///
    /// This implementation will NOT parse the padding 0 bytes
    /// mentioned in the XZ documentation that occur between concatenated streams of two xz files.
    /// The caller will have to skip all 0 bytes between such streams.
    ///
    /// # Errors
    /// Most errors returned by this fn are fatal, and the decoder must be reset afterward.
    /// If decode is called again when the decoder had a fatal error, then it will cause an Err with `XzError::NeedsReset`.
    /// The only errors that are not fatal are:
    /// - `XzError::NeedsLargerInputBuffer`
    ///     - Input buffer does not contain enough data to make progress
    ///
    pub fn decode(
        &mut self,
        input_data: &[u8],
        output_data: &mut [u8],
    ) -> Result<XzNextBlockResult, XzError> {
        self.inner
            .decode(input_data, output_data, &mut self.dictionary_buffer)
    }

    /// Returns whether a complete LZMA2 chunk has ended.
    ///
    /// When true, the decoder is before the next LZMA2 control byte
    /// and has no compressed input buffered internally.
    ///
    /// This does not indicate why the chunk ended, XZ stream
    /// completion or check validation, or that all filtered output
    /// has been returned.
    #[must_use]
    pub const fn is_lzma2_chunk_boundary(&self) -> bool {
        self.inner.is_lzma2_chunk_boundary()
    }

    /// Reset the decoder
    pub const fn reset(&mut self) {
        self.inner.reset();
    }
}

#[cfg(feature = "alloc")]
impl Default for XzDecoder<'static> {
    fn default() -> Self {
        Self::with_alloc_dict_size(4096, 1 << 26)
    }
}

/// Type of filter in a filter slot, there are 3 slots.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
enum Filter {
    /// Lzma/Empty.
    Empty = 0,
    ///Bcj filter
    #[cfg(feature = "bcj")]
    Bcj,
    ///Delta filter
    #[cfg(feature = "delta")]
    Delta,
}

/// Contains the entire state of the decoder except for the dictionary buffer.
#[derive(Debug)]
pub struct XzInnerDecoder {
    /// state machine state
    state: XzDecoderState,
    /// check algorithm to use
    check_type: XzCheckType,
    /// VLI decoder
    vli_decoder: VliDecoder,
    /// Crc32 and Crc64 state
    crc: u64,
    /// flag if we did not have enough data during the last call.
    had_not_enough_data: bool,
    /// previous size of the input buffer.
    last_input_buffer_size: usize,
    /// previous size of the output buffer.
    last_output_buffer_size: usize,
    /// Did we error and want to be reset?
    needs_reset: bool,
    /// current block header info
    block_header: XzBlockHeader,
    /// block decoding info
    block: XzDecBlock,
    /// index decoder
    index: XzDecoderIndex,
    /// temp buffer
    temp: XzTempBuffer,
    /// lzma decoder state
    lzma2: XzLzma2Decoder,
    /// Which filter chain are we using?
    filter_chain: [Filter; 3],
    /// state of the bcj filter.
    #[cfg(feature = "bcj")]
    bcj0: BcjFilterState,
    /// state of the bcj filter.
    #[cfg(feature = "bcj")]
    bcj1: BcjFilterState,
    /// state of the bcj filter.
    #[cfg(feature = "bcj")]
    bcj2: BcjFilterState,
    ///Delta decoder state
    #[cfg(feature = "delta")]
    delta0: DeltaDecoder,
    ///Delta decoder state
    #[cfg(feature = "delta")]
    delta1: DeltaDecoder,
    ///Delta decoder state
    #[cfg(feature = "delta")]
    delta2: DeltaDecoder,
    /// sha256 state
    #[cfg(feature = "sha256")]
    sha256: XzSha256,
}

impl Default for XzInnerDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl XzInnerDecoder {
    /// Constructor
    pub const fn new() -> Self {
        Self {
            state: XzDecoderState::StreamHeader,
            vli_decoder: VliDecoder::new(),
            crc: 0,
            check_type: XzCheckType::None,
            had_not_enough_data: false,
            last_input_buffer_size: 0,
            last_output_buffer_size: 0,
            needs_reset: false,
            block_header: XzBlockHeader::new(),
            block: XzDecBlock::new(),
            index: XzDecoderIndex::new(),
            temp: XzTempBuffer::new(),
            lzma2: XzLzma2Decoder::new(),
            #[cfg(feature = "sha256")]
            sha256: XzSha256::new(),
            #[cfg(feature = "bcj")]
            bcj0: BcjFilterState::new(),
            #[cfg(feature = "bcj")]
            bcj1: BcjFilterState::new(),
            #[cfg(feature = "bcj")]
            bcj2: BcjFilterState::new(),
            #[cfg(feature = "delta")]
            delta0: DeltaDecoder::new(),
            #[cfg(feature = "delta")]
            delta1: DeltaDecoder::new(),
            #[cfg(feature = "delta")]
            delta2: DeltaDecoder::new(),
            filter_chain: [Filter::Empty; 3],
        }
    }

    /// Returns whether the LZMA2 decoder is between chunks.
    const fn is_lzma2_chunk_boundary(&self) -> bool {
        !self.needs_reset
            && matches!(self.state, XzDecoderState::BlockUncompress)
            && matches!(self.lzma2.sequence, LzmaStreamState::Control)
            && !self.lzma2.need_dict_reset
            && self.lzma2.temp_size == 0
    }

    /// Updates the size and crc32 of the index.
    fn index_update(&mut self, b: &mut XzInOutBuffer, in_start: usize) {
        let position = b.input_position();
        b.input_seek_set(in_start);
        let in_used = position.sub(in_start);

        self.index.size = self.index.size.wrapping_add(in_used as u64);
        let x = &b.input_slice()[..in_used];
        self.crc = u64::from(crc32(clamp_u64_to_u32(self.crc), x));

        b.input_seek_set(position);
    }

    /// Decodes the block index.
    fn dec_index(
        &mut self,
        b: &mut XzInOutBuffer,
        in_start: usize,
    ) -> Result<DecodeResult, XzError> {
        loop {
            let vli = match self.vli_decoder.decode(b.input_slice()) {
                VliResult::Ok(vli, length) => {
                    b.input_seek_add(length);
                    vli
                }
                VliResult::MoreDataNeeded(length) => {
                    b.input_seek_add(length);
                    self.index_update(b, in_start);
                    return Ok(DecodeResult::NeedMoreData);
                }
                VliResult::InvalidVli => {
                    return Err(XzError::CorruptedDataInBlockIndex);
                }
            };
            match self.index.sequence {
                XzDecoderIndexSequence::Count => {
                    self.index.count = vli;
                    if self.index.count != self.block.count {
                        return Err(XzError::CorruptedDataInBlockIndex);
                    }
                    self.index.sequence = XzDecoderIndexSequence::Unpadded;
                }
                XzDecoderIndexSequence::Unpadded => {
                    self.index.hash.unpadded = self.index.hash.unpadded.wrapping_add(vli);
                    self.index.sequence = XzDecoderIndexSequence::Uncompressed;
                }
                XzDecoderIndexSequence::Uncompressed => {
                    self.index.hash.uncompressed = self.index.hash.uncompressed.wrapping_add(vli);
                    self.index.hash.calculate_crc32();
                    self.index.count = self.index.count.wrapping_sub(1);
                    self.index.sequence = XzDecoderIndexSequence::Unpadded;
                }
            }
            if self.index.count == 0 {
                break;
            }
        }
        Ok(DecodeResult::EndOfDataStructure)
    }

    /// decodes the stream footer and verifies that the magic number of the footer matches and the crc32 of the footer is equal to the one indicated in the header.
    fn dec_stream_footer(&self) -> Result<(), XzError> {
        const MAGIC_NUMBER: &[u8] = b"YZ";
        let buf = self.temp.buf();
        if &buf[10..10 + MAGIC_NUMBER.len()] != MAGIC_NUMBER {
            return Err(XzError::FooterMagicNumberMismatch);
        }

        let expected_crc = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let actual_crc = crc32(0, &buf[4..10]);
        if actual_crc != expected_crc {
            return Err(XzError::FooterCrc32Mismatch(actual_crc, expected_crc));
        }

        let actual_index = u64::from(u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]));

        if self.index.size >> 2i32 != actual_index {
            return Err(XzError::FooterDecoderIndexMismatch(
                actual_index,
                self.index.size >> 2,
            ));
        }
        if self.temp.buf[8] != 0 || self.check_type != self.temp.buf[9] {
            let actual = u32::from(u16::from_le_bytes([self.temp.buf[8], self.temp.buf[9]]));
            return Err(XzError::FooterCheckTypeMismatch(actual, self.check_type));
        }
        Ok(())
    }

    /// decodes a block header from the stream.
    #[allow(clippy::too_many_lines)] //Todo re-implement this function with some sort of borrowed cursor and split it into sections that make sense.
    fn dec_block_header(&mut self, d: &mut XzDictBuffer) -> Result<(), XzError> {
        //the temp buffer size is determined by the block header size, which should be at least 8 even with a malicious input file.
        debug_assert!(self.temp.size >= 8);

        let expected_crc = u32::from_le_bytes(self.temp.remove_trailing_4bytes());
        let actual_crc = crc32(0, self.temp.buf());
        if actual_crc != expected_crc {
            return Err(XzError::BlockHeaderCrc32Mismatch(actual_crc, expected_crc));
        }

        debug_assert_eq!(self.temp.pos, 0);
        let buf = self.temp.buf();

        let mut pos = 2usize;
        if buf[1] & 0x3C != 0 {
            //TODO unreached.
            return Err(XzError::UnsupportedBlockHeaderOption);
        }
        if buf[1] & 0x40 != 0 {
            let Some((vli, len)) = self.vli_decoder.decode_single(&buf[pos..]) else {
                // TODO unreached
                return Err(XzError::CorruptedCompressedLengthVliInBlockHeader);
            };

            pos += len;
            self.block_header.compressed = vli;
        } else {
            self.block_header.compressed = u64::MAX;
        }

        if buf[1] & 0x80 != 0 {
            let Some((vli, len)) = self.vli_decoder.decode_single(&buf[pos..]) else {
                return Err(XzError::CorruptedUncompressedLengthVliInBlockHeader);
            };

            pos += len;
            self.block_header.uncompressed = vli;
        } else {
            self.block_header.uncompressed = u64::MAX;
        }

        let filter_count = (buf[1] & 0x03) as usize;
        for i in 0..filter_count {
            let bcj = buf[pos] != 3;
            #[cfg(feature = "bcj")]
            {
                if bcj {
                    self.filter_chain[i] = Filter::Bcj;
                    if self.temp.size.wrapping_sub(pos) < 2 {
                        //TODO unreached.
                        return Err(XzError::BlockHeaderTooSmall);
                    }
                    let filter_id = buf[pos];
                    pos += 1;
                    let bcj_filter = match i {
                        0 => &mut self.bcj0,
                        1 => &mut self.bcj1,
                        2 => &mut self.bcj2,
                        _ => unreachable!(),
                    };

                    let offset_size = buf[pos];

                    if offset_size == 0 {
                        // No bcj offset
                        pos += 1;
                        bcj_filter.reset(filter_id, 0)?;
                        continue;
                    }

                    if offset_size != 4 {
                        //Offsets that are not 4 byte not supported.
                        return Err(XzError::UnsupportedBlockHeaderOption);
                    }

                    // Offset is present, 32 bit le. In theory should be nonzero.
                    if self.temp.size.wrapping_sub(pos) < 5 {
                        //TODO unreached.
                        return Err(XzError::BlockHeaderTooSmall);
                    }

                    bcj_filter.reset(
                        filter_id,
                        u32::from_le_bytes([
                            buf[pos + 1],
                            buf[pos + 2],
                            buf[pos + 3],
                            buf[pos + 4],
                        ]),
                    )?;

                    pos += 5;
                    continue;
                }
            }
            #[cfg(not(feature = "bcj"))]
            {
                if bcj {
                    return Err(XzError::BcjFilterNotSupported);
                }
            }
            let delta = buf[pos] == 3;
            #[cfg(feature = "delta")]
            {
                if delta {
                    if self.temp.size.wrapping_sub(pos) < 2 {
                        //TODO unreached
                        return Err(XzError::BlockHeaderTooSmall);
                    }
                    pos += 1;
                    //length of "distance" we only support 1 byte distance aka 1 to 256
                    if buf[pos] != 1 {
                        //TODO unreached
                        return Err(XzError::UnsupportedBlockHeaderOption);
                    }
                    pos += 1;
                    let distance = buf[pos]; //0 means distance of 1!
                    let delta_coder = match i {
                        0 => &mut self.delta0,
                        1 => &mut self.delta1,
                        2 => &mut self.delta2,
                        _ => unreachable!(),
                    };
                    delta_coder.reset(
                        NonZeroUsize::new((distance as usize) + 1)
                            .ok_or(XzError::UnsupportedBlockHeaderOption)?,
                    ); //ERR is unreachable!
                    self.filter_chain[i] = Filter::Delta;
                    pos += 1;
                    continue;
                }
            }
            #[cfg(not(feature = "delta"))]
            {
                if delta {
                    return Err(XzError::DeltaFilterUnsupported);
                }
            }

            self.filter_chain[i] = Filter::Empty; //This should be unreachable!
        }

        for i in filter_count..self.filter_chain.len() {
            self.filter_chain[i] = Filter::Empty;
        }

        if self.temp.size().saturating_sub(pos) < 2 {
            return Err(XzError::BlockHeaderTooSmall);
        }

        if buf[pos] != 0x21 {
            return Err(XzError::UnsupportedBlockHeaderOption);
        }
        pos += 1;

        if self.temp.buf[pos] != 0x1 {
            //TODO unreached
            return Err(XzError::UnsupportedBlockHeaderOption);
        }
        pos += 1;

        if self.temp.size().saturating_sub(pos) < 1 {
            return Err(XzError::BlockHeaderTooSmall);
        }
        self.lzma2.xz_dec_lzma2_reset(self.temp.buf[pos], d)?;
        pos += 1;

        while pos < self.temp.size() {
            if buf[pos] != 0 {
                return Err(XzError::UnsupportedBlockHeaderOption);
            }
            pos += 1;
        }
        self.temp.pos = 0; //why?
        self.block.compressed = 0;
        self.block.uncompressed = 0;
        Ok(())
    }

    /// Fills the temp buffer.
    fn fill_temp(&mut self, b: &mut XzInOutBuffer) -> bool {
        //TODO move this fn to the temp buffer.
        let input = b.input_slice();
        let copy_size = self.temp.available().min(input.len());
        self.temp
            .fill_slice(copy_size)
            .copy_from_slice(&input[..copy_size]);

        b.input_seek_add(copy_size);

        self.temp.pos += copy_size;
        if self.temp.pos == self.temp.size {
            self.temp.pos = 0;
            return true;
        }
        false
    }

    /// Delegates block decoding to the bcj filter or calls the lzma decoder.
    pub fn apply_filter(
        &mut self,
        b: &mut XzInOutBuffer,
        d: &mut XzDictBuffer,
    ) -> Result<DecodeResult, XzError> {
        match (
            self.filter_chain[0],
            self.filter_chain[1],
            self.filter_chain[2],
        ) {
            (Filter::Empty, _, _) => self.lzma2.xz_dec_lzma2_run(b, d),
            #[cfg(feature = "delta")]
            (Filter::Delta, f2, f3) => self.delta0.run(
                |b, d| match f2 {
                    Filter::Empty => self.lzma2.xz_dec_lzma2_run(b, d),
                    #[cfg(feature = "bcj")]
                    Filter::Bcj => self.bcj1.run(
                        |b, d| match f3 {
                            Filter::Empty => self.lzma2.xz_dec_lzma2_run(b, d),
                            Filter::Bcj => {
                                self.bcj2
                                    .run(|b, d| self.lzma2.xz_dec_lzma2_run(b, d), b, d)
                            }
                            Filter::Delta => {
                                self.delta2
                                    .run(|b, d| self.lzma2.xz_dec_lzma2_run(b, d), b, d)
                            }
                        },
                        b,
                        d,
                    ),
                    Filter::Delta => self.delta1.run(
                        |b, d| match f3 {
                            Filter::Empty => self.lzma2.xz_dec_lzma2_run(b, d),
                            #[cfg(feature = "bcj")]
                            Filter::Bcj => {
                                self.bcj2
                                    .run(|b, d| self.lzma2.xz_dec_lzma2_run(b, d), b, d)
                            }
                            Filter::Delta => {
                                self.delta2
                                    .run(|b, d| self.lzma2.xz_dec_lzma2_run(b, d), b, d)
                            }
                        },
                        b,
                        d,
                    ),
                },
                b,
                d,
            ),
            #[cfg(feature = "bcj")]
            (Filter::Bcj, f2, f3) => self.bcj0.run(
                |b, d| match f2 {
                    Filter::Empty => self.lzma2.xz_dec_lzma2_run(b, d),
                    Filter::Bcj => self.bcj1.run(
                        |b, d| match f3 {
                            Filter::Empty => self.lzma2.xz_dec_lzma2_run(b, d),
                            Filter::Bcj => {
                                self.bcj2
                                    .run(|b, d| self.lzma2.xz_dec_lzma2_run(b, d), b, d)
                            }
                            #[cfg(feature = "delta")]
                            Filter::Delta => {
                                self.delta2
                                    .run(|b, d| self.lzma2.xz_dec_lzma2_run(b, d), b, d)
                            }
                        },
                        b,
                        d,
                    ),
                    #[cfg(feature = "delta")]
                    Filter::Delta => self.delta1.run(
                        |b, d| match f3 {
                            Filter::Empty => self.lzma2.xz_dec_lzma2_run(b, d),
                            Filter::Bcj => {
                                self.bcj2
                                    .run(|b, d| self.lzma2.xz_dec_lzma2_run(b, d), b, d)
                            }
                            Filter::Delta => {
                                self.delta2
                                    .run(|b, d| self.lzma2.xz_dec_lzma2_run(b, d), b, d)
                            }
                        },
                        b,
                        d,
                    ),
                },
                b,
                d,
            ),
        }
    }

    /// Decodes a block
    fn dec_block(
        &mut self,
        b: &mut XzInOutBuffer,
        d: &mut XzDictBuffer,
    ) -> Result<DecodeResult, XzError> {
        // Note: in the C impl this used to write to global state, we use the stack here.
        // This was likely an attempt to save stack space in the C impl.
        // Since this is not a priority of this implementation, we just use local variables here.
        // It appears this is not needed since StreamStart state will overwrite the "state" again
        // and all state transitions until StreamStart did not use it.
        let in_start = b.input_position();
        let out_start = b.output_position();

        let ret = self.apply_filter(b, d)?;

        //TODO probably doesnt wrap
        self.block.compressed = self
            .block
            .compressed
            .wrapping_add(b.input_position().wrapping_sub(in_start) as u64);

        self.block.uncompressed = self
            .block
            .uncompressed
            .wrapping_add(b.output_position().wrapping_sub(out_start) as u64);

        if self.block.compressed > self.block_header.compressed
            || self.block.uncompressed > self.block_header.uncompressed
        {
            return Err(XzError::MoreDataInBlockBodyThanHeaderIndicated);
        }

        match self.check_type {
            #[cfg(feature = "sha256")]
            XzCheckType::Sha256 => self.sha256.update(b.output_slice_look_back(out_start)),

            #[cfg(feature = "crc64")]
            XzCheckType::Crc64 => {
                self.crc = crate::crc64xz::crc64xz(self.crc, b.output_slice_look_back(out_start));
            }
            XzCheckType::Crc32 => {
                self.crc = u64::from(crc32(
                    clamp_u64_to_u32(self.crc),
                    b.output_slice_look_back(out_start),
                ));
            }
            XzCheckType::None => (),
        }

        if ret != DecodeResult::EndOfDataStructure {
            return Ok(ret);
        }

        if self.block_header.compressed != u64::MAX
            && self.block_header.compressed != self.block.compressed
        {
            return Err(XzError::LessDataInBlockBodyThanHeaderIndicated);
        }
        if self.block_header.uncompressed != u64::MAX
            && self.block_header.uncompressed != self.block.uncompressed
        {
            //TODO unreached
            return Err(XzError::LessDataInBlockBodyThanHeaderIndicated);
        }
        self.block.hash.unpadded = self
            .block
            .hash
            .unpadded
            .wrapping_add((self.block_header.size as u64).wrapping_add(self.block.compressed));

        self.block.hash.unpadded = self
            .block
            .hash
            .unpadded
            .wrapping_add(self.check_type.check_size() as u64);
        self.block.hash.uncompressed = self
            .block
            .hash
            .uncompressed
            .wrapping_add(self.block.uncompressed);
        self.block.hash.calculate_crc32();
        self.block.count += 1;
        Ok(DecodeResult::EndOfDataStructure)
    }

    /// main decoder loop
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::match_wildcard_for_single_variants)]
    fn dec_main(
        &mut self,
        b: &mut XzInOutBuffer,
        d: &mut XzDictBuffer,
    ) -> Result<DecodeResult, XzError> {
        let mut in_start = b.input_position();
        loop {
            match self.state {
                XzDecoderState::StreamHeader => {
                    if !self.fill_temp(b) {
                        return Ok(DecodeResult::NeedMoreData);
                    }
                    self.dec_stream_header()?;
                    self.state = XzDecoderState::StreamStart;
                }
                XzDecoderState::StreamStart => {
                    let Some(m) = b.input_peek_byte::<usize>() else {
                        return Ok(DecodeResult::NeedMoreData);
                    };

                    if m == 0 {
                        in_start = b.input_pos;
                        b.input_pos = b.input_pos.wrapping_add(1);
                        self.state = XzDecoderState::Index;
                        continue;
                    }

                    self.block_header.size = (m + 1) * 4;
                    self.temp.size = self.block_header.size;
                    self.temp.pos = 0;
                    self.state = XzDecoderState::BlockHeader;
                }
                XzDecoderState::BlockHeader => {
                    if !self.fill_temp(b) {
                        return Ok(DecodeResult::NeedMoreData);
                    }
                    self.dec_block_header(d)?;

                    #[cfg(feature = "sha256")]
                    if self.check_type == XzCheckType::Sha256 {
                        self.sha256.reset();
                    }
                    self.state = XzDecoderState::BlockUncompress;
                }
                XzDecoderState::BlockUncompress => match self.dec_block(b, d)? {
                    DecodeResult::EndOfDataStructure => {
                        self.state = XzDecoderState::BlockPadding;
                    }
                    other => return Ok(other),
                },
                XzDecoderState::BlockPadding => match self.block.read_block_padding(b)? {
                    DecodeResult::EndOfDataStructure => {
                        self.state = XzDecoderState::BlockCheck;
                    }
                    other => return Ok(other),
                },
                XzDecoderState::BlockCheck => {
                    match self.check_type {
                        XzCheckType::Crc32 => {
                            self.temp.size = 4;
                            if !self.fill_temp(b) {
                                return Ok(DecodeResult::NeedMoreData);
                            }

                            let expected_crc = u32::from_le_bytes([
                                self.temp.buf[0],
                                self.temp.buf[1],
                                self.temp.buf[2],
                                self.temp.buf[3],
                            ]);
                            let actual_crc = clamp_u64_to_u32(self.crc);
                            if expected_crc != actual_crc {
                                return Err(XzError::ContentCrc32Mismatch(
                                    actual_crc,
                                    expected_crc,
                                ));
                            }
                            self.crc = 0;
                        }
                        #[cfg(feature = "crc64")]
                        XzCheckType::Crc64 => {
                            self.temp.size = 8;
                            if !self.fill_temp(b) {
                                return Ok(DecodeResult::NeedMoreData);
                            }

                            let expected_crc = u64::from_le_bytes([
                                self.temp.buf[0],
                                self.temp.buf[1],
                                self.temp.buf[2],
                                self.temp.buf[3],
                                self.temp.buf[4],
                                self.temp.buf[5],
                                self.temp.buf[6],
                                self.temp.buf[7],
                            ]);
                            if expected_crc != self.crc {
                                return Err(XzError::ContentCrc64Mismatch(self.crc, expected_crc));
                            }
                            self.crc = 0;
                        }
                        #[cfg(feature = "sha256")]
                        XzCheckType::Sha256 => {
                            self.temp.size = 32;
                            if !self.fill_temp(b) {
                                return Ok(DecodeResult::NeedMoreData);
                            }
                            self.sha256.validate(&self.temp.buf[0..32])?;
                        }
                        XzCheckType::None => (),
                    }
                    self.state = XzDecoderState::StreamStart;
                }
                XzDecoderState::Index => {
                    match self.dec_index(b, in_start)? {
                        DecodeResult::EndOfDataStructure => (),
                        other => return Ok(other),
                    }

                    self.state = XzDecoderState::IndexPadding;
                }
                XzDecoderState::IndexPadding => {
                    while self
                        .index
                        .size
                        .wrapping_add((b.input_pos - in_start) as u64)
                        & 3
                        != 0
                    {
                        let Some(next_byte) = b.input_read_byte::<u8>() else {
                            self.index_update(b, in_start);
                            return Ok(DecodeResult::NeedMoreData);
                        };
                        if next_byte != 0 {
                            return Err(XzError::CorruptedData);
                        }
                    }
                    self.index_update(b, in_start);
                    if self.block.hash != self.index.hash {
                        return Err(XzError::CorruptedData);
                    }
                    self.state = XzDecoderState::IndexCrc32;
                }
                XzDecoderState::IndexCrc32 => {
                    self.temp.size = 4;
                    if !self.fill_temp(b) {
                        return Ok(DecodeResult::NeedMoreData);
                    }

                    let expected_crc = u32::from_le_bytes([
                        self.temp.buf[0],
                        self.temp.buf[1],
                        self.temp.buf[2],
                        self.temp.buf[3],
                    ]);
                    let actual_crc = clamp_u64_to_u32(self.crc);
                    if expected_crc != actual_crc {
                        return Err(XzError::IndexCrc32Mismatch(actual_crc, expected_crc));
                    }
                    self.crc = 0;

                    self.temp.size = 12;
                    self.state = XzDecoderState::StreamFooter;
                }
                XzDecoderState::StreamFooter => {
                    if !self.fill_temp(b) {
                        return Ok(DecodeResult::NeedMoreData);
                    }
                    self.dec_stream_footer()?;
                    return Ok(DecodeResult::EndOfDataStructure);
                }
            }
        }
    }

    /// Determine if a more output or input buffer result should trigger error or not.
    fn should_buffer_error(&mut self, buf: &XzInOutBuffer) -> bool {
        if buf.input_position() != 0 || buf.output_position() != 0 {
            self.had_not_enough_data = false;
            self.last_input_buffer_size = 0;
            self.last_output_buffer_size = 0;

            return false;
        }

        let input_size = buf.input_remaining();
        let output_size = buf.output_len();
        if self.had_not_enough_data
            && self.last_input_buffer_size >= input_size
            && self.last_output_buffer_size >= output_size
        {
            return true;
        }
        self.last_input_buffer_size = input_size.max(self.last_input_buffer_size);
        self.last_output_buffer_size = output_size.max(self.last_output_buffer_size);
        self.had_not_enough_data = true;
        false
    }

    /// Begins decoding, high level function that's called externally.
    /// Mainly takes care of error handling.
    fn decode(
        &mut self,
        input_data: &[u8],
        output_data: &mut [u8],
        d: &mut XzDictBuffer,
    ) -> Result<XzNextBlockResult, XzError> {
        if self.needs_reset {
            return Err(XzError::NeedsReset);
        }
        if input_data.is_empty() {
            return Err(XzError::NeedsLargerInputBuffer);
        }

        let mut buf = XzInOutBuffer::new(input_data, output_data);
        match self
            .dec_main(&mut buf, d)
            .inspect_err(|_| self.needs_reset = true)?
        {
            DecodeResult::NeedMoreData => {
                if self.should_buffer_error(&buf) {
                    return Err(XzError::NeedsLargerInputBuffer);
                }

                Ok(XzNextBlockResult::NeedMoreData(
                    buf.input_position(),
                    buf.output_position(),
                ))
            }
            DecodeResult::EndOfDataStructure => {
                self.needs_reset = true;
                Ok(XzNextBlockResult::EndOfStream(
                    buf.input_position(),
                    buf.output_position(),
                ))
            }
        }
    }

    /// decodes the stream header and calculates/validates its crc32.
    fn dec_stream_header(&mut self) -> Result<(), XzError> {
        const MAGIC_NUMBER: &[u8] = b"\xFD7zXZ\0";
        let buf = self.temp.buf();
        if &buf[0..MAGIC_NUMBER.len()] != MAGIC_NUMBER {
            return Err(XzError::StreamHeaderMagicNumberMismatch);
        }

        let expected_crc = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
        let actual_crc = crc32(0, &buf[6..8]);
        if actual_crc != expected_crc {
            return Err(XzError::StreamHeaderCrc32Mismatch(actual_crc, expected_crc));
        }

        if buf[6] != 0 {
            return Err(XzError::UnsupportedStreamHeaderOption);
        }

        if buf[7] > 15 {
            return Err(XzError::UnsupportedStreamHeaderOption);
        }

        self.check_type = XzCheckType::try_from(buf[7])?;
        Ok(())
    }

    /// Reset the entire decoder to its default state where it's ready to process a fresh stream.
    const fn reset(&mut self) {
        self.state = XzDecoderState::StreamHeader;
        self.had_not_enough_data = false;
        self.needs_reset = false;
        self.last_output_buffer_size = 0;
        self.last_input_buffer_size = 0;
        self.vli_decoder.reset();
        self.crc = 0;
        self.block.reset();
        self.index.reset();
        self.temp.pos = 0;
        self.temp.size = 12;
    }
}

/// Temporary buffer that is filled by some steps during decoding.
#[derive(Clone, Debug)]
pub struct XzTempBuffer {
    /// Position in the temp buffer.
    pub pos: usize,
    /// reduced size of the temp buffer. always <= 1024
    pub size: usize,
    /// actual buffer.
    pub buf: [u8; 1024],
}

impl XzTempBuffer {
    /// Constructor.
    const fn new() -> Self {
        Self {
            pos: 0,
            size: 12,
            buf: [0; 1024],
        }
    }

    /// Some debug only checks. This function is a noop on release builds and will be optimized away.
    fn check_consistent(&self) {
        debug_assert!(self.size >= self.pos, "{} >= {}", self.size, self.pos);
        debug_assert!(
            self.buf.len() >= self.size,
            "{} >= {}",
            self.buf.len(),
            self.size
        );
    }

    /// size of temp buffer.
    fn size(&self) -> usize {
        self.check_consistent();
        self.size
    }

    /// position in temp buffer.
    fn pos(&self) -> usize {
        self.check_consistent();
        self.pos
    }

    /// amount of bytes that can still be read from (or written to) the buffer.
    fn available(&self) -> usize {
        self.check_consistent();
        self.size() - self.pos()
    }

    /// The buffer at position and limited to size.
    fn buf(&self) -> &[u8] {
        self.check_consistent();
        &self.buf.as_slice()[self.pos()..self.size()]
    }

    /// removes 4 trailing bytes.
    /// This fn should not be called if `available()` is less than 4.
    ///
    /// # Panics and Safety
    /// always panics on debug builds if `available()` is less than 4.
    /// behavior on release builds depends on how usize underflow's.
    /// If `size()` is less than 4 and usize wraps around normally during an underflow (x86 for ex.)
    /// then this fn will panic in release mode.
    /// If usize clamps to 0 then this fn will return 4 "unexpected" bytes
    /// and leave the temp buffer in an otherwise illegal state.
    /// If `size()` is greater than 4 but `available()` is less than 4 then this fn will always leave the buffer
    /// in an illegal state and return 4 "unexpected" bytes on release builds.
    fn remove_trailing_4bytes(&mut self) -> [u8; 4] {
        self.check_consistent();
        self.size -= 4;
        self.check_consistent();
        [
            self.buf[self.size],
            self.buf[self.size + 1],
            self.buf[self.size + 2],
            self.buf[self.size + 3],
        ]
    }

    /// Returns a slice that is exactly `feed_count` elements big.
    /// This fn should not be called with a `feed_count` larger than `available()`.
    ///
    /// # Panics and Safety
    /// panics on debug builds if `feed_count` is larger than `available()`
    /// On release builds unless `feed_count` + `pos()` is larger than 1024 this fn will
    /// return a slice that writes beyond the desired limit of the buffer. This memory
    /// is still part of the buffer allocation but any bytes written to there
    /// are likely to be discarded. Causing errors later down the line.
    ///
    fn fill_slice(&mut self, feed_count: usize) -> &mut [u8] {
        self.check_consistent();
        debug_assert!(
            self.available() >= feed_count,
            "{} >= {}",
            self.available(),
            feed_count
        );
        let pos = self.pos();
        &mut self.buf.as_mut_slice()[pos..pos + feed_count]
    }
}

impl Default for XzTempBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// Enum that defines the state of the index decoder state machine.
#[derive(Clone, Default, Debug)]
#[repr(u8)]
pub enum XzDecoderIndexSequence {
    #[default]
    /// TODO
    Count,
    /// TODO
    Unpadded,
    /// TODO
    Uncompressed,
}

/// Index decoder
#[derive(Clone, Default, Debug)]
struct XzDecoderIndex {
    /// state machine state
    sequence: XzDecoderIndexSequence,
    /// TODO
    size: u64,
    /// TODO
    count: u64,
    /// hash of the index
    hash: XzDecoderHash,
}

impl XzDecoderIndex {
    /// Constructor
    const fn new() -> Self {
        Self {
            sequence: XzDecoderIndexSequence::Count,
            size: 0,
            count: 0,
            hash: XzDecoderHash::new(),
        }
    }

    /// Reset to default state.
    const fn reset(&mut self) {
        self.sequence = XzDecoderIndexSequence::Count;
        self.size = 0;
        self.count = 0;
        self.hash.reset();
    }
}

///Hash information to verify the decoding state.
#[derive(Clone, Default, Eq, PartialEq, Debug)]
pub struct XzDecoderHash {
    /// calculated based on unpadded bytes during index and compressed bytes during block decoding.
    pub unpadded: u64,
    /// Calculated based on uncompressed bytes during block decoding.
    pub uncompressed: u64,
    /// crc32 state that continuously gets updated with each block.
    pub crc32: u32,
}

impl XzDecoderHash {
    /// Constructor
    const fn new() -> Self {
        Self {
            unpadded: 0,
            uncompressed: 0,
            crc32: 0,
        }
    }

    /// Reset the hash.
    const fn reset(&mut self) {
        self.unpadded = 0;
        self.uncompressed = 0;
        self.crc32 = 0;
    }

    /// Calculates the crc32 of the block.
    fn calculate_crc32(&mut self) {
        let unpadded_bytes = self.unpadded.to_ne_bytes();
        let uncompressed_bytes = self.uncompressed.to_ne_bytes();
        let crc32_bytes = self.crc32.to_ne_bytes();
        let buf = [
            unpadded_bytes[0],
            unpadded_bytes[1],
            unpadded_bytes[2],
            unpadded_bytes[3],
            unpadded_bytes[4],
            unpadded_bytes[5],
            unpadded_bytes[6],
            unpadded_bytes[7],
            uncompressed_bytes[0],
            uncompressed_bytes[1],
            uncompressed_bytes[2],
            uncompressed_bytes[3],
            uncompressed_bytes[4],
            uncompressed_bytes[5],
            uncompressed_bytes[6],
            uncompressed_bytes[7],
            crc32_bytes[0],
            crc32_bytes[1],
            crc32_bytes[2],
            crc32_bytes[3],
        ];

        self.crc32 = crc32(self.crc32, buf.as_slice());
    }
}

/// This struct gets populated during the entire decoding process of all blocks and tracks information about the decoding.
#[derive(Clone, Default, Debug)]
struct XzDecBlock {
    /// Amount of compressed bytes
    pub compressed: u64,
    /// Amount of uncompressed bytes
    pub uncompressed: u64,
    /// Amount of blocks already decoded.
    pub count: u64,

    ///Hash information to verify the decoding state.
    pub hash: XzDecoderHash,
}

impl XzDecBlock {
    /// Constructor.
    const fn new() -> Self {
        Self {
            compressed: 0,
            uncompressed: 0,
            count: 0,
            hash: XzDecoderHash::new(),
        }
    }

    /// Reads the padding bytes calculated based on the dec block.
    fn read_block_padding(&mut self, b: &mut XzInOutBuffer) -> Result<DecodeResult, XzError> {
        while self.compressed & 3 != 0 {
            let Some(padding_byte) = b.input_read_byte::<u8>() else {
                return Ok(DecodeResult::NeedMoreData);
            };
            if padding_byte != 0 {
                return Err(XzError::CorruptedData);
            }
            self.compressed = self.compressed.wrapping_add(1);
        }
        Ok(DecodeResult::EndOfDataStructure)
    }

    /// Rests the struct to its default state.
    const fn reset(&mut self) {
        self.compressed = 0;
        self.uncompressed = 0;
        self.count = 0;
        self.hash.reset();
    }
}

/// Block header information.
#[derive(Clone, Default, Debug)]
pub struct XzBlockHeader {
    /// amount of compressed data `u64::MAX` means N/A
    pub compressed: u64,

    /// amount of uncompressed data `u64::MAX` means N/A
    pub uncompressed: u64,

    /// Size of the header in bytes.
    pub size: usize,
}

impl XzBlockHeader {
    /// Constructor
    const fn new() -> Self {
        Self {
            compressed: 0,
            uncompressed: 0,
            size: 0,
        }
    }
}

/// Result enum that specifies the outcome of a decoding operation
#[derive(Debug, Eq, PartialEq, Clone, Default, Copy)]
#[repr(u8)]
pub enum DecodeResult {
    /// Decoder requires more data to make progress
    #[default]
    NeedMoreData = 0,
    /// Decoder made progress and a data structure is fully read
    EndOfDataStructure = 1,
}
