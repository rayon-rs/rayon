use super::internal::*;

pub struct NoopConsumer;

impl NoopConsumer {
    pub fn new() -> Self {
        NoopConsumer
    }
}

impl<ITEM> Consumer<ITEM> for NoopConsumer {
    type Folder = NoopConsumer;
    type Reducer = NoopReducer;
    type Result = ();

    fn cost(&mut self, cost: f64) -> f64 {
        cost
    }

    fn split_at(self, _index: usize) -> (Self, Self, NoopReducer) {
        (NoopConsumer, NoopConsumer, NoopReducer)
    }

    fn into_folder(self) -> Self {
        self
    }
}

impl<ITEM> Folder<ITEM> for NoopConsumer {
    type Result = ();

    fn consume(self, _item: ITEM) -> Self {
        self
    }

    fn complete(self) {}
}

impl<ITEM> UnindexedConsumer<ITEM> for NoopConsumer {
    fn split_off(&self) -> Self {
        NoopConsumer
    }

    fn to_reducer(&self) -> NoopReducer {
        NoopReducer
    }
}

pub struct NoopReducer;

impl Reducer<()> for NoopReducer {
    fn reduce(self, _left: (), _right: ()) {}
}
