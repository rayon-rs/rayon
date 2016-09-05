extern crate rayon;

use std::rc::Rc;

fn main() {
    let r = Rc::new(22);
    rayon::join(|| r.clone(), || r.clone());
    //~^ ERROR E0277
}
