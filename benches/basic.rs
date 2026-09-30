use std::{
    hint::black_box,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
    thread,
};

use bytemuck::NoUninit;
use criterion::{Criterion, criterion_group, criterion_main};

#[derive(Clone, Copy)]
struct Huge {
    _data: [u8; 128],
}

impl Default for Huge {
    #[inline]
    fn default() -> Self {
        Self { _data: [0u8; 128] }
    }
}

unsafe impl NoUninit for Huge {}

fn pin_cpu(cpu: usize) {
    let mask = 1 << cpu;

    unsafe {
        let previous = windows_sys::Win32::System::Threading::SetThreadAffinityMask(
            windows_sys::Win32::System::Threading::GetCurrentThread(),
            mask,
        );

        assert_ne!(previous, 0);

        let ok = windows_sys::Win32::System::Threading::SetPriorityClass(
            windows_sys::Win32::System::Threading::GetCurrentProcess(),
            windows_sys::Win32::System::Threading::HIGH_PRIORITY_CLASS,
        );

        assert_ne!(ok, 0);

        let ok = windows_sys::Win32::System::Threading::SetThreadPriority(
            windows_sys::Win32::System::Threading::GetCurrentThread(),
            windows_sys::Win32::System::Threading::THREAD_PRIORITY_HIGHEST,
        );

        assert_ne!(ok, 0);
    }
}

const WRITER_CPU: usize = 10;
const READER_CPU: usize = 0;

const CAPACITY: usize = 128;
const OPS: usize = 1000;

// ============================================================
// Cyclotrace
// ============================================================

fn bench_cyclotrace_write<T, const READERS: usize>(c: &mut Criterion, name: &'static str)
where
    T: NoUninit + Send + Default,
{
    c.bench_function(name, |b| {
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;

            for _ in 0..iters {
                let (mut writer, reader) = cyclotrace::create_buffer::<T, CAPACITY>();

                let ready = Arc::new(AtomicU8::new(0));
                let finish = Arc::new(AtomicBool::new(false));

                let mut handles = Vec::with_capacity(READERS);

                let rx = reader.clone();
                let f = Arc::clone(&finish);
                let r = Arc::clone(&ready);
                handles.push(thread::spawn(move || {
                    pin_cpu(READER_CPU);

                    r.fetch_add(1, Ordering::Relaxed);

                    while !f.load(Ordering::Relaxed) {
                        black_box(rx.try_read_latest());
                    }
                }));

                for i in 0..(READERS - 1) {
                    let r = reader.clone();
                    let finish = Arc::clone(&finish);
                    let ready = Arc::clone(&ready);

                    handles.push(thread::spawn(move || {
                        pin_cpu(i + 2);

                        ready.fetch_add(1, Ordering::Relaxed);

                        while !finish.load(Ordering::Relaxed) {
                            black_box(r.try_read_latest());
                        }
                    }));
                }

                let writer = thread::spawn(move || {
                    pin_cpu(WRITER_CPU);

                    while ready.load(Ordering::Relaxed) != READERS as u8 {
                        std::hint::spin_loop();
                    }

                    let start = std::time::Instant::now();

                    for _ in 0..OPS {
                        black_box(writer.write(T::default()));
                    }

                    start.elapsed()
                });

                total += writer.join().unwrap();

                finish.store(true, Ordering::Relaxed);

                for handle in handles {
                    handle.join().unwrap();
                }
            }

            total / OPS as u32
        });
    });
}

fn bench_cyclotrace_read<T, const READERS: usize>(c: &mut Criterion, name: &'static str)
where
    T: NoUninit + Send + Default,
{
    c.bench_function(name, |b| {
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;

            for _ in 0..iters {
                let (mut writer, reader) = cyclotrace::create_buffer::<T, CAPACITY>();

                let ready = Arc::new(AtomicU8::new(0));
                let finish = Arc::new(AtomicBool::new(false));

                let mut handles = Vec::with_capacity(READERS);

                // Background readers.
                for i in 0..(READERS - 1) {
                    let r = reader.clone();
                    let finish = Arc::clone(&finish);
                    let ready = Arc::clone(&ready);

                    handles.push(thread::spawn(move || {
                        pin_cpu(i + 2);

                        ready.fetch_add(1, Ordering::Relaxed);

                        while !finish.load(Ordering::Relaxed) {
                            black_box(r.try_read_latest());
                        }
                    }));
                }

                let r = ready.clone();
                let f = finish.clone();
                // Keep producer active while the measured reader runs.
                handles.push(thread::spawn(move || {
                    pin_cpu(WRITER_CPU);

                    r.fetch_add(1, Ordering::Relaxed);

                    while !f.load(Ordering::Relaxed) {
                        black_box(writer.write(T::default()));
                    }
                }));

                let reader = thread::spawn(move || {
                    pin_cpu(READER_CPU);

                    while ready.load(Ordering::Relaxed) != READERS as u8 {
                        std::hint::spin_loop();
                    }

                    let start = std::time::Instant::now();

                    for _ in 0..OPS {
                        black_box(reader.try_read_latest());
                    }

                    start.elapsed()
                });

                total += reader.join().unwrap();

                finish.store(true, Ordering::Relaxed);

                for handle in handles {
                    handle.join().unwrap();
                }
            }

            total / OPS as u32
        });
    });
}

