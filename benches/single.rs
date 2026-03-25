use std::{
    hint::black_box,
    sync::{Arc, Barrier},
    thread,
};

use criterion::{Criterion, criterion_group, criterion_main};
use cyclotrace::*;

fn single_thread(c: &mut Criterion) {
    c.bench_function("write_read", |b| {
        let (writer, reader) = create_buffer::<u32, 1024>();

        b.iter(|| {
            writer.write(0);
            black_box(reader.get_latest());
        });
    });
}

fn spsc(c: &mut Criterion) {
    c.bench_function("spsc", |b| {
        b.iter_custom(|iters| {
            let (writer, reader) = create_buffer::<u32, 1024>();
            let barrier = Arc::new(Barrier::new(2));
            let b_thread = Arc::clone(&barrier);

            let t = thread::spawn(move || {
                b_thread.wait();
                for i in 0..iters {
                    writer.write(i as u32);
                }
            });

            barrier.wait();
            let start = std::time::Instant::now();
            for _ in 0..iters {
                black_box(reader.get_latest());
            }
            let duration = start.elapsed();

            t.join().unwrap();
            duration
        });
    });
}

fn spmc(c: &mut Criterion) {
    let num_readers = 4;
    c.bench_function("spmc", |b| {
        b.iter_custom(|iters| {
            let (writer, reader) = create_buffer::<u32, 1024>();
            let barrier = Arc::new(Barrier::new(num_readers + 1));

            let b_writer = Arc::clone(&barrier);
            let t_writer = thread::spawn(move || {
                b_writer.wait();
                for i in 0..iters {
                    writer.write(i as u32);
                }
            });

            let mut readers = Vec::new();
            for _ in 0..num_readers {
                let r = reader.clone();
                let b_reader = Arc::clone(&barrier);
                readers.push(thread::spawn(move || {
                    b_reader.wait();
                    for _ in 0..iters {
                        black_box(r.get_latest());
                    }
                }));
            }

            let start = std::time::Instant::now();

            t_writer.join().unwrap();
            for h in readers {
                h.join().unwrap();
            }

            start.elapsed()
        });
    });
}

criterion_group!(benches, single_thread, spsc, spmc);
criterion_main!(benches);
