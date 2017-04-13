use std::usize;
use std::ops::{Add, AddAssign, Sub, SubAssign};

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Weight {
    weight: usize,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Priority {
    priority: usize,
}

impl Weight {
    pub fn new(w: usize) -> Weight {
        Weight { weight: w }
    }

    pub fn zero() -> Weight {
        Weight::new(0)
    }

    pub fn max() -> Weight {
        Weight { weight: usize::MAX }
    }

    pub fn to_usize(self) -> usize {
        self.weight
    }

    pub fn is_max(self) -> bool {
        self.weight == usize::MAX
    }

    /// Returns a priority for tours with this weight; lighter tours
    /// have higher priority.
    pub fn to_priority(self) -> Priority {
        Priority { priority: usize::MAX - self.weight }
    }

    pub fn average(self, w: Weight) -> Weight {
        if self < w {
            let half_diff = (w.weight - self.weight) / 2;
            Weight::new(self.weight + half_diff)
        } else {
            w.average(self)
        }
    }
}

impl Add for Weight {
    type Output = Weight;

    fn add(self, rhs: Weight) -> Weight {
        Weight::new(self.weight + rhs.weight)
    }
}

impl AddAssign for Weight {
    fn add_assign(&mut self, rhs: Weight) {
        *self = *self + rhs;
    }
}

impl Sub for Weight {
    type Output = Weight;

    fn sub(self, rhs: Weight) -> Weight {
        Weight::new(self.weight - rhs.weight)
    }
}

impl SubAssign for Weight {
    fn sub_assign(&mut self, rhs: Weight) {
        *self = *self - rhs;
    }
}
