use alloc::vec;
use alloc::vec::Vec;
use crate::encoder;
use crate::encoder::range_coder;
use crate::encoder::state::State;

const POS_STATES_MAX : i32 = 1 << 4;

const MATCH_LEN_MIN : i32 = 2;

const LOW_SYMBOLS : i32 = 1 << 3;
const MID_SYMBOLS : i32 = 1 << 3;
const HIGH_SYMBOLS : i32 = 1 << 8;

const MATCH_LEN_MAX : i32 = MATCH_LEN_MIN + LOW_SYMBOLS + MID_SYMBOLS + HIGH_SYMBOLS - 1;

const DIST_STATES : i32 = 4;
const DIST_SLOTS : i32 = 1 << 6;
const DIST_MODEL_START : i32 = 4;
const DIST_MODEL_END : i32 = 14;
const FULL_DISTANCES : i32 = 1 << (DIST_MODEL_END / 2);

const ALIGN_BITS : i32 = 4;
const ALIGN_SIZE : i32 = 1 << ALIGN_BITS;
const ALIGN_MASK : i32 = ALIGN_SIZE - 1;
const REPS : i32 = 4;
struct LZMACoder {
    posMask : i32,
    reps : [i32; REPS as usize],
    state: State,
    isMatch: [[i16; POS_STATES_MAX as usize]; encoder::state::STATES as usize],
    isRep: [i16; encoder::state::STATES as usize],
    isRep0: [i16; encoder::state::STATES as usize],
    isRep1: [i16; encoder::state::STATES as usize],
    isRep2: [i16; encoder::state::STATES as usize],
    isRep0Long: [[i16; POS_STATES_MAX as usize]; encoder::state::STATES as usize],
    distSlots: [[i16; DIST_SLOTS as usize]; DIST_STATES as usize],
    distSpecial: Vec<Vec<i16>>, //2,2,4,4,8,8,16,16,32,32
    distAlign: [i16; ALIGN_SIZE as usize],
}

impl LZMACoder {
    pub fn getDistState(len: i32) -> i16 {
        if len < DIST_STATES + MATCH_LEN_MIN {
            return (len - MATCH_LEN_MIN) as i16;
        }
        (DIST_STATES - 1) as i16
    }

    pub fn new(pb: i32) -> Self {
        Self {
            posMask: (1 << pb) - 1,
            reps: [0; 4],
            state: Default::default(),
            isMatch: [[0; POS_STATES_MAX as usize]; 12],
            isRep: [0; 12],
            isRep0: [0; 12],
            isRep1: [0; 12],
            isRep2: [0; 12],
            isRep0Long: [[0; POS_STATES_MAX as usize]; 12],
            distSlots: [[0; DIST_SLOTS as usize]; DIST_STATES as usize],
            distSpecial: vec![
                vec![0; 2], vec![0; 2],
                vec![0; 4], vec![0; 4],
                vec![0; 8], vec![0; 8],
                vec![0; 16], vec![0; 16],
                vec![0; 32], vec![0; 32]
            ],
            distAlign: [0; ALIGN_SIZE as usize],
        }
    }

    pub fn reset(&mut self) {
        self.reps[0] = 0;
        self.reps[1] = 0;
        self.reps[2] = 0;
        self.reps[3] = 0;
        self.state.reset();

        for i in 0..self.isMatch.len() {
            range_coder::initProbs(self.isMatch[i].as_mut_slice());
        }

        range_coder::initProbs(self.isRep.as_mut_slice());
        range_coder::initProbs(self.isRep0.as_mut_slice());
        range_coder::initProbs(self.isRep1.as_mut_slice());
        range_coder::initProbs(self.isRep2.as_mut_slice());

        for i in 0..self.isRep0Long.len() {
            range_coder::initProbs(self.isRep0Long[i].as_mut_slice());
        }

        for i in 0..self.distSlots.len() {
            range_coder::initProbs(self.distSlots[i].as_mut_slice());
        }

        for i in 0..self.distSpecial.len() {
            range_coder::initProbs(self.distSpecial[i].as_mut_slice());
        }

        range_coder::initProbs(self.distAlign.as_mut_slice());
    }
}

struct LiteralCoder {
    lc: i32,
    literalPosMask: i32,
}

impl LiteralCoder {
    pub fn new(lc: i32, lp: i32) -> Self {
        Self {
            lc,
            literalPosMask: (1 << lp) - 1,
        }
    }

    pub fn getSubcoderIndex(&self, prevByte: i32, pos: i32) -> i32 {
        let low = prevByte >> (8 - self.lc);
        let high = (pos & self.literalPosMask) << self.lc;
        low + high
    }
}

pub struct LiteralSubcoder {
    pub probs: [i16; 0x300]
}

impl LiteralSubcoder {
    pub fn reset(&mut self) {
        range_coder::initProbs(self.probs.as_mut_slice());
    }
}

pub struct LengthCoder {
    choice : [i16; 2],
    low : [[i16; LOW_SYMBOLS as usize]; POS_STATES_MAX as usize],
    mid : [[i16; MID_SYMBOLS as usize]; POS_STATES_MAX as usize],
    high : [i16; HIGH_SYMBOLS as usize],
}

impl LengthCoder {
    pub fn reset(&mut self) {
        range_coder::initProbs(self.choice.as_mut_slice());

        for i in 0..self.low.len() {
            range_coder::initProbs(self.low[i].as_mut_slice());
        }
        for i in 0..self.mid.len() {
            range_coder::initProbs(self.mid[i].as_mut_slice());
        }

        range_coder::initProbs(self.high.as_mut_slice());
    }
}


