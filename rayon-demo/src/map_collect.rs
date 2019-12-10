//! Some benchmarks stress-testing various ways to build the standard
//! `HashMap` data structures from the standard library.

mod util {
    use rayon::prelude::*;
    use std::collections::{HashMap, LinkedList};
    use std::hash::Hash;
    use std::iter::FromIterator;
    use std::sync::Mutex;

    /// Do whatever `collect` does by default.
    pub fn collect<K, V, PI>(pi: PI) -> HashMap<K, V>
    where
        K: Send + Hash + Eq,
        V: Send,
        PI: ParallelIterator<Item = (K, V)> + Send,
    {
        pi.collect()
    }

    /// Use a system mutex.
    pub fn mutex<K, V, PI>(pi: PI) -> HashMap<K, V>
    where
        K: Send + Hash + Eq,
        V: Send,
        PI: ParallelIterator<Item = (K, V)> + Send,
    {
        let mutex = Mutex::new(HashMap::new());
        pi.for_each(|(k, v)| {
            let mut guard = mutex.lock().unwrap();
            guard.insert(k, v);
        });
        mutex.into_inner().unwrap()
    }

    /// Use a system mutex over a folded vec.
    pub fn mutex_vec<K, V, PI>(pi: PI) -> HashMap<K, V>
    where
        K: Send + Hash + Eq,
        V: Send,
        PI: ParallelIterator<Item = (K, V)> + Send,
    {
        let mutex = Mutex::new(HashMap::new());
        pi.fold(Vec::new, |mut vec, elem| {
            vec.push(elem);
            vec
        })
        .for_each(|vec| {
            let mut guard = mutex.lock().unwrap();
            guard.extend(vec);
        });
        mutex.into_inner().unwrap()
    }

    /// Collect a linked list intermediary.
    pub fn linked_list_collect<K, V, PI>(pi: PI) -> HashMap<K, V>
    where
        K: Send + Hash + Eq,
        V: Send,
        PI: ParallelIterator<Item = (K, V)> + Send,
    {
        let list: LinkedList<(_, _)> = pi.collect();
        list.into_iter().collect()
    }

    /// Collect a linked list of vectors intermediary.
    pub fn linked_list_collect_vec<K, V, PI>(pi: PI) -> HashMap<K, V>
    where
        K: Send + Hash + Eq,
        V: Send,
        PI: ParallelIterator<Item = (K, V)> + Send,
    {
        let list: LinkedList<Vec<(_, _)>> = pi
            .fold(Vec::new, |mut vec, elem| {
                vec.push(elem);
                vec
            })
            .collect();
        list.into_iter().fold(HashMap::new(), |mut map, vec| {
            map.extend(vec);
            map
        })
    }

    /// Collect a linked list of vectors intermediary, with a size hint.
    pub fn linked_list_collect_vec_sized<K, V, PI>(pi: PI) -> HashMap<K, V>
    where
        K: Send + Hash + Eq,
        V: Send,
        PI: ParallelIterator<Item = (K, V)> + Send,
    {
        let list: LinkedList<Vec<(_, _)>> = pi
            .fold(Vec::new, |mut vec, elem| {
                vec.push(elem);
                vec
            })
            .collect();

        let len = list.iter().map(Vec::len).sum();
        list.into_iter()
            .fold(HashMap::with_capacity(len), |mut map, vec| {
                map.extend(vec);
                map
            })
    }

    /// Map-Reduce a linked list of vectors intermediary, with a size hint.
    pub fn linked_list_map_reduce_vec_sized<K, V, PI>(pi: PI) -> HashMap<K, V>
    where
        K: Send + Hash + Eq,
        V: Send,
        PI: ParallelIterator<Item = (K, V)> + Send,
    {
        let list: LinkedList<Vec<(_, _)>> = pi
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
            .fold(HashMap::with_capacity(len), |mut map, vec| {
                map.extend(vec);
                map
            })
    }

    /// Map-Reduce a vector of vectors intermediary, with a size hint.
    pub fn vec_vec_sized<K, V, PI>(pi: PI) -> HashMap<K, V>
    where
        K: Send + Hash + Eq,
        V: Send,
        PI: ParallelIterator<Item = (K, V)> + Send,
    {
        let vecs: Vec<Vec<(_, _)>> = pi
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
            .fold(HashMap::with_capacity(len), |mut map, vec| {
                map.extend(vec);
                map
            })
    }

    /// Fold into hashmaps and then reduce them together.
    pub fn fold<K, V, PI>(pi: PI) -> HashMap<K, V>
    where
        K: Send + Hash + Eq,
        V: Send,
        PI: ParallelIterator<Item = (K, V)> + Send,
    {
        pi.fold(HashMap::new, |mut map, (k, v)| {
            map.insert(k, v);
            map
        })
        .reduce(HashMap::new, |mut map1, mut map2| {
            if map1.len() > map2.len() {
                map1.extend(map2);
                map1
            } else {
                map2.extend(map1);
                map2
            }
        })
    }

