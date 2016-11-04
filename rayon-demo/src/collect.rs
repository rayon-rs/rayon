//! Simple benchmarks of `collect_map()` performance

mod util {
    use rayon::prelude::*;
    use std::collections::{LinkedList, HashMap};
    use std::hash::Hash;
    use std::sync::Mutex;

    pub fn flat_combine<K: Send + Hash + Eq, V: Send, PI: ParallelIterator<Item=(K, V)> + Send>(pi: PI) -> HashMap<K, V> {
        pi.collect()
    }

    pub fn mutex<K: Send + Hash + Eq, V: Send, PI: ParallelIterator<Item=(K, V)>>(pi: PI) -> HashMap<K, V> {
        let mutex = Mutex::new(HashMap::new());
        pi.for_each(|(k, v)| {
            let mut guard = mutex.lock().unwrap();
            guard.insert(k, v);
        });
        mutex.into_inner().unwrap()
    }

    pub fn linked_list<K: Send + Hash + Eq, V: Send, PI: ParallelIterator<Item=(K, V)>>(pi: PI) -> HashMap<K, V> {
        let list: LinkedList<(_, _)> = pi.collect();
        list.into_iter().collect()
    }
}

/// Tests a big map mapping `i -> i` forall i in 0 to N. This map is
/// interesting because it has no conflicts, so each parallel
/// iteration adds a distinct entry into the map.
mod i_to_i {
    use rayon::prelude::*;
    use test::Bencher;
    use std::collections::{HashMap};
    use super::util;

    const N: u32 = 256*1024;

    fn generate() -> impl ParallelIterator<Item=(u32, u32)> {
        (0_u32..N)
            .into_par_iter()
            .map(|i| (i, i))
    }

    fn check(hm: &HashMap<u32, u32>) {
        assert_eq!(hm.len(), N as usize);
        for i in 0..N {
            assert_eq!(hm[&i], i);
        }
    }

    #[bench]
    fn with_flat_combine(b: &mut Bencher) {
        let mut map = None;
        b.iter(|| map = Some(util::flat_combine(generate())));
        check(&map.unwrap());
    }

    #[bench]
    fn with_mutex(b: &mut Bencher) {
        let mut map = None;
        b.iter(|| map = Some(util::mutex(generate())));
        check(&map.unwrap());
    }

    #[bench]
    fn with_linked_list(b: &mut Bencher) {
        let mut map = None;
        b.iter(|| map = Some(util::linked_list(generate())));
        check(&map.unwrap());
    }
}

/// Tests a big map mapping `i % 10 -> i` forall i in 0 to N. This map
/// is interesting because it has lots of conflicts, so parallel
/// iterations sometimes overwrite entries.
mod i_mod_10_to_i {
    use rayon::prelude::*;
    use test::Bencher;
    use std::collections::{HashMap};
    use super::util;

    const N: u32 = 256*1024;

    fn generate() -> impl ParallelIterator<Item=(u32, u32)> {
        (0_u32..N)
            .into_par_iter()
            .map(|i| (i % 10, i))
    }

    fn check(hm: &HashMap<u32, u32>) {
        assert_eq!(hm.len(), 10);
        for (&k, &v) in hm {
            assert_eq!(k, v % 10);
        }
    }

    #[bench]
    fn with_flat_combine(b: &mut Bencher) {
        let mut map = None;
        b.iter(|| map = Some(util::flat_combine(generate())));
        check(&map.unwrap());
    }

    #[bench]
    fn with_mutex(b: &mut Bencher) {
        let mut map = None;
        b.iter(|| map = Some(util::mutex(generate())));
        check(&map.unwrap());
    }

    #[bench]
    fn with_linked_list(b: &mut Bencher) {
        let mut map = None;
        b.iter(|| map = Some(util::linked_list(generate())));
        check(&map.unwrap());
    }
}

