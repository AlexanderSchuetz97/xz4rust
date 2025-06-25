pub const STATES : i32 = 12;

pub const LIT_STATES : i32 = 7;

pub const LIT_LIT : i32 = 0;
pub const MATCH_LIT_LIT : i32 = 1;
pub const REP_LIT_LIT : i32 = 2;
pub const SHORTREP_LIT_LIT : i32 = 3;
pub const MATCH_LIT : i32 = 4;
pub const REP_LIT : i32 = 5;
pub const SHORTREP_LIT : i32 = 6;
pub const LIT_MATCH : i32 = 7;
pub const LIT_LONGREP : i32 = 8;
pub const LIT_SHORTREP : i32 = 9;
pub const NONLIT_MATCH : i32 = 10;
pub const NONLIT_REP : i32 = 11;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct State {
    state: i32,
}

impl Default for State {
    fn default() -> Self {
        Self { state: 0 }
    }
}

impl State {
    pub fn new() -> Self {
        Self { state: LIT_LIT }
    }

    pub fn new2(other: Self) -> Self {
        Self { state: other.state }
    }

    pub fn reset(&mut self) {
        self.state = LIT_LIT;
    }

    pub fn get(&self) -> i32 {
        self.state
    }

    pub fn set(&mut self, other: Self) {
        self.state = other.state;
    }

    pub fn updateLiteral(&mut self) {
        if self.state <= SHORTREP_LIT_LIT {
            self.state = LIT_LIT;
        } else if self.state <= LIT_SHORTREP {
            self.state -= 3;
        } else {
            self.state -= 6;
        }
    }

    pub fn updateMatch(&mut self) {
        if self.state < LIT_STATES {
            self.state = LIT_LIT;
        } else {
            self.state = NONLIT_MATCH;
        }
    }

    pub fn updateLongRep(&mut self) {
        if self.state < LIT_STATES {
            self.state = LIT_LONGREP;
        } else {
            self.state = NONLIT_REP;
        }
    }

    pub fn updateShortRep(&mut self) {
        if self.state < LIT_STATES {
            self.state = LIT_SHORTREP;
        } else {
            self.state = NONLIT_REP;
        }
    }

    pub fn isLiteral(&mut self) -> bool {
        self.state < LIT_STATES
    }
}