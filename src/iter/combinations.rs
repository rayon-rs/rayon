use std::marker::PhantomData;
use std::ops::Div;
use std::sync::Arc;

use super::plumbing::*;
use super::*;

const OVERFLOW_MSG: &str = "Total number of combinations is too big.";

/// `Combinations` is an iterator that iterates over the k-length combinations
/// of the elements from the original iterator.
/// This struct is created by the [`combinations()`] method on [`ParallelIterator`]
///
/// [`combinations()`]: trait.ParallelIterator.html#method.combinations()
/// [`ParallelIterator`]: trait.ParallelIterator.html
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
#[derive(Debug, Clone)]
pub struct Combinations<I> {
    base: I,
    k: usize,
}

impl<I> Combinations<I> {
    /// Creates a new `Combinations` iterator.
    pub(super) fn new(base: I, k: usize) -> Self {
        Self { k, base }
    }
}

impl<I> ParallelIterator for Combinations<I>
where
    I: ParallelIterator,
    I::Item: Clone + Send + Sync,
{
    type Item = Vec<I::Item>;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        let producer = CombinationsProducer::<_, Vec<_>>::from_unindexed(self.base, self.k);
        bridge_unindexed(producer, consumer)
    }

    fn opt_len(&self) -> Option<usize> {
        // HACK: even though the length can be computed with the code below,
        // our implementation of drive_unindexed does not support indexed Consumer

        // self.base
        //     .opt_len()
        //     .and_then(|l| checked_binomial(l, self.k))
        None
    }
}

impl<I> IndexedParallelIterator for Combinations<I>
where
    I: IndexedParallelIterator,
    I::Item: Clone + Send + Sync,
{
    fn len(&self) -> usize {
        checked_binomial(self.base.len(), self.k).expect(OVERFLOW_MSG)
    }

    fn drive<C: Consumer<Self::Item>>(self, consumer: C) -> C::Result {
        bridge(self, consumer)
    }

    fn with_producer<CB: ProducerCallback<Self::Item>>(self, callback: CB) -> CB::Output {
        let producer = CombinationsProducer::<_, Vec<_>>::from_indexed(self.base, self.k);
        callback.callback(producer)
    }
}

/// `ArrayCombinations` is an iterator that iterates over the k-length combinations
/// of the elements from the original iterator.
/// This struct is created by the [`array_combinations()`] method on [`ParallelIterator`]
///
/// [`combinations()`]: trait.ParallelIterator.html#method.combinations()
/// [`ParallelIterator`]: trait.ParallelIterator.html
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
#[derive(Debug, Clone)]
pub struct ArrayCombinations<I, const K: usize> {
    base: I,
}

impl<I, const K: usize> ArrayCombinations<I, K> {
    /// Creates a new `Combinations` iterator.
    pub(super) fn new(base: I) -> Self {
        Self { base }
    }
}

impl<I, const K: usize> ParallelIterator for ArrayCombinations<I, K>
where
    I: ParallelIterator,
    I::Item: Clone + Send + Sync,
{
    type Item = Vec<I::Item>;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        let items: Arc<[I::Item]> = self.base.collect();
        let n = items.len();
        let until = checked_binomial(n, K).expect(OVERFLOW_MSG);
        let producer = CombinationsProducer {
            items,
            offset: 0,
            until,
            k: K,
            indices: PhantomData::<[usize; K]>,
        };
        bridge_unindexed(producer, consumer)
    }

    fn opt_len(&self) -> Option<usize> {
        // HACK: even though the length can be computed with the code below,
        // our implementation of drive_unindexed does not support indexed Consumer

        // self.base
        //     .opt_len()
        //     .and_then(|l| checked_binomial(l, self.k))
        None
    }
}