fn bench_read_write<T: NoUninit + Send + Default>(c: &mut Criterion, name: &'static str) {
    c.bench_function(name, |b| {
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;

            for _ in 0..iters {
                let buffer = cyclotrace::create_buffer::<T, CAPACITY>();

                let (mut writer, _reader) = buffer;

                let start = std::time::Instant::now();

                for _ in 0..OPS {
                    black_box(writer.write(T::default()));
                    black_box(writer.read_latest());
                }

                total += start.elapsed();
            }

            total / OPS as u32
        });
    });
}

// ============================================================
// spmc-buffer
// ============================================================

fn bench_spmc_write<T, const READERS: usize>(c: &mut Criterion, name: &'static str)
where
    T: Copy + Send + Sync + 'static + Default,
{
    c.bench_function(name, |b| {
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;

            for _ in 0..iters {
                let buffer = spmc_buffer::SPMCBuffer::new(READERS, T::default());

                let (mut writer, reader) = buffer.split();

                let ready = Arc::new(AtomicU8::new(0));
                let finish = Arc::new(AtomicBool::new(false));

                let mut handles = Vec::with_capacity(READERS);

                let mut rx = reader.clone();
                let f = Arc::clone(&finish);
                let r = Arc::clone(&ready);
                handles.push(thread::spawn(move || {
                    pin_cpu(READER_CPU);

                    r.fetch_add(1, Ordering::Relaxed);

                    while !f.load(Ordering::Relaxed) {
                        black_box(rx.read());
                    }
                }));

                for _ in 0..(READERS - 1) {
                    let mut r = reader.clone();
                    let ready = Arc::clone(&ready);
                    let finish = Arc::clone(&finish);

                    let cpu = 2 + handles.len();
                    handles.push(thread::spawn(move || {
                        pin_cpu(cpu);

                        ready.fetch_add(1, Ordering::Relaxed);

                        while !finish.load(Ordering::Relaxed) {
                            black_box(r.read());
                        }
                    }));
                }

                let writer = thread::spawn(move || {
                    pin_cpu(WRITER_CPU);

                    while ready.load(Ordering::Relaxed) != READERS as u8 {
                        std::hint::spin_loop();
                    }

                    let start = std::time::Instant::now();

                    for _ in 0..OPS {
                        black_box(writer.write(T::default()));
                    }

                    start.elapsed()
                });

                total += writer.join().unwrap();

                finish.store(true, Ordering::Relaxed);

                for handle in handles {
                    handle.join().unwrap();
                }
            }

            total / OPS as u32
        });
    });
}

fn bench_spmc_read<T, const READERS: usize>(c: &mut Criterion, name: &'static str)
where
    T: Copy + Send + Sync + 'static + Default,
{
    c.bench_function(name, |b| {
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;

            for _ in 0..iters {
                let buffer = spmc_buffer::SPMCBuffer::new(READERS, T::default());

                let (mut writer, mut reader) = buffer.split();

                let finish = Arc::new(AtomicBool::new(false));
                let ready = Arc::new(AtomicU8::new(0));

                let mut handles = Vec::with_capacity(READERS - 1);

                handles.push(thread::spawn({
                    let r = Arc::clone(&ready);
                    let f = Arc::clone(&finish);
                    move || {
                        pin_cpu(WRITER_CPU);

                        r.fetch_add(1, Ordering::Relaxed);

                        while !f.load(Ordering::Relaxed) {
                            black_box(writer.write(T::default()));
                        }
                    }
                }));

                // Background readers.
                for _ in 0..(READERS - 1) {
                    let mut r = reader.clone();
                    let finish = Arc::clone(&finish);
                    let ready = Arc::clone(&ready);

                    let cpu = 2 + handles.len();
                    handles.push(thread::spawn(move || {
                        pin_cpu(cpu);

                        ready.fetch_add(1, Ordering::Relaxed);

                        while !finish.load(Ordering::Relaxed) {
                            black_box(r.read());
                        }
                    }));
                }

                let reader = thread::spawn(move || {
                    while ready.load(Ordering::Relaxed) != READERS as u8 {
                        std::hint::spin_loop();
                    }

                    pin_cpu(READER_CPU);

                    let start = std::time::Instant::now();

                    for _ in 0..OPS {
                        let r = black_box(reader.read());
                        let s = black_box(*black_box(r));
                        black_box(s);
                    }

                    start.elapsed()
                });

                total += reader.join().unwrap();

                finish.store(true, Ordering::Relaxed);

                for handle in handles {
                    handle.join().unwrap();
                }
            }

            total / OPS as u32
        });
    });
}

