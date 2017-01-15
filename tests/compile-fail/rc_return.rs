extern crate rayon_core;

use std::rc::Rc;

fn main() {
    rayon_core::join(|| Rc::new(22), || Rc::new(23));
    //~^ ERROR E0277
    //~| ERROR E0277
}