impl<I, const K: usize> IndexedParallelIterator for ArrayCombinations<I, K>
where
    I: IndexedParallelIterator,
    I::Item: Clone + Send + Sync,
{
    fn len(&self) -> usize {
        checked_binomial(self.base.len(), K).expect(OVERFLOW_MSG)
    }

    fn drive<C: Consumer<Self::Item>>(self, consumer: C) -> C::Result {
        bridge(self, consumer)
    }

    fn with_producer<CB: ProducerCallback<Self::Item>>(self, callback: CB) -> CB::Output {
        let producer = CombinationsProducer::<_, [_; K]>::from_indexed(self.base, K);
        callback.callback(producer)
    }
}

struct CombinationsProducer<T, I> {
    items: Arc<[T]>,
    offset: usize,
    until: usize,
    k: usize,
    indices: PhantomData<I>,
}

impl<T, I> CombinationsProducer<T, I> {
    fn new(items: Arc<[T]>, offset: usize, until: usize, k: usize) -> Self {
        Self {
            items,
            offset,
            until,
            k,
            indices: PhantomData,
        }
    }

    fn n(&self) -> usize {
        self.items.len()
    }

    fn len(&self) -> usize {
        self.until - self.offset
    }
}

impl<T, I> CombinationsProducer<T, I>
where
    T: Send,
{
    fn from_unindexed<B>(base: B, k: usize) -> Self
    where
        B: ParallelIterator<Item = T>,
    {
        let items: Arc<[T]> = base.collect();
        let n = items.len();
        let until = checked_binomial(n, k).expect(OVERFLOW_MSG);

        Self::new(items, 0, until, k)
    }

    fn from_indexed<B>(base: B, k: usize) -> Self
    where
        B: IndexedParallelIterator<Item = T>,
    {
        let n = base.len();
        let items = {
            let mut buffer = Vec::with_capacity(n);
            base.collect_into_vec(&mut buffer);
            buffer.into()
        };
        let until = checked_binomial(n, k).expect(OVERFLOW_MSG);

        Self::new(items, 0, until, k)
    }
}

impl<T, I> Producer for CombinationsProducer<T, I>
where
    T: Clone + Send + Sync,
    I: AsRef<[usize]> + AsMut<[usize]> + PartialEq + Clone + DefaultWithSize + Send,
{
    type Item = Vec<T>;
    type IntoIter = CombinationsSeq<T, I>;

    fn into_iter(self) -> Self::IntoIter {
        let n = self.n();
        CombinationsSeq::from_offsets(self.items, self.offset, self.until, n, self.k)
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let index = index + self.offset;

        (
            Self::new(Arc::clone(&self.items), self.offset, index, self.k),
            Self::new(self.items, index, self.until, self.k),
        )
    }
}

impl<T, I> UnindexedProducer for CombinationsProducer<T, I>
where
    T: Clone + Send + Sync,
    I: AsRef<[usize]> + AsMut<[usize]> + PartialEq + Clone + DefaultWithSize + Send,
{
    type Item = Vec<T>;

    fn split(self) -> (Self, Option<Self>) {
        if self.len() == 1 {
            return (self, None);
        }
        let mid_point = self.len() / 2;
        let (a, b) = self.split_at(mid_point);
        (a, Some(b))
    }

    fn fold_with<F>(self, folder: F) -> F
    where
        F: Folder<Self::Item>,
    {
        folder.consume_iter(self.into_iter())
    }
}

struct CombinationsSeq<T, I> {
    items: Arc<[T]>,
    indices: Indices<I>,
    until: Indices<I>,
    len: usize,
}

impl<T, I> CombinationsSeq<T, I>
where
    I: AsMut<[usize]> + DefaultWithSize,
{
    fn from_offsets(items: Arc<[T]>, from: usize, until: usize, n: usize, k: usize) -> Self {
        Self {
            indices: Indices::new(I::default_with_size(k), from, n),
            until: Indices::new(I::default_with_size(k), until, n),
            items,
            len: until - from,
        }
    }
}
impl<T, I> CombinationsSeq<T, I>
where
    I: AsRef<[usize]> + PartialEq,
{
    fn may_next(&self) -> Option<()> {
        if self.indices.is_empty() || self.until.is_empty() {
            // there are no indices to compute
            return None;
        }
        if self.indices == self.until {
            // reached the end
            return None;
        }

        Some(())
    }
}

