extern crate rayon;

fn bad_scope<F>(f: F)
    where F: FnOnce(&i32) + Send,
{
    rayon::scope(|s| {
        let x = 22;
        s.spawn(|_| f(&x)); //~ ERROR `x` does not live long enough
    });
}

fn good_scope<F>(f: F)
    where F: FnOnce(&i32) + Send,
{
    let x = 22;
    rayon::scope(|s| {
        s.spawn(|_| f(&x));
    });
}

fn main() {
}
