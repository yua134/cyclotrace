# Cyclotrace

[![crates.io](https://img.shields.io/crates/v/cyclotrace.svg)](https://crates.io/crates/cyclotrace)
[![docs.rs](https://docs.rs/cyclotrace/badge.svg)](https://docs.rs/cyclotrace)
[![CI](https://github.com/yua134/cyclotrace/actions/workflows/ci.yml/badge.svg)](https://github.com/yua134/cyclotrace/actions/workflows/ci.yml)

Cyclotrace is a wait-free ring buffer implementation in Rust, designed for high-performance concurrent scenarios. It supports a single producer and multiple consumers, allowing efficient communication between threads without blocking. Losing data is possible but latency is guaranteed. Unlike traditional queues, the reader semantics in Cyclotrace are based on peeking rather than consuming, ensuring that the data remains available in the buffer until overwritten by the producer.

## Features

- Wait-free API for adding items and peeking at the backward from the latest item (when atomic variables behave wait-free).
- Snapshot API to read a range of items atomically, ensuring consistency across multiple slots.
- 8-byte metadata size for buffer management, even when the a number of items are large, the metadata size remains constant.
- Support for `no_std` environments, making it suitable for embedded systems.
- Comprehensive tests using the [Loom](https://github.com/tokio-rs/loom) to ensure correctness under concurrent scenarios.
- Comprehensive tests using the miri to ensure no undefined behavior occurs in the implementation.

## Use Cases

- audio processing
- telemetry data collection
- sensor data buffering
- real-time logging

## Usage
Add the following to your `Cargo.toml`:

```toml
[dependencies]
cyclotrace = "0.3.0"
```

```rust,no_run
use cyclotrace::{create_buffer, BufWriter, BufReader, Sink};

let (mut writer, reader) = create_buffer::<u32, 512>(); // Create a ring buffer with capacity of 512 items

// Producer thread
std::thread::spawn({
    move || {
        for i in 0..1000 {
            writer.write(i); // Add items to the buffer
        }
    }
});

// Consumer thread
std::thread::spawn({
    let reader = reader.clone(); // Reader can be cloned cheaply
    move || {
        for _ in 0..100 {
            if let Some(item) = reader.get_latest() { // Peek(but copied) at the latest item
                println!("Got item: {}", item);
            }
        }

        let item1 = reader.get(0); // Get the latest item
        let item2 = reader.get(1); // Get the second latest item
        println!("Got latest item: {:?}, Got second latest item: {:?}", item1, item2); // But these can be non-sequential items due to concurrent writes

        let mut buf = Vec::new();
        if reader.get_range(0..100, &mut buf).is_some() { // Get a snapshot of the first 100 items
            println!("Got range: {:?}", buf);
        }
    }
});
```

## Feature Flags

- `alloc`: Enables the `BufWriter` and `BufReader` structs to be used with heap-allocated buffers. This is required for using `Sink` implementations with `Vec<T>`. This feature is enabled by default, but can be disabled for `no_std` environments that do not have access to the heap.

- `portable-atomic`: Enables the use of the `portable-atomic` crate for atomic operations on platforms that do not support the standard library's atomic types.

- `loom`: Enables tests using the Loom framework for verifying correctness under concurrent scenarios. This is unnecessary for normal usage and can be disabled to reduce compile time.

- `arrayvec`: Enables support for using `arrayvec::ArrayVec` as a buffer type in the snapshot API. This allows for efficient storage of items without heap allocation, but requires the `arrayvec` crate as a dependency.

- `heapless`: Enables support for using `heapless::Vec` as a buffer type in the snapshot API. This is particularly useful in `no_std` environments where heap allocation is not available, but requires the `heapless` crate as a dependency.

## Benchmarks

The benchmarks were run on the following system:

- **CPU:** Intel Core Ultra 5 125U
- **OS:** Windows 11
- **Rust:** rustc 1.95.0
- **NUMA:** All participating threads are placed within the same NUMA node

The benchmark consists of one continuously running writer and a specified number of continuously running readers.

The writer and the measured reader are pinned to separate P-cores. When more than one reader is used, the additional readers are pinned to E-cores, one reader per E-core. Thus, increasing the reader count keeps the writer and measured reader on P-cores while adding concurrent readers on E-cores.

For read benchmarks, the measured reader is always the reader running on a P-core. For write benchmarks, the measured writer is running on a P-core.

The `u32` and `u128` benchmarks operate on values of the corresponding types. `huge` operates on a 128-byte structure.

### Concurrent read/write benchmarks

The following table shows the median latency for each configuration.

#### `u32`

| Benchmark         |  1 reader | 2 readers | 4 readers | 8 readers |
| ----------------- | --------: | --------: | --------: | --------: |
| cyclotrace write  |  16.25 ns |  20.73 ns |  19.34 ns |  22.39 ns |
| cyclotrace read   |  29.95 ns |   8.85 ns |  12.97 ns |  13.96 ns |
| spmc_buffer write | 260.11 ns |         — |         — | 241.29 ns |
| spmc_buffer read  |  11.01 ns |         — |         — |  69.18 ns |
| ringcast write    |  62.79 ns |         — |         — |  86.26 ns |
| ringcast read     |  65.12 ns |         — |         — |  43.35 ns |

#### `u128`

| Benchmark        | 1 reader | 2 readers | 4 readers | 8 readers |
| ---------------- | -------: | --------: | --------: | --------: |
| cyclotrace write | 15.58 ns |  32.42 ns |  32.69 ns |  36.40 ns |
| cyclotrace read  | 22.33 ns |  14.61 ns |  16.07 ns |  17.02 ns |

#### 128-byte structure

| Benchmark         |  1 reader | 2 readers | 4 readers | 8 readers |
| ----------------- | --------: | --------: | --------: | --------: |
| cyclotrace write  |  46.32 ns |  59.34 ns |  72.36 ns |  80.50 ns |
| cyclotrace read   |  87.03 ns |  87.95 ns |  91.51 ns | 102.42 ns |
| spmc_buffer write | 253.38 ns |         — |         — | 234.91 ns |
| spmc_buffer read  |  40.70 ns |         — |         — | 135.78 ns |
| ringcast write    |  75.72 ns |         — |         — | 127.38 ns |
| ringcast read     |  76.60 ns |         — |         — |  43.05 ns |

The cyclotrace results show low latency for small values, while the effect of additional concurrent readers becomes more pronounced for larger data.

For the 128-byte structure, cyclotrace write latency increases from 46.32 ns with one reader to 80.50 ns with eight readers. The measured read latency increases from 87.03 ns to 102.42 ns over the same range.

For `u32`, the measured cyclotrace write latency remains within approximately 16–22 ns across the tested reader counts. The measured read latency is non-monotonic across the configurations and should not be interpreted as a direct improvement from adding readers.

`spmc_buffer` shows a substantial increase in read latency when moving from one to eight readers: 11.01 → 69.18 ns for `u32` and 40.70 → 135.78 ns for the 128-byte structure.

`ringcast` shows higher write latency with eight readers than with one reader, while its measured read latency is lower in the eight-reader configuration in these measurements. These values describe the observed benchmark results and do not by themselves establish the cause of the difference.

Because all participating cores are within the same NUMA node, these measurements do not represent cross-NUMA access. The reader-count experiments instead add E-core readers to a fixed P-core writer/P-core measured-reader pair.

### Single-thread `read_write`

`cyclotrace_read_write` is a separate benchmark. It measures a single thread repeatedly performing one read followed by one write and therefore should not be directly compared with the concurrent reader/writer benchmarks above.

| Benchmark                    |   Median |
| ---------------------------- | -------: |
| `cyclotrace_read_write_u32`  |  1.38 ns |
| `cyclotrace_read_write_u128` |  1.72 ns |
| `cyclotrace_read_write_huge` | 39.57 ns |

These measurements characterize the cost of the single-threaded read/write loop rather than the cost of concurrent communication between separate writer and reader threads.


## License

MIT OR Apache-2.0, at your option. See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) for details.
