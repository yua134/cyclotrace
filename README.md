# Cyclotrace

[![crates.io](https://img.shields.io/crates/v/cyclotrace.svg)](https://crates.io/crates/cyclotrace)
[![docs.rs](https://docs.rs/cyclotrace/badge.svg)](https://docs.rs/cyclotrace)
[![CI](https://github.com/yua134/cyclotrace/actions/workflows/ci.yml/badge.svg)](https://github.com/yua134/cyclotrace/actions/workflows/ci.yml)

Cyclotrace is a wait-free ring buffer implementation in Rust, designed for high-performance concurrent scenarios. It supports a single producer and multiple consumers, allowing efficient communication between threads without blocking. Losing data is possible but latency is guaranteed. Unlike traditional queues, the reader semantics in Cyclotrace are based on peeking rather than consuming, ensuring that the data remains available in the buffer until overwritten by the producer.

## Features

- Wait-free API for adding items and peeking at the backward from the latest item.
- Snapshot API to read a range of items atomically, ensuring consistency across multiple slots.
- Support for `no_std` environments, making it suitable for embedded systems.
- Comprehensive tests using the Loom framework to ensure correctness under concurrent scenarios.

## Use Cases

- audio processing
- telemetry data collection
- sensor data buffering
- real-time logging

## Usage
Add the following to your `Cargo.toml`:

```toml
[dependencies]
cyclotrace = "0.1.0"
```

Then, you can use the `RingBuf` struct to create a wait-free ring buffer:

```rust,no_run
use cyclotrace::{create_buffer, BufWriter, BufReader, Sink};

let (writer, reader) = create_buffer::<u32, 512>(); // Create a ring buffer with capacity of 512 items

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
            if let Some(item) = reader.get_latest() { // Peek at the latest item
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

- `loom`: Enables tests using the Loom framework for verifying correctness under concurrent scenarios. This is unnecessary for normal usage and can be disabled to reduce compile time.

- `arrayvec`: Enables support for using `arrayvec::ArrayVec` as a buffer type in the snapshot API. This allows for efficient storage of items without heap allocation, but requires the `arrayvec` crate as a dependency.

- `heapless`: Enables support for using `heapless::Vec` as a buffer type in the snapshot API. This is particularly useful in `no_std` environments where heap allocation is not available, but requires the `heapless` crate as a dependency.

## Benchmarks

compare with crossbeam's `crossbeam_channel::bounded`
| elements | cyclotrace (ns) | crossbeam (ns) |
|------------|-----------------|-----------------|
| 2 | 74.5 | 162.8 |
| 64 | 11.4 | 66.4 |
| 1024 | 8.45 | 65.3 |

## License

MIT OR Apache-2.0, at your option. See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) for details.
