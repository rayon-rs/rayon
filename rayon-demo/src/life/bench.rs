use super::Board;

#[bench]
fn generations(b: &mut ::test::Bencher) {
    b.iter(|| super::generations(Board::new(200, 200).random(), 100));
}

#[bench]
fn parallel_generations(b: &mut ::test::Bencher) {
    b.iter(|| super::parallel_generations(Board::new(200, 200).random(), 100));
}
