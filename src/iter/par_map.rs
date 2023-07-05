use crate::ScopeFifo;
use std::sync::Arc;
use std::collections::VecDeque;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::sync_channel;

trait LocalOrGlobalScope<'scope>
where
    Self: 'scope
{
    fn spawn_in_scope<Task>(&self, task: Task)
    where
        Task: for<'scoperef> FnOnce(&'scoperef Self),
        Task: Send + 'scope;
}

impl<'scope> LocalOrGlobalScope<'scope> for ScopeFifo<'scope> {
    fn spawn_in_scope<Task>(&self, task: Task)
    where
        Task: for<'scoperef> FnOnce(&'scoperef Self),
        Task: Send + 'scope
    {
        self.spawn_fifo(task);
    }
}

#[derive(Debug)]
pub struct GlobalScope;

impl LocalOrGlobalScope<'static> for GlobalScope {
    fn spawn_in_scope<Task>(&self, task: Task)
    where
        Task: for<'scoperef> FnOnce(&'scoperef Self),
        Task: Send + 'static
    {
        crate::spawn_fifo(||task(&GlobalScope));
    }
}

pub struct ParallelMapIter<'scoperef, 'scope, SomeScope, InputIter, OutputItem>
where
    InputIter: Iterator
{
    chans: VecDeque<Receiver<OutputItem>>,
    iter: std::iter::Fuse<InputIter>,
    scope: &'scoperef SomeScope,

    // We have to use "dyn" here, because return_position_impl_trait_in_trait is not yet stable
    op: Arc<dyn for<'scoperef2> Fn(&'scoperef2 SomeScope, InputIter::Item) -> OutputItem + Sync + Send + 'scope>,
}

impl<'scoperef, 'scope, SomeScope, InputIter: Iterator, OutputItem> std::fmt::Debug for ParallelMapIter<'scoperef, 'scope, SomeScope, InputIter, OutputItem> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.debug_struct("ParallelMapIter").finish()
    }
}

fn push_task<'scoperef, 'scope, SomeScope, InputItem, OutputItem>(
    input: InputItem,
    scope: &'scoperef SomeScope,
    op: Arc<dyn for<'scoperef2> Fn(&'scoperef2 SomeScope, InputItem) -> OutputItem + Sync + Send + 'scope>,
    chans: &mut VecDeque<Receiver<OutputItem>>
)
where
    InputItem: Send + 'scope,
    OutputItem: Send + 'scope,
    SomeScope: LocalOrGlobalScope<'scope>,
{
    let (send, recv) = sync_channel(1);
    scope.spawn_in_scope(|scope|{
        send.send(op(scope, input)).unwrap();
        drop(send);
        drop(op);
    });
    chans.push_back(recv);
}

fn low_level_par_map<'scoperef, 'scope, SomeScope, InputIter, OutputItem, Op>(iter: InputIter, scope: &'scoperef SomeScope, capacity: usize, op: Op) -> ParallelMapIter<'scoperef, 'scope, SomeScope, InputIter, OutputItem>
where
    Op: for<'scoperef2> Fn(&'scoperef2 SomeScope, InputIter::Item) -> OutputItem,
    Op: Sync + Send + 'scope,
    InputIter::Item: Send + 'scope,
    OutputItem: Send + 'scope,
    SomeScope: LocalOrGlobalScope<'scope>,
    InputIter: Iterator,
{
    assert!(capacity >= 1);
    let mut chans = VecDeque::new();
    let op: Arc<dyn for<'scoperef2> Fn(&'scoperef2 SomeScope, InputIter::Item) -> OutputItem + Sync + Send + 'scope> = Arc::new(op);
    let mut iter = iter.fuse();
    for _ in 0..(capacity - 1) {
        if let Some(input) = iter.next() {
            push_task(input, scope, Arc::clone(&op), &mut chans);
        } else {
            break;
        }
    }
    ParallelMapIter { chans, iter, scope, op }
}

