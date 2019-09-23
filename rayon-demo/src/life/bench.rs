use super::Board;

#[bench]
fn generations(b: &mut ::test::Bencher) {
    b.iter(|| super::generations(Board::new(200, 200).random(), 100));
}

#[bench]
fn par_iter_generations(b: &mut ::test::Bencher) {
    b.iter(|| super::parallel_generations(Board::new(200, 200).random(), 100));
}

#[bench]
fn par_bridge_generations(b: &mut ::test::Bencher) {
    b.iter(|| super::par_bridge_generations(Board::new(200, 200).random(), 100));
}
