use std::cell::Cell;

use super::CasList;

#[test]
fn simple_list() {
    let list = CasList::new();
    list.prepend(66);
    list.prepend(44);
    list.prepend(22);
    let v: Vec<_> = list.iter().cloned().collect();
    assert_eq!(&v[..], &[22, 44, 66]);
}

struct NoisyDrop<'c> {
    counter: &'c Cell<usize>,
    value: usize,
}

impl<'c> Drop for NoisyDrop<'c> {
    fn drop(&mut self) {
        self.counter.set(self.counter.get() + self.value);
    }
}

#[test]
fn drop() {
    let counter = Cell::new(0);

    {
        let list = CasList::new();
        list.prepend(NoisyDrop { counter: &counter, value: 1 });
        list.prepend(NoisyDrop { counter: &counter, value: 10 });
        list.prepend(NoisyDrop { counter: &counter, value: 100 });

        println!("about to drop");
    }

    assert_eq!(counter.get(), 111);
}