/// TODO
pub trait ParallelMap: Iterator + Sized {
    /// TODO
    fn par_map_with_scope_and_capacity<'scoperef, 'scope, OutputItem, Op>(self, scope: &'scoperef ScopeFifo<'scope>, capacity: usize, op: Op) -> ParallelMapIter<'scoperef, 'scope, ScopeFifo<'scope>, Self, OutputItem>
    where
        Op: for<'scoperef2> Fn(&'scoperef2 ScopeFifo<'scope>, Self::Item) -> OutputItem,
        Op: Sync + Send + 'scope,
        Self::Item: Send + 'scope,
        OutputItem: Send + 'scope,
    {
        low_level_par_map(self, scope, capacity, op)
    }
    /// TODO
    fn par_map_with_scope<'scoperef, 'scope, OutputItem, Op>(self, scope: &'scoperef ScopeFifo<'scope>, op: Op) -> ParallelMapIter<'scoperef, 'scope, ScopeFifo<'scope>, Self, OutputItem>
    where
        Op: for<'scoperef2> Fn(&'scoperef2 ScopeFifo<'scope>, Self::Item) -> OutputItem,
        Op: Sync + Send + 'scope,
        Self::Item: Send + 'scope,
        OutputItem: Send + 'scope,
    {
        // We can just call rayon::current_num_threads. Unfortunately, this will not work if the scope was created using rayon::ThreadPool::in_place_scope_fifo. So we have to spawn new task merely to learn number of threads
        let (send, recv) = sync_channel(1);
        scope.spawn_fifo(|_|{
            send.send(crate::current_num_threads()).unwrap();
            drop(send);
        });
        let capacity = recv.recv().unwrap() * 2;
        drop(recv);
        self.par_map_with_scope_and_capacity(scope, capacity, op)
    }
    /// TODO
    fn par_map_with_capacity<OutputItem, Op>(self, capacity: usize, op: Op) -> ParallelMapIter<'static, 'static, GlobalScope, Self, OutputItem>
    where
        Op: Fn(Self::Item) -> OutputItem,
        Op: Sync + Send + 'static,
        Self::Item: Send + 'static,
        OutputItem: Send + 'static,
    {
        low_level_par_map(self, &GlobalScope, capacity, move |_, input|op(input))
    }
    /// TODO
    fn par_map<OutputItem, Op>(self, op: Op) -> ParallelMapIter<'static, 'static, GlobalScope, Self, OutputItem>
    where
        Op: Fn(Self::Item) -> OutputItem,
        Op: Sync + Send + 'static,
        Self::Item: Send + 'static,
        OutputItem: Send + 'static,
    {
        self.par_map_with_capacity(crate::current_num_threads() * 2, op)
    }
    /// TODO
    fn par_map_for_each_with_capacity<MapOp, ForEachOp, OutputItem>(self, capacity: usize, map_op: MapOp, for_each_op: ForEachOp)
    where
        MapOp: Fn(Self::Item) -> OutputItem,
        MapOp: Sync + Send,
        ForEachOp: FnMut(OutputItem),
        Self::Item: Send,
        OutputItem: Send,
    {
        crate::in_place_scope_fifo(|s|{
            self.par_map_with_scope_and_capacity(s, capacity, move |_, input|map_op(input)).for_each(for_each_op);
        });
    }
    /// TODO
    fn par_map_for_each<MapOp, ForEachOp, OutputItem>(self, map_op: MapOp, for_each_op: ForEachOp)
    where
        MapOp: Fn(Self::Item) -> OutputItem,
        MapOp: Sync + Send,
        ForEachOp: FnMut(OutputItem),
        Self::Item: Send,
        OutputItem: Send,
    {
        self.par_map_for_each_with_capacity(crate::current_num_threads() * 2, map_op, for_each_op);
    }
}

impl<InputIter: Iterator> ParallelMap for InputIter {
}

impl<'scoperef, 'scope, SomeScope, InputIter, OutputItem> Iterator for ParallelMapIter<'scoperef, 'scope, SomeScope, InputIter, OutputItem>
where
    InputIter: Iterator,
    InputIter::Item: Send + 'scope,
    OutputItem: Send + 'scope,
    SomeScope: LocalOrGlobalScope<'scope>,
{
    type Item = OutputItem;
    fn next(&mut self) -> Option<OutputItem> {
        if let Some(input) = self.iter.next() {
            push_task(input, self.scope, Arc::clone(&self.op), &mut self.chans);
        }
        #[allow(clippy::manual_map)]
        if let Some(output) = self.chans.pop_front() {
            Some(output.recv().unwrap())
        } else {
            None
        }
    }
}
