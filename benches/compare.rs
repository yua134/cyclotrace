use std::hint::black_box;
use std::thread;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use crossbeam_channel as cb;
use cyclotrace::*;

const OPS: usize = 100_000;

const SIZES: &[usize] = &[2, 64, 1024];

fn bench_spmc(c: &mut Criterion) {
    let mut group = c.benchmark_group("spmc_4readers");
    group.throughput(Throughput::Elements(OPS as u64));

    for &n in SIZES {
        match n {
            2 => bench_spmc_lockfree::<2>(&mut group),
            64 => bench_spmc_lockfree::<64>(&mut group),
            1024 => bench_spmc_lockfree::<1024>(&mut group),
            _ => unreachable!(),
        }

        bench_spmc_crossbeam(&mut group, n);
    }

    group.finish();
}

fn bench_spmc_lockfree<const N: usize>(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
) {
    group.bench_with_input(BenchmarkId::new("cyclotrace", N), &N, |b, &_size| {
        b.iter(|| {
            let (writer, reader) = create_buffer::<u32, N>();

            let r1 = reader.clone();
            let r2 = reader.clone();
            let r3 = reader.clone();

            let producer = thread::spawn(move || {
                for i in 0..OPS {
                    writer.write(black_box(i as u32));
                }
            });

            let t1 = thread::spawn(move || {
                for _ in 0..(OPS / 4) {
                    black_box(r1.read_latest());
                }
            });

            let t2 = thread::spawn(move || {
                for _ in 0..(OPS / 4) {
                    black_box(r2.read_latest());
                }
            });

            let t3 = thread::spawn(move || {
                for _ in 0..(OPS / 4) {
                    black_box(r3.read_latest());
                }
            });

            for _ in 0..(OPS - 3 * (OPS / 4)) {
                black_box(reader.read_latest());
            }

            producer.join().unwrap();
            t1.join().unwrap();
            t2.join().unwrap();
            t3.join().unwrap();
        });
    });
}

fn bench_spmc_crossbeam(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    n: usize,
) {
    group.bench_with_input(BenchmarkId::new("crossbeam_channel", n), &n, |b, &size| {
        b.iter(|| {
            let (tx, rx) = cb::bounded::<u32>(size);

            let rx1 = rx.clone();
            let rx2 = rx.clone();
            let rx3 = rx.clone();

            let producer = thread::spawn(move || {
                for i in 0..OPS {
                    tx.send(black_box(i as u32)).unwrap();
                }
            });

            let t1 = thread::spawn(move || {
                for _ in 0..(OPS / 4) {
                    black_box(rx1.recv().unwrap());
                }
            });

            let t2 = thread::spawn(move || {
                for _ in 0..(OPS / 4) {
                    black_box(rx2.recv().unwrap());
                }
            });

            let t3 = thread::spawn(move || {
                for _ in 0..(OPS / 4) {
                    black_box(rx3.recv().unwrap());
                }
            });

            for _ in 0..(OPS - 3 * (OPS / 4)) {
                black_box(rx.recv().unwrap());
            }

            producer.join().unwrap();
            t1.join().unwrap();
            t2.join().unwrap();
            t3.join().unwrap();
        });
    });
}

criterion_group!(benches, bench_spmc);
criterion_main!(benches);
