use super::nbody::NBodyBenchmark;

// Because benchmarks run iteratively, use smaller constants by default:
const BENCH_BODIES: usize = 1000;

const BENCH_TICKS: usize = 10;

fn nbody_bench<TICK>(b: &mut test::Bencher, mut tick: TICK)
where
    TICK: FnMut(&mut NBodyBenchmark),
{
    let mut rng = crate::seeded_rng();
    let mut benchmark = NBodyBenchmark::new(BENCH_BODIES, &mut rng);
    b.iter(|| {
        for _ in 0..BENCH_TICKS {
            tick(&mut benchmark);
        }
    });
}

#[bench]
fn nbody_seq(b: &mut ::test::Bencher) {
    nbody_bench(b, |n| {
        n.tick_seq();
    });
}

#[bench]
fn nbody_par_iter(b: &mut ::test::Bencher) {
    nbody_bench(b, |n| {
        n.tick_par();
    });
}

#[bench]
fn nbody_par_bridge(b: &mut ::test::Bencher) {
    nbody_bench(b, |n| {
        n.tick_par_bridge();
    });
}

#[bench]
fn nbody_parreduce(b: &mut ::test::Bencher) {
    nbody_bench(b, |n| {
        n.tick_par_reduce();
    });
}