// ============================================================
// ringcast
// ============================================================

fn bench_ringcast_write<T, const READERS: usize>(c: &mut Criterion, name: &'static str)
where
    T: Copy + Send + 'static + Default,
{
    c.bench_function(name, |b| {
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;

            for _ in 0..iters {
                let (writer, reader) = ringcast::bounded::<T>(CAPACITY);

                let ready = Arc::new(AtomicU8::new(0));
                let finish = Arc::new(AtomicBool::new(false));

                let mut handles = Vec::with_capacity(READERS);

                // First receiver.
                {
                    let mut r = reader;
                    let finish = Arc::clone(&finish);
                    let ready = Arc::clone(&ready);

                    handles.push(thread::spawn(move || {
                        pin_cpu(READER_CPU);

                        ready.fetch_add(1, Ordering::Relaxed);

                        while !finish.load(Ordering::Relaxed) {
                            let _ = black_box(r.try_recv());
                        }
                    }));
                }

                // Additional receivers.
                for i in 0..(READERS - 1) {
                    let mut r = writer.subscribe();
                    let finish = Arc::clone(&finish);
                    let ready = Arc::clone(&ready);

                    handles.push(thread::spawn(move || {
                        pin_cpu(i + 2);

                        ready.fetch_add(1, Ordering::Relaxed);

                        while !finish.load(Ordering::Relaxed) {
                            let _ = black_box(r.try_recv());
                        }
                    }));
                }

                let writer = thread::spawn(move || {
                    pin_cpu(WRITER_CPU);

                    while ready.load(Ordering::Relaxed) != READERS as u8 {
                        std::hint::spin_loop();
                    }

                    let start = std::time::Instant::now();

                    for _ in 0..OPS {
                        black_box(writer.send(T::default()));
                    }

                    start.elapsed()
                });

                total += writer.join().unwrap();

                finish.store(true, Ordering::Relaxed);

                for handle in handles {
                    handle.join().unwrap();
                }
            }

            total / OPS as u32
        });
    });
}

fn bench_ringcast_read<T, const READERS: usize>(c: &mut Criterion, name: &'static str)
where
    T: Copy + Send + 'static + Default,
{
    c.bench_function(name, |b| {
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;

            for _ in 0..iters {
                let (writer, mut reader) = ringcast::bounded::<T>(CAPACITY);

                let finish = Arc::new(AtomicBool::new(false));
                let ready = Arc::new(AtomicU8::new(0));

                let mut handles = Vec::with_capacity(READERS - 1);

                // Background receivers.
                for _ in 0..(READERS - 1) {
                    let mut r = writer.subscribe();
                    let finish = Arc::clone(&finish);
                    let ready = Arc::clone(&ready);

                    let cpu = 2 + handles.len();
                    handles.push(thread::spawn(move || {
                        pin_cpu(cpu);

                        ready.fetch_add(1, Ordering::Relaxed);

                        while !finish.load(Ordering::Relaxed) {
                            let _ = black_box(r.try_recv());
                        }
                    }));
                }

                let r = Arc::clone(&ready);
                let f = Arc::clone(&finish);
                // Producer.
                handles.push(thread::spawn(move || {
                    pin_cpu(WRITER_CPU);

                    r.fetch_add(1, Ordering::Relaxed);

                    while !f.load(Ordering::Relaxed) {
                        black_box(writer.send(T::default()));
                    }
                }));

                let reader = thread::spawn(move || {
                    while ready.load(Ordering::Relaxed) != READERS as u8 {
                        std::hint::spin_loop();
                    }

                    pin_cpu(READER_CPU);

                    let start = std::time::Instant::now();

                    for _ in 0..OPS {
                        let _ = black_box(reader.try_recv());
                    }

                    start.elapsed()
                });

                total += reader.join().unwrap();

                finish.store(true, Ordering::Relaxed);

                for handle in handles {
                    handle.join().unwrap();
                }
            }

            total / OPS as u32
        });
    });
}

// ============================================================
// Bench groups
// ============================================================

