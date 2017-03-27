//! This module contains the parallel iterator types for linked lists
//! (`LinkedList<T>`). You will rarely need to interact with it directly
//! unless you have need to name one of the iterator types.

use std::collections::LinkedList;

use iter::*;
use iter::internal::*;

use vec;

into_par_vec!{
    LinkedList<T> => IntoIter<T>,
    impl<T: Send>
}

into_par_vec!{
    &'a LinkedList<T> => Iter<'a, T>,
    impl<'a, T: Sync>
}

into_par_vec!{
    &'a mut LinkedList<T> => IterMut<'a, T>,
    impl<'a, T: Send>
}



delegate_iterator!{
    #[doc = "Parallel iterator over a linked list"]
    IntoIter<T> => vec::IntoIter<T>,
    impl<T: Send>
}


delegate_iterator!{
    #[doc = "Parallel iterator over an immutable reference to a linked list"]
    Iter<'a, T> => vec::IntoIter<&'a T>,
    impl<'a, T: Sync + 'a>
}


delegate_iterator!{
    #[doc = "Parallel iterator over a mutable reference to a linked list"]
    IterMut<'a, T> => vec::IntoIter<&'a mut T>,
    impl<'a, T: Send + 'a>
}
