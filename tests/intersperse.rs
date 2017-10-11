extern crate rayon;

use rayon::prelude::*;
use rayon::iter::internal::*;

#[test]
fn check_intersperse() {
    let v: Vec<_> = (0..1000).into_par_iter().intersperse(-1).collect();
    assert_eq!(v.len(), 1999);
    for (i, x) in v.into_iter().enumerate() {
        assert_eq!(x, if i % 2 == 0 { i as i32 / 2 } else { -1 });
    }
}

#[test]
fn check_intersperse_again() {
    let v: Vec<_> = (0..1000).into_par_iter().intersperse(-1).intersperse(-2).collect();
    assert_eq!(v.len(), 3997);
    for (i, x) in v.into_iter().enumerate() {
        let y = match i % 4 {
            0 => i as i32 / 4,
            2 => -1,
            _ => -2,
        };
        assert_eq!(x, y);
    }
}

#[test]
fn check_intersperse_unindexed() {
    let v: Vec<_> = (0..1000).map(|i| i.to_string()).collect();
    let s = v.join(",");
    let s2 = v.join(";");
    let par: String = s.par_split(',').intersperse(";").collect();
    assert_eq!(par, s2);
}

#[test]
fn check_intersperse_producer() {
    (0..1000).into_par_iter().intersperse(-1)
        .zip_eq(0..1999)
        .for_each(|(x, i)| {
            assert_eq!(x, if i % 2 == 0 { i / 2 } else { -1 });
        });
}

#[test]
fn check_intersperse_rev() {
    (0..1000).into_par_iter().intersperse(-1)
        .zip_eq(0..1999).rev()
        .for_each(|(x, i)| {
            assert_eq!(x, if i % 2 == 0 { i / 2 } else { -1 });
        });
}

#[test]
fn check_producer_splits() {
    struct Callback(usize, usize, usize);

    impl ProducerCallback<i32> for Callback {
        type Output = Vec<i32>;
        fn callback<P>(self, producer: P) -> Self::Output
            where P: Producer<Item = i32>
        {
            println!("splitting {} {} {}", self.0, self.1, self.2);

            // Splitting the outer indexes first gets us an arbitrary mid section,
            // which we then split further to get full test coverage.
            let (a, right) = producer.split_at(self.0);
            let (mid, d) = right.split_at(self.2 - self.0);
            let (b, c) = mid.split_at(self.1 - self.0);

            let (a, b, c, d) = (a.into_iter(), b.into_iter(), c.into_iter(), d.into_iter());
            assert_eq!(a.len(), self.0);
            assert_eq!(b.len(), self.1 - self.0);
            assert_eq!(c.len(), self.2 - self.1);

            a.chain(b).chain(c).chain(d).collect()
        }
    }

    let v = [1, 2, 3, 4, 5];
    let vi = [1, -1, 2, -1, 3, -1, 4, -1, 5];

    for i in 0 .. vi.len() + 1 {
        for j in i .. vi.len() + 1 {
            for k in j .. vi.len() + 1 {
                let res = v.par_iter().cloned()
                    .intersperse(-1)
                    .with_producer(Callback(i, j, k));
                assert_eq!(res, vi);
            }
        }
    }
}
