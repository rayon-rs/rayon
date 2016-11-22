#[derive(Copy, Clone)]
pub struct ParallelLen {
    /// Maximal number of elements that we will write
    pub maximal_len: usize,

    /// An estimate of the "cost" of this operation. This is a kind of
    /// abstract concept you can use to influence how fine-grained the
    /// threads are.
    ///
    /// TODO: refine this metric.
    pub cost: f64,

    /// If true, all elements will be written. If false, some may not.
    /// For example, `sparse` will be false if there is a filter.
    /// When doing a collect, sparse iterators require a compression
    /// step.
    pub sparse: bool,
}

impl ParallelLen {
    pub fn left_cost(&self, mid: usize) -> ParallelLen {
        ParallelLen {
            maximal_len: mid,
            cost: self.cost / 2.0,
            sparse: self.sparse,
        }
    }

    pub fn right_cost(&self, mid: usize) -> ParallelLen {
        ParallelLen {
            maximal_len: self.maximal_len - mid,
            cost: self.cost / 2.0,
            sparse: self.sparse,
        }
    }
}

// The threshold cost where it is worth falling back to sequential.
// This may be tweaked over time!
pub const THRESHOLD: f64 = 10. * 1024.0;

// The default is to assume that each function we execute (e.g., map,
// filter) takes an additional 5% of time per item.
pub const FUNC_ADJUSTMENT: f64 = 1.05;