impl<T, I> Iterator for CombinationsSeq<T, I>
where
    T: Clone,
    I: AsRef<[usize]> + AsMut<[usize]> + PartialEq + Clone,
{
    type Item = Vec<T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.may_next()?;
        let indices = self.indices.clone();
        self.indices.increment();
        Some(indices.deindex(&self.items))
    }
}

impl<T, I> ExactSizeIterator for CombinationsSeq<T, I>
where
    T: Clone,
    I: AsRef<[usize]> + AsMut<[usize]> + PartialEq + Clone,
{
    fn len(&self) -> usize {
        self.len
    }
}

impl<T, I> DoubleEndedIterator for CombinationsSeq<T, I>
where
    T: Clone,
    I: AsRef<[usize]> + AsMut<[usize]> + PartialEq + Clone,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.may_next()?;
        self.until.decrement();
        Some(self.until.deindex(&self.items))
    }
}

#[derive(PartialEq, Clone)]
struct Indices<I> {
    indices: I,
    n: usize,
    position: IndicesPosition,
}

#[derive(PartialEq, Clone, Copy)]
enum IndicesPosition {
    End,
    Middle,
}

impl<I> Indices<I>
where
    I: AsMut<[usize]>,
{
    fn new(mut indices: I, offset: usize, n: usize) -> Self {
        let k = indices.as_mut().len();
        let total = checked_binomial(n, k).expect(OVERFLOW_MSG);
        if offset == total {
            unrank(indices.as_mut(), offset - 1, n);
            Self {
                indices,
                n,
                position: IndicesPosition::End,
            }
        } else if offset < total {
            unrank(indices.as_mut(), offset, n);
            Self {
                indices,
                n,
                position: IndicesPosition::Middle,
            }
        } else {
            panic!("Offset should be inside the bounds of the possible combinations.")
        }
    }

    fn increment(&mut self) {
        match self.position {
            IndicesPosition::Middle => {
                if is_last(self.indices.as_mut(), self.n) {
                    self.position = IndicesPosition::End
                } else {
                    increment_indices(self.indices.as_mut(), self.n);
                }
            }
            IndicesPosition::End => panic!("No more indices"),
        }
    }

    fn decrement(&mut self) {
        match self.position {
            IndicesPosition::End => self.position = IndicesPosition::Middle,
            IndicesPosition::Middle => decrement_indices(self.indices.as_mut(), self.n),
        }
    }
}

impl<I> Indices<I>
where
    I: AsRef<[usize]>,
{
    fn deindex<T: Clone>(&self, items: &[T]) -> Vec<T> {
        match self.position {
            IndicesPosition::Middle => self
                .indices
                .as_ref()
                .iter()
                .map(|i| &items[*i])
                .cloned()
                .collect(),
            IndicesPosition::End => panic!("End position indices cannot be deindexed."),
        }
    }

    fn is_empty(&self) -> bool {
        self.indices.as_ref().is_empty()
    }
}