fn bench_cyclotrace(c: &mut Criterion) {
    bench_cyclotrace_write::<u32, 1>(c, "cyclotrace_write_u32_1reader");

    bench_cyclotrace_write::<u32, 2>(c, "cyclotrace_write_u32_2readers");

    bench_cyclotrace_write::<u32, 4>(c, "cyclotrace_write_u32_4readers");

    bench_cyclotrace_write::<u32, 8>(c, "cyclotrace_write_u32_8readers");

    bench_cyclotrace_read::<u32, 1>(c, "cyclotrace_read_u32_1reader");

    bench_cyclotrace_read::<u32, 2>(c, "cyclotrace_read_u32_2readers");

    bench_cyclotrace_read::<u32, 4>(c, "cyclotrace_read_u32_4readers");

    bench_cyclotrace_read::<u32, 8>(c, "cyclotrace_read_u32_8readers");

    bench_cyclotrace_write::<u128, 1>(c, "cyclotrace_write_u128_1reader");

    bench_cyclotrace_write::<u128, 2>(c, "cyclotrace_write_u128_2readers");

    bench_cyclotrace_write::<u128, 4>(c, "cyclotrace_write_u128_4readers");

    bench_cyclotrace_write::<u128, 8>(c, "cyclotrace_write_u128_8readers");

    bench_cyclotrace_read::<u128, 1>(c, "cyclotrace_read_u128_1reader");

    bench_cyclotrace_read::<u128, 2>(c, "cyclotrace_read_u128_2readers");

    bench_cyclotrace_read::<u128, 4>(c, "cyclotrace_read_u128_4readers");

    bench_cyclotrace_read::<u128, 8>(c, "cyclotrace_read_u128_8readers");

    bench_cyclotrace_write::<Huge, 1>(c, "cyclotrace_write_huge_1reader");

    bench_cyclotrace_write::<Huge, 2>(c, "cyclotrace_write_huge_2readers");

    bench_cyclotrace_write::<Huge, 4>(c, "cyclotrace_write_huge_4readers");

    bench_cyclotrace_write::<Huge, 8>(c, "cyclotrace_write_huge_8readers");

    bench_cyclotrace_read::<Huge, 1>(c, "cyclotrace_read_huge_1reader");

    bench_cyclotrace_read::<Huge, 2>(c, "cyclotrace_read_huge_2readers");

    bench_cyclotrace_read::<Huge, 4>(c, "cyclotrace_read_huge_4readers");

    bench_cyclotrace_read::<Huge, 8>(c, "cyclotrace_read_huge_8readers");

    // single thread read/write
    bench_read_write::<u32>(c, "cyclotrace_read_write_u32");

    bench_read_write::<u128>(c, "cyclotrace_read_write_u128");

    bench_read_write::<Huge>(c, "cyclotrace_read_write_huge");
}

fn bench_spmc_buffer(c: &mut Criterion) {
    // // u32
    bench_spmc_write::<u32, 1>(c, "spmc_buffer_write_u32_1reader");

    bench_spmc_write::<u32, 8>(c, "spmc_buffer_write_u32_8readers");

    bench_spmc_read::<u32, 1>(c, "spmc_buffer_read_u32_1reader");

    bench_spmc_read::<u32, 8>(c, "spmc_buffer_read_u32_8readers");

    // Huge
    bench_spmc_write::<Huge, 1>(c, "spmc_buffer_write_huge_1reader");

    bench_spmc_write::<Huge, 8>(c, "spmc_buffer_write_huge_8readers");

    bench_spmc_read::<Huge, 1>(c, "spmc_buffer_read_huge_1reader");

    bench_spmc_read::<Huge, 8>(c, "spmc_buffer_read_huge_8readers");
}

fn bench_ringcast(c: &mut Criterion) {
    // u32
    bench_ringcast_write::<u32, 1>(c, "ringcast_write_u32_1reader");

    bench_ringcast_write::<u32, 8>(c, "ringcast_write_u32_8readers");

    bench_ringcast_read::<u32, 1>(c, "ringcast_read_u32_1reader");

    bench_ringcast_read::<u32, 8>(c, "ringcast_read_u32_8readers");

    // Huge
    bench_ringcast_write::<Huge, 1>(c, "ringcast_write_huge_1reader");

    bench_ringcast_write::<Huge, 8>(c, "ringcast_write_huge_8readers");

    bench_ringcast_read::<Huge, 1>(c, "ringcast_read_huge_1reader");

    bench_ringcast_read::<Huge, 8>(c, "ringcast_read_huge_8readers");
}

criterion_group!(benches, bench_cyclotrace, bench_spmc_buffer, bench_ringcast,);

criterion_main!(benches);
