extern crate rayon;

use std::rc::Rc;

fn main() {
    rayon::join(|| Rc::new(22), || ()); //~ ERROR E0277
    rayon::join(|| (), || Rc::new(23)); //~ ERROR E0277
}