/// Calculates the binomial coefficient for given `n` and `k`.
///
/// The binomial coefficient, often denoted as C(n, k) or "n choose k", represents
/// the number of ways to choose `k` elements from a set of `n` elements without regard
/// to the order of selection.
///
/// # Overflow
/// Note that this function will overflow even if the result is smaller than [`isize::MAX`].
/// The guarantee is that it does not overflow if `nCk * k <= isize::MAX`.
fn checked_binomial(n: usize, k: usize) -> Option<usize> {
    if k > n {
        return Some(0);
    }

    // nCk is symmetric around (n-k), so choose the smallest to iterate faster
    let k = std::cmp::min(k, n - k);

    // Fast paths x2 ~ x6 speedup
    match k {
        1 => return Some(n),
        2 => return n.checked_mul(n - 1).map(|r| r / 2),
        3 => {
            return n
                .checked_mul(n - 1)?
                .div(2)
                .checked_mul(n - 2)
                .map(|r| r / 3)
        }
        4 if n <= 77_937 /* prevent unnecessary overflow */ => {
            return n
                .checked_mul(n - 1)?
                .div(2)
                .checked_mul(n - 2)?
                .checked_mul(n - 3)
                .map(|r| r / 12)
        }
        5 if n <= 10_811 /* prevent unnecessary overflow */ => {
            return n
                .checked_mul(n - 1)?
                .div(2)
                .checked_mul(n - 2)?
                .checked_mul(n - 3)?
                .div(4)
                .checked_mul(n - 4)
                .map(|r| r / 15)
        }
        _ => {}
    }

    // `factorial(n) / factorial(n - k) / factorial(k)` but trying to avoid overflows
    let mut result: usize = 1;
    for i in 0..k {
        result = result.checked_mul(n - i)?;
        result /= i + 1;
    }
    Some(result)
}

/// The indices corresponding to a given `offset`
/// in the corresponding ordered list of possible combinations of `n` choose `k`.
///
/// # Panics
/// - If n < k: not enough elements to form a list of indices
/// - If the total number of combinations of `n` choose `k` is too big, roughly if it does not fit in usize.
fn unrank(indices: &mut [usize], offset: usize, n: usize) {
    // Source:
    //   Antoine Genitrini, Martin Pépin. Lexicographic unranking of combinations revisited.
    //   Algorithms, 2021, 14 (3), pp.97. 10.3390/a14030097. hal-03040740v2

    let k = indices.len();

    assert!(
        n >= k,
        "Not enough elements to choose k from. Note that n must be at least k. Called with n={n}, k={k}"
    );

    debug_assert!(offset < checked_binomial(n, k).unwrap());

    if k == 1 {
        indices[0] = offset;
        return;
    }

    let mut b = checked_binomial(n - 1, k - 1).expect(OVERFLOW_MSG);
    let mut m = 0;
    let mut i = 0;
    let mut u = offset;

    let mut first = true;

    while i < k - 1 {
        if first {
            first = false;
        } else {
            b /= n - m - i;
        }

        if b > u {
            indices[i] = m + i;
            b *= k - i - 1;
            i += 1;
        } else {
            u -= b;
            b *= n - m - k;
            m += 1;
        }
    }

    if k > 0 {
        indices[k - 1] = n + u - b;
    }
}

/// Increments indices representing the combination to advance to the next
/// (in lexicographic order by increasing sequence) combination. For example
/// if we have n=4 & k=2 then `[0, 1] -> [0, 2] -> [0, 3] -> [1, 2] -> ...`
fn increment_indices(indices: &mut [usize], n: usize) {
    if indices.is_empty() {
        return;
    }

    let k = indices.len();

    // Scan from the end, looking for an index to increment
    let mut i: usize = indices.len() - 1;

    while indices[i] == i + n - k {
        debug_assert!(i > 0);
        i -= 1;
    }

    // Increment index, and reset the ones to its right
    indices[i] += 1;
    for j in i + 1..indices.len() {
        indices[j] = indices[j - 1] + 1;
    }
}

fn decrement_indices(indices: &mut [usize], n: usize) {
    if indices.is_empty() {
        return;
    }

    let k = indices.len();

    if k == 1 {
        indices[0] -= 1;
        return;
    }

    let mut i = k - 1;

    while (i > 0) && indices[i] == indices[i - 1] + 1 {
        debug_assert!(i > 0);
        i -= 1;
    }

    // Decrement index, and reset the ones to its right
    indices[i] -= 1;
    for j in i + 1..indices.len() {
        indices[j] = n - k + j;
    }
}

