//! Some benchmarks stress-testing various ways to build a standard `Vec`.

mod util {
    use rayon::prelude::*;
    use std::collections::LinkedList;

    /// Do whatever `collect` does by default.
    pub fn collect<T, PI>(pi: PI) -> Vec<T>
    where
        T: Send,
        PI: ParallelIterator<Item = T> + Send,
    {
        pi.collect()
    }

    /// Collect a linked list of vectors intermediary.
    pub fn linked_list_collect_vec<T, PI>(pi: PI) -> Vec<T>
    where
        T: Send,
        PI: ParallelIterator<Item = T> + Send,
    {
        let list: LinkedList<Vec<_>> = pi
            .fold(Vec::new, |mut vec, elem| {
                vec.push(elem);
                vec
            })
            .collect();
        list.into_iter().fold(Vec::new(), |mut vec, mut sub| {
            vec.append(&mut sub);
            vec
        })
    }

    /// Collect a linked list of vectors intermediary, with a size hint.
    pub fn linked_list_collect_vec_sized<T, PI>(pi: PI) -> Vec<T>
    where
        T: Send,
        PI: ParallelIterator<Item = T> + Send,
    {
        let list: LinkedList<Vec<_>> = pi
            .fold(Vec::new, |mut vec, elem| {
                vec.push(elem);
                vec
            })
            .collect();

        let len = list.iter().map(Vec::len).sum();
        list.into_iter()
            .fold(Vec::with_capacity(len), |mut vec, mut sub| {
                vec.append(&mut sub);
                vec
            })
    }

    /// Map-Reduce a linked list of vectors intermediary, with a size hint.
    pub fn linked_list_map_reduce_vec_sized<T, PI>(pi: PI) -> Vec<T>
    where
        T: Send,
        PI: ParallelIterator<Item = T> + Send,
    {
        let list: LinkedList<Vec<_>> = pi
            .fold(Vec::new, |mut vec, elem| {
                vec.push(elem);
                vec
            })
            .map(|vec| {
                let mut list = LinkedList::new();
                list.push_back(vec);
                list
            })
            .reduce(LinkedList::new, |mut list1, mut list2| {
                list1.append(&mut list2);
                list1
            });

        let len = list.iter().map(Vec::len).sum();
        list.into_iter()
            .fold(Vec::with_capacity(len), |mut vec, mut sub| {
                vec.append(&mut sub);
                vec
            })
    }

    /// Map-Reduce a vector of vectors intermediary, with a size hint.
    pub fn vec_vec_sized<T, PI>(pi: PI) -> Vec<T>
    where
        T: Send,
        PI: ParallelIterator<Item = T> + Send,
    {
        let vecs: Vec<Vec<_>> = pi
            .fold(Vec::new, |mut vec, elem| {
                vec.push(elem);
                vec
            })
            .map(|v| vec![v])
            .reduce(Vec::new, |mut left, mut right| {
                left.append(&mut right);
                left
            });

        let len = vecs.iter().map(Vec::len).sum();
        vecs.into_iter()
            .fold(Vec::with_capacity(len), |mut vec, mut sub| {
                vec.append(&mut sub);
                vec
            })
    }

    /// Fold into vectors and then reduce them together.
    pub fn fold<T, PI>(pi: PI) -> Vec<T>
    where
        T: Send,
        PI: ParallelIterator<Item = T> + Send,
    {
        pi.fold(Vec::new, |mut vec, x| {
            vec.push(x);
            vec
        })
        .reduce(Vec::new, |mut vec1, mut vec2| {
            vec1.append(&mut vec2);
            vec1
        })
    }
}

macro_rules! make_bench {
    ($generate:ident, $check:ident) => {
        #[bench]
        fn with_collect(b: &mut ::test::Bencher) {
            use crate::vec_collect::util;
            let mut vec = None;
            b.iter(|| vec = Some(util::collect($generate())));
            $check(&vec.unwrap());
        }

        #[bench]
        fn with_linked_list_collect_vec(b: &mut ::test::Bencher) {
            use crate::vec_collect::util;
            let mut vec = None;
            b.iter(|| vec = Some(util::linked_list_collect_vec($generate())));
            $check(&vec.unwrap());
        }

        #[bench]
        fn with_linked_list_collect_vec_sized(b: &mut ::test::Bencher) {
            use crate::vec_collect::util;
            let mut vec = None;
            b.iter(|| vec = Some(util::linked_list_collect_vec_sized($generate())));
            $check(&vec.unwrap());
        }

        #[bench]
        fn with_linked_list_map_reduce_vec_sized(b: &mut ::test::Bencher) {
            use crate::vec_collect::util;
            let mut vec = None;
            b.iter(|| vec = Some(util::linked_list_map_reduce_vec_sized($generate())));
            $check(&vec.unwrap());
        }

        #[bench]
        fn with_vec_vec_sized(b: &mut ::test::Bencher) {
            use crate::vec_collect::util;
            let mut vec = None;
            b.iter(|| vec = Some(util::vec_vec_sized($generate())));
            $check(&vec.unwrap());
        }

        #[bench]
        fn with_fold(b: &mut ::test::Bencher) {
            use crate::vec_collect::util;
            let mut vec = None;
            b.iter(|| vec = Some(util::fold($generate())));
            $check(&vec.unwrap());
        }
    };
}

/// Tests a big vector of i forall i in 0 to N.
mod vec_i {
    use rayon::prelude::*;

    const N: u32 = 4 * 1024 * 1024;

    fn generate() -> impl IndexedParallelIterator<Item = u32> {
        (0_u32..N).into_par_iter()
    }

    fn check(v: &[u32]) {
        assert!(v.iter().cloned().eq(0..N));
    }

    #[bench]
    fn with_collect_into_vec(b: &mut ::test::Bencher) {
        let mut vec = None;
        b.iter(|| {
            let mut v = vec![];
            generate().collect_into_vec(&mut v);
            vec = Some(v);
        });
        check(&vec.unwrap());
    }

    #[bench]
    fn with_collect_into_vec_reused(b: &mut ::test::Bencher) {
        let mut vec = vec![];
        b.iter(|| generate().collect_into_vec(&mut vec));
        check(&vec);
    }

    make_bench!(generate, check);
}

/// Tests a big vector of i forall i in 0 to N, with a no-op
/// filter just to make sure it's not an exact iterator.
mod vec_i_filtered {
    use rayon::prelude::*;

    const N: u32 = 4 * 1024 * 1024;

    fn generate() -> impl ParallelIterator<Item = u32> {
        (0_u32..N).into_par_iter().filter(|_| true)
    }

    fn check(v: &[u32]) {
        assert!(v.iter().cloned().eq(0..N));
    }

    make_bench!(generate, check);
}
