use super::plumbing::*;
use super::*;
use std::usize;
use std::collections::LinkedList;

pub(super) fn scan<PI, P, T>(pi: PI, scan_op: P, id: T) -> Scan<T, P>
where
    PI: ParallelIterator<Item = T>,
    P: Fn(&T, &T) -> T + Send + Sync,
    T: Send + Sync,
{
    let list = scan_p1(pi, &scan_op);
    let data = list.into_iter().collect();
    let offsets = compute_offsets(&data, &scan_op, id);
    Scan::new(data, offsets, scan_op)
}

// Compute the offset for each chunk by performing another sequential scan
// on the last value of each chunk
fn compute_offsets<'a, P, T>(data: &Vec<Vec<T>>, scan_op: &'a P, id: T) -> Vec<T>
where
    P: Fn(&T, &T) -> T,
{
    let mut offsets: Vec<T> = Vec::with_capacity(data.len());
    offsets.push(id);

    for it in data {
        // offsets is never empty because we already pushed id to it
        let last = offsets.last().unwrap();
        // `it` can never be empty due to implementation of ScanP1Folder
        let next: T = (scan_op)(last, &it.last().unwrap());
        offsets.push(next);
    }
    offsets
}

/******* scan part 1: consumer ******/

// Breaks the iterator into pieces and performs sequential scan on each
// Returns intermediate data, a LinkedList of the result of each seq scan
fn scan_p1<'a, PI, P, T>(pi: PI, scan_op: &'a P) -> LinkedList<Vec<T>>
where
    PI: ParallelIterator<Item = T>,
    P: Fn(&T, &T) -> T + Send + Sync,
    T: Send,
{
    let consumer = ScanP1Consumer { scan_op };
    pi.drive_unindexed(consumer)
}

struct ScanP1Consumer<'p, P> {
    scan_op: &'p P,
}

impl<'p, P: Send> ScanP1Consumer<'p, P> {
    fn new(scan_op: &'p P) -> ScanP1Consumer<'p, P> {
        ScanP1Consumer { scan_op }
    }
}

impl<'p, T, P: 'p> Consumer<T> for ScanP1Consumer<'p, P>
where
    T: Send,
    P: Fn(&T, &T) -> T + Send + Sync,
{
    type Folder = ScanP1Folder<'p, T, P>;
    type Reducer = ScanP1Reducer;
    type Result = LinkedList<Vec<T>>;

    fn split_at(self, _index: usize) -> (Self, Self, Self::Reducer) {
        (
            ScanP1Consumer::new(self.scan_op),
            ScanP1Consumer::new(self.scan_op),
            ScanP1Reducer,
        )
    }

    fn into_folder(self) -> Self::Folder {
        ScanP1Folder {
            vec: Vec::new(),
            scan_op: self.scan_op,
        }
    }

    fn full(&self) -> bool {
        false
    }
}

impl<'p, T, P: 'p> UnindexedConsumer<T> for ScanP1Consumer<'p, P>
where
    T: Send,
    P: Fn(&T, &T) -> T + Send + Sync,
{
    fn split_off_left(&self) -> Self {
        Self {
            scan_op: self.scan_op,
        }
    }

    fn to_reducer(&self) -> Self::Reducer {
        ScanP1Reducer
    }
}

struct ScanP1Folder<'p, T, P> {
    vec: Vec<T>,
    scan_op: &'p P,
}

impl<'p, T, P> Folder<T> for ScanP1Folder<'p, T, P>
where
    P: Fn(&T, &T) -> T + 'p,
{
    type Result = LinkedList<Vec<T>>;

    fn consume(mut self, item: T) -> Self {
        let next = match self.vec.last() {
            None => item,
            Some(prev) => (self.scan_op)(prev, &item),
        };
        self.vec.push(next);
        self
    }

    fn complete(self) -> Self::Result {
        let mut list = LinkedList::new();
        if !self.vec.is_empty() {
            list.push_back(self.vec);
        }
        list
    }

    fn full(&self) -> bool {
        false
    }
}

struct ScanP1Reducer;

impl<T> Reducer<LinkedList<T>> for ScanP1Reducer {
    fn reduce(self, mut left: LinkedList<T>, mut right: LinkedList<T>) -> LinkedList<T> {
        left.append(&mut right);
        left
    }
}

/*********** scan part 2: producer **********/

#[derive(Debug)]
pub struct Scan<T, P> {
    data: Vec<Vec<T>>,
    offsets: Vec<T>,
    scan_op: P,
}

impl<T, P> Scan<T, P>
where
    T: Send + Sync,
    P: Fn(&T, &T) -> T + Send + Sync,
{
    pub(super) fn new(data: Vec<Vec<T>>, offsets: Vec<T>, scan_op: P) -> Self {
        Scan {
            data,
            offsets,
            scan_op,
        }
    }
}

impl<T, P> ParallelIterator for Scan<T, P>
where
    T: Send + Sync,
    P: Fn(&T, &T) -> T + Send + Sync,
{
    type Item = T;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        bridge_unindexed(
            ScanP2Producer {
                data: &self.data,
                offsets: &self.offsets,
                scan_op: &self.scan_op,
            },
            consumer,
        )
    }
}

struct ScanP2Producer<'a, T, P> {
    data: &'a [Vec<T>],
    offsets: &'a [T],
    scan_op: &'a P,
}

impl<'a, T, P> ScanP2Producer<'a, T, P>
where
    T: Send + Sync,
    P: Fn(&T, &T) -> T + Send + Sync,
{
    pub(super) fn new(data: &'a [Vec<T>], offsets: &'a [T], scan_op: &'a P) -> Self {
        ScanP2Producer {
            data,
            offsets,
            scan_op,
        }
    }
}

impl<'a, T, P> UnindexedProducer for ScanP2Producer<'a, T, P>
where
    T: Send + Sync,
    P: Fn(&T, &T) -> T + Send + Sync,
{
    type Item = T;

    fn split(self) -> (Self, Option<Self>) {
        let mid = self.offsets.len() / 2;
        if mid == 0 {
            return (self, None);
        }
        let (data_l, data_r) = self.data.split_at(mid);
        let (offsets_l, offsets_r) = self.offsets.split_at(mid);

        (
            ScanP2Producer::new(data_l, offsets_l, self.scan_op),
            Some(ScanP2Producer::new(data_r, offsets_r, self.scan_op)),
        )
    }

    fn fold_with<F>(self, folder: F) -> F
    where
        F: Folder<Self::Item>,
    {
        let iter = self
            .data
            .iter()
            .zip(self.offsets.iter())
            .flat_map(|(chunk, offset)| chunk.iter().map(|x| (self.scan_op)(offset, x)));
        folder.consume_iter(iter)
    }
}