fn is_last(indices: &[usize], n: usize) -> bool {
    let k = indices.len();

    // BENCH: vs == ((n-k)..n).collect()
    indices
        .iter()
        .enumerate()
        .all(|(i, &index)| index == n - k + i)
}

trait DefaultWithSize {
    fn default_with_size(size: usize) -> Self;
}

impl<T> DefaultWithSize for Vec<T>
where
    T: Default + Copy,
{
    fn default_with_size(size: usize) -> Self {
        vec![T::default(); size]
    }
}

impl<T, const K: usize> DefaultWithSize for [T; K]
where
    T: Default + Copy,
{
    fn default_with_size(size: usize) -> Self {
        assert_eq!(size, K);
        [T::default(); K]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unrank_vec(offset: usize, n: usize, k: usize) -> Vec<usize> {
        let mut indices = vec![0; k];
        unrank(&mut indices, offset, n);
        indices
    }

    fn unrank_const<const K: usize>(offset: usize, n: usize) -> [usize; K] {
        let mut indices = [0; K];
        unrank(&mut indices, offset, n);
        indices
    }

    #[test]
    fn checked_binomial_works() {
        assert_eq!(checked_binomial(7, 2), Some(21));
        assert_eq!(checked_binomial(8, 2), Some(28));
        assert_eq!(checked_binomial(11, 3), Some(165));
        assert_eq!(checked_binomial(11, 4), Some(330));
        assert_eq!(checked_binomial(64, 2), Some(2016));
        assert_eq!(checked_binomial(64, 7), Some(621_216_192));
    }

    #[test]
    fn checked_binomial_k_greater_than_n() {
        assert_eq!(checked_binomial(5, 6), Some(0));
        assert_eq!(checked_binomial(3, 4), Some(0));
    }

    #[test]
    fn test_binom_symmetry() {
        assert_eq!(checked_binomial(10, 2), checked_binomial(10, 8));
        assert_eq!(checked_binomial(103, 2), checked_binomial(103, 101));
    }

    #[test]
    fn checked_binomial_repeated_values() {
        assert_eq!(checked_binomial(1000, 1000), Some(1));
        assert_eq!(checked_binomial(2048, 2048), Some(1));
        assert_eq!(checked_binomial(1000, 0), Some(1));
        assert_eq!(checked_binomial(2048, 0), Some(1));
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn checked_binomial_not_overflows() {
        assert_eq!(
            checked_binomial(4_294_967_296, 2),
            Some(9_223_372_034_707_292_160)
        );
        assert_eq!(
            checked_binomial(3_329_022, 3),
            Some(6_148_913_079_097_324_540)
        );
        assert_eq!(
            checked_binomial(102_570, 4),
            Some(4_611_527_207_790_103_770)
        );
        assert_eq!(checked_binomial(13_467, 5), Some(3_688_506_309_678_005_178));
        assert_eq!(checked_binomial(109, 15), Some(1_015_428_940_004_440_080));
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn checked_overflows() {
        assert_eq!(checked_binomial(109, 16), None);
    }

    #[test]
    fn checked_binomial_identity() {
        assert_eq!(checked_binomial(1, 1), Some(1));
        assert_eq!(checked_binomial(100, 1), Some(100));
        assert_eq!(checked_binomial(25_000, 1), Some(25_000));
        assert_eq!(checked_binomial(25_000, 0), Some(1));
    }

    #[test]
    fn unrank_5_2() {
        assert_eq!(unrank_vec(0, 5, 2), vec![0, 1]);
        assert_eq!(unrank_vec(1, 5, 2), vec![0, 2]);
        assert_eq!(unrank_vec(2, 5, 2), vec![0, 3]);
        assert_eq!(unrank_vec(3, 5, 2), vec![0, 4]);
        assert_eq!(unrank_vec(4, 5, 2), vec![1, 2]);
        assert_eq!(unrank_vec(5, 5, 2), vec![1, 3]);
        assert_eq!(unrank_vec(6, 5, 2), vec![1, 4]);
        assert_eq!(unrank_vec(7, 5, 2), vec![2, 3]);
        assert_eq!(unrank_vec(8, 5, 2), vec![2, 4]);
        assert_eq!(unrank_vec(9, 5, 2), vec![3, 4]);
    }

    #[test]
    fn unrank_zero_index() {
        for k in 1..7 {
            for n in k..11 {
                dbg!(n, k);
                assert_eq!(unrank_vec(0, n, k), (0..k).collect::<Vec<_>>());
            }
        }
    }

    #[test]
    fn unrank_one_index() {
        for n in 4..50 {
            for k in 1..n {
                let mut expected: Vec<_> = (0..k).collect();
                *expected.last_mut().unwrap() += 1;
                assert_eq!(unrank_vec(1, n, k), expected);
            }
        }
    }

    #[test]
    fn unrank_last() {
        for n in 4..50 {
            for k in 1..n {
                let expected: Vec<_> = ((n - k)..n).collect();
                let offset = checked_binomial(n, k).unwrap() - 1;
                assert_eq!(unrank_vec(offset, n, k), expected);
            }
        }
    }

    #[test]
    fn unrank_k1() {
        for n in 1..50 {
            for offset in 0..n {
                assert_eq!(unrank_const(offset, n), [offset]);
            }
        }
    }

    #[test]
    fn increment_indices_rank_consistent() {
        for i in 0..16 {
            for n in (i + 2)..(2 * (i + 1)) {
                for k in 1..n {
                    let mut indices = unrank_vec(i, n, k);
                    increment_indices(&mut indices, n);
                    assert_eq!(unrank_vec(i + 1, n, k), indices);
                }
            }
        }
    }

    #[test]
    fn increment_indices_rank_consistent_last_indices() {
        for n in 2..15 {
            for k in 1..n {
                let max_iter = checked_binomial(n, k).unwrap();
                for i in (max_iter - n / 2)..(max_iter - 1) {
                    let mut indices = unrank_vec(i, n, k);
                    increment_indices(&mut indices, n);
                    assert_eq!(unrank_vec(i + 1, n, k), indices);
                }
            }
        }
    }

    #[test]
    fn decrement_indices_rank_consistent() {
        for i in 1..16 {
            for n in (i + 2)..(2 * (i + 1)) {
                for k in 1..n {
                    let mut indices = unrank_vec(i, n, k);
                    decrement_indices(&mut indices, n);
                    assert_eq!(unrank_vec(i - 1, n, k), indices);
                }
            }
        }
    }

    #[test]
    fn decrement_indices_rank_consistent_last_indices() {
        for n in 2..15 {
            for k in 1..n {
                let max_iter = checked_binomial(n, k).unwrap();
                for i in (max_iter - n / 2)..(max_iter) {
                    let mut indices = unrank_vec(i, n, k);
                    decrement_indices(&mut indices, n);
                    assert_eq!(unrank_vec(i - 1, n, k), indices);
                }
            }
        }
    }

    #[test]
    #[should_panic]
    fn unrank_offset_outofbounds() {
        let n = 10;
        const K: usize = 2;
        let u = checked_binomial(n, K).unwrap() + 1;
        unrank_const::<K>(u, n);
    }

    #[test]
    fn par_iter_works() {
        let a: Vec<_> = (0..5).into_par_iter().combinations(2).collect();

        // FIXME: somewhere `until` is not correctly set
        assert_eq!(
            a,
            vec![
                vec![0, 1],
                vec![0, 2],
                vec![0, 3],
                vec![0, 4],
                vec![1, 2],
                vec![1, 3],
                vec![1, 4],
                vec![2, 3],
                vec![2, 4],
                vec![3, 4],
            ]
        );

        let b: Vec<_> = (0..5).into_par_iter().combinations(3).collect();

        assert_eq!(
            b,
            vec![
                vec![0, 1, 2],
                vec![0, 1, 3],
                vec![0, 1, 4],
                vec![0, 2, 3],
                vec![0, 2, 4],
                vec![0, 3, 4],
                vec![1, 2, 3],
                vec![1, 2, 4],
                vec![1, 3, 4],
                vec![2, 3, 4],
            ]
        );
    }

    #[test]
    fn par_iter_k1() {
        assert_eq!(
            (0..200).into_par_iter().combinations(1).collect::<Vec<_>>(),
            (0..200).map(|i| vec![i]).collect::<Vec<_>>()
        )
    }

    #[test]
    fn par_iter_rev_works() {
        let a: Vec<_> = (0..5).into_par_iter().combinations(2).rev().collect();
        assert_eq!(
            a,
            vec![
                vec![0, 1],
                vec![0, 2],
                vec![0, 3],
                vec![0, 4],
                vec![1, 2],
                vec![1, 3],
                vec![1, 4],
                vec![2, 3],
                vec![2, 4],
                vec![3, 4],
            ]
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
        );

        let b: Vec<_> = (0..5).into_par_iter().combinations(3).rev().collect();
        assert_eq!(
            b,
            vec![
                vec![0, 1, 2],
                vec![0, 1, 3],
                vec![0, 1, 4],
                vec![0, 2, 3],
                vec![0, 2, 4],
                vec![0, 3, 4],
                vec![1, 2, 3],
                vec![1, 2, 4],
                vec![1, 3, 4],
                vec![2, 3, 4],
            ]
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn array_par_iter_works() {
        let a: Vec<_> = (0..5).into_par_iter().array_combinations::<2>().collect();

        // FIXME: somewhere `until` is not correctly set
        assert_eq!(
            a,
            vec![
                [0, 1],
                [0, 2],
                [0, 3],
                [0, 4],
                [1, 2],
                [1, 3],
                [1, 4],
                [2, 3],
                [2, 4],
                [3, 4],
            ]
        );

        let b: Vec<_> = (0..5).into_par_iter().array_combinations::<3>().collect();

        assert_eq!(
            b,
            vec![
                [0, 1, 2],
                [0, 1, 3],
                [0, 1, 4],
                [0, 2, 3],
                [0, 2, 4],
                [0, 3, 4],
                [1, 2, 3],
                [1, 2, 4],
                [1, 3, 4],
                [2, 3, 4],
            ]
        );
    }

    #[test]
    fn array_par_iter_k1() {
        assert_eq!(
            (0..200)
                .into_par_iter()
                .array_combinations::<1>()
                .collect::<Vec<_>>(),
            (0..200).map(|i| [i]).collect::<Vec<_>>()
        )
    }

    #[test]
    fn array_par_iter_rev_works() {
        let a: Vec<_> = (0..5)
            .into_par_iter()
            .array_combinations::<2>()
            .rev()
            .collect();
        assert_eq!(
            a,
            vec![
                [0, 1],
                [0, 2],
                [0, 3],
                [0, 4],
                [1, 2],
                [1, 3],
                [1, 4],
                [2, 3],
                [2, 4],
                [3, 4],
            ]
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
        );

        let b: Vec<_> = (0..5)
            .into_par_iter()
            .array_combinations::<3>()
            .rev()
            .collect();
        assert_eq!(
            b,
            vec![
                [0, 1, 2],
                [0, 1, 3],
                [0, 1, 4],
                [0, 2, 3],
                [0, 2, 4],
                [0, 3, 4],
                [1, 2, 3],
                [1, 2, 4],
                [1, 3, 4],
                [2, 3, 4],
            ]
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
        );
    }
}
