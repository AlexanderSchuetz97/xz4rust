//public abstract class RangeCoder {
pub const SHIFT_BITS : i32 = 8;
pub const TOP_MASK : i32 = 0xFF000000u32 as i32;
pub const BIT_MODEL_TOTAL_BITS : i32 = 11;
pub const BIT_MODEL_TOTAL : i32 = 1 << BIT_MODEL_TOTAL_BITS;
pub const PROB_INIT : i16 = (BIT_MODEL_TOTAL / 2) as i16;
pub const MOVE_BITS : i32 = 5;

pub fn initProbs(probs: &mut [i16]) {
    probs.fill(0);
}