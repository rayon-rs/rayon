/// An internal rope-like data structure. Not published publicly
/// because it's too embarassing -- but simple, and handy enough.
pub struct VecTree<T> {
    data: VecTreeData<T>,
}

enum VecTreeData<T> {
    Leaf(Vec<T>),
    Branch {
        left: Box<VecTree<T>>,
        left_len: usize,
        right: Box<VecTree<T>>,
        right_len: usize,
    },
}

impl<T> VecTree<T> {
    pub fn new() -> Self {
        VecTree { data: VecTreeData::Leaf(vec![]) }
    }

    pub fn leaf(vec: Vec<T>) -> Self {
        VecTree { data: VecTreeData::Leaf(vec) }
    }

    pub fn branch(left: VecTree<T>, right: VecTree<T>) -> Self {
        let left_len = left.len();
        let right_len = right.len();

        if left_len == 0 {
            right
        } else if right_len == 0 {
            left
        } else {
            VecTree {
                data: VecTreeData::Branch {
                    left: Box::new(left),
                    left_len: left_len,
                    right: Box::new(right),
                    right_len: right_len,
                },
            }
        }
    }

    pub fn len(&self) -> usize {
        match self.data {
            VecTreeData::Leaf(ref v) => v.len(),
            VecTreeData::Branch { left_len, right_len, .. } => left_len + right_len,
        }
    }

    pub fn foreach<F>(self, op: &mut F)
        where F: FnMut(T)
    {
        match self.data {
            VecTreeData::Leaf(vec) => {
                for elem in vec {
                    op(elem);
                }
            }
            VecTreeData::Branch { left, right, .. } => {
                left.foreach(op);
                right.foreach(op);
            }
        }
    }
}
