# v0.1.0 Initial release

**date: 2026-n-n**

[source]()

## Added

- Initial implementation of a lock-free ring buffer supporting a single producer and multiple consumers.
- Basic API for adding items and peeking at the backward from the latest item.
- Support for `no_std` environments, allowing usage in embedded systems.

## Notes

- Comprehensive tests using the Loom framework to verify correctness under concurrent scenarios.
- Benchmarks to evaluate the performance of the ring buffer under various conditions.
- CI workflow set up to run tests and benchmarks on each commit.