    /// Fold into vecs and then reduce them together as hashmaps.
    pub fn fold_vec<K, V, PI>(pi: PI) -> HashMap<K, V>
    where
        K: Send + Hash + Eq,
        V: Send,
        PI: ParallelIterator<Item = (K, V)> + Send,
    {
        pi.fold(Vec::new, |mut vec, elem| {
            vec.push(elem);
            vec
        })
        .map(HashMap::from_iter)
        .reduce(HashMap::new, |mut map1, map2| {
            map1.extend(map2);
            map1
        })
    }
}

macro_rules! make_bench {
    ($generate:ident, $check:ident) => {
        #[bench]
        fn with_collect(b: &mut ::test::Bencher) {
            use crate::map_collect::util;
            let mut map = None;
            b.iter(|| map = Some(util::collect($generate())));
            $check(&map.unwrap());
        }

        #[bench]
        fn with_mutex(b: &mut ::test::Bencher) {
            use crate::map_collect::util;
            let mut map = None;
            b.iter(|| map = Some(util::mutex($generate())));
            $check(&map.unwrap());
        }

        #[bench]
        fn with_mutex_vec(b: &mut ::test::Bencher) {
            use crate::map_collect::util;
            let mut map = None;
            b.iter(|| map = Some(util::mutex_vec($generate())));
            $check(&map.unwrap());
        }

        #[bench]
        fn with_linked_list_collect(b: &mut ::test::Bencher) {
            use crate::map_collect::util;
            let mut map = None;
            b.iter(|| map = Some(util::linked_list_collect($generate())));
            $check(&map.unwrap());
        }

        #[bench]
        fn with_linked_list_collect_vec(b: &mut ::test::Bencher) {
            use crate::map_collect::util;
            let mut map = None;
            b.iter(|| map = Some(util::linked_list_collect_vec($generate())));
            $check(&map.unwrap());
        }

        #[bench]
        fn with_linked_list_collect_vec_sized(b: &mut ::test::Bencher) {
            use crate::map_collect::util;
            let mut map = None;
            b.iter(|| map = Some(util::linked_list_collect_vec_sized($generate())));
            $check(&map.unwrap());
        }

        #[bench]
        fn with_linked_list_map_reduce_vec_sized(b: &mut ::test::Bencher) {
            use crate::map_collect::util;
            let mut map = None;
            b.iter(|| map = Some(util::linked_list_map_reduce_vec_sized($generate())));
            $check(&map.unwrap());
        }

        #[bench]
        fn with_vec_vec_sized(b: &mut ::test::Bencher) {
            use crate::map_collect::util;
            let mut map = None;
            b.iter(|| map = Some(util::vec_vec_sized($generate())));
            $check(&map.unwrap());
        }

        #[bench]
        fn with_fold(b: &mut ::test::Bencher) {
            use crate::map_collect::util;
            let mut map = None;
            b.iter(|| map = Some(util::fold($generate())));
            $check(&map.unwrap());
        }

        #[bench]
        fn with_fold_vec(b: &mut ::test::Bencher) {
            use crate::map_collect::util;
            let mut map = None;
            b.iter(|| map = Some(util::fold_vec($generate())));
            $check(&map.unwrap());
        }
    };
}

/// Tests a big map mapping `i -> i` forall i in 0 to N. This map is
/// interesting because it has no conflicts, so each parallel
/// iteration adds a distinct entry into the map.
mod i_to_i {
    use rayon::prelude::*;
    use std::collections::HashMap;

    const N: u32 = 256 * 1024;

    fn generate() -> impl ParallelIterator<Item = (u32, u32)> {
        (0_u32..N).into_par_iter().map(|i| (i, i))
    }

    fn check(hm: &HashMap<u32, u32>) {
        assert_eq!(hm.len(), N as usize);
        for i in 0..N {
            assert_eq!(hm[&i], i);
        }
    }

    make_bench!(generate, check);
}

/// Tests a big map mapping `i % 10 -> i` forall i in 0 to N. This map
/// is interesting because it has lots of conflicts, so parallel
/// iterations sometimes overwrite entries.
mod i_mod_10_to_i {
    use rayon::prelude::*;
    use std::collections::HashMap;

    const N: u32 = 256 * 1024;

    fn generate() -> impl ParallelIterator<Item = (u32, u32)> {
        (0_u32..N).into_par_iter().map(|i| (i % 10, i))
    }

    fn check(hm: &HashMap<u32, u32>) {
        assert_eq!(hm.len(), 10);
        for (&k, &v) in hm {
            assert_eq!(k, v % 10);
        }
    }

    make_bench!(generate, check);
}
