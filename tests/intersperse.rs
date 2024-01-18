use rayon::prelude::*;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

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
    let v: Vec<_> = (0..1000)
        .into_par_iter()
        .intersperse(-1)
        .intersperse(-2)
        .collect();
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
    (0..1000)
        .into_par_iter()
        .intersperse(-1)
        .zip_eq(0..1999)
        .for_each(|(x, i)| {
            assert_eq!(x, if i % 2 == 0 { i / 2 } else { -1 });
        });
}

#[test]
fn check_intersperse_rev() {
    (0..1000)
        .into_par_iter()
        .intersperse(-1)
        .zip_eq(0..1999)
        .rev()
        .for_each(|(x, i)| {
            assert_eq!(x, if i % 2 == 0 { i / 2 } else { -1 });
        });
}
