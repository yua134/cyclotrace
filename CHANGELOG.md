# Changelog for `cyclotrace`

## v0.3.0 release

**date: 2026-9-26**

[diff](https://github.com/yua134/cyclotrace/compare/v0.2.0...v0.3.0)

### Breaking Changes
- All elements in the ring buffer require `bytemuck::NoUninit` to ensure that the ring buffer does not contain uninitialized memory, because all elements can be cast to integer types for atomic operations. This change is necessary to remove UBs in the implementation.
- The `Sink` trait now has `set_len` and `as_mut_ptr` methods and removed `push` and `truncate` methods.
- The implementation of the `Sink` trait for `[T; N]` and `[T]` types has been removed.
- It is less sufficient to use `static_buffer!` macro to create a ring buffer, so it is removed, you should use `StaticBuffer` struct directly to create a ring buffer instead.
- `create_buffer` and `StaticBuffer::new` require `size_of::<T>()` * N is a multiple of `cyclotrace::ATOMIC_SIZE`.
- `BufWriter::write` and `Writer::write` now require `&mut self` instead of `&self`.

### Deprecated
- `get`, `get_latest`, and `get_range` methods of the `BufReader` and `Reader` structs are deprecated, and will be removed in the next release. Use `read`, `read_latest`, and `read_range` methods instead, these methods are the same as the deprecated methods.

### Added
- Added new dependency: `bytemuck`
- Added new module: `per_byte_atomic`
- Added new feature flag: `portable-atomic` to support portable atomic operations.
- Added miri tests to CI to check UBs.
- Added `read`, `read_latest`, `read_range` methods to the `BufReader` and `Writer` structs to read items from the writer.

### Changes
- Relaxed the version requirement for some dependencies to allow for more flexibility in dependency resolution.
- Selected the maximum atomic size in the environment automatically.

### Removed
- Removed the `maybe_const` crate dependency.

### Fixes
- Fixed a critical bug in the implementation that caused data races on Rust-memory-model, which was the reason for yanking the previous release.

### Notes
- Updated benchmarks to reflect the changes and added some comparisons with other lock-free ring buffer implementations.
- Refined the tests to ensure that the ring buffer behaves correctly under various scenarios and reflect the API-changes.

## v0.2.0 yanked

### Reason
The v0.2.0 release was yanked due to a critical bug in the implementation that caused data races on Rust-memory-model. The issue will be fixed and a new version will be released once the problem is resolved.

## v0.2.0 release

**date: 2026-4-8**

[diff](https://github.com/yua134/cyclotrace/compare/v0.1.0...v0.2.0)

### Breaking Changes
- The `Sink` trait now features a `push` method instead of the previous `extend` method.

### Changes
- Each item in the ring buffer aligned to a cache line.

### Added
- Added dependency on `maybe_const` crate to switch between `const` and non-`const` functions based on the presence of the `loom` feature. This does not affect performance in non-`loom` runs.

### Fixes
- Fixed a critical bug in the implementation that caused data tearing, which was the reason for yanking the initial release. The issue has been resolved, and the ring buffer now operates correctly under concurrent scenarios.

### Notes
- Performance has substantially regressed when using large memory due to the fix.

## v0.1.0 yanked

**date: 2026-4-8**

### Reason
The initial release of `cyclotrace` was yanked due to a critical bug in the implementation that caused data tearing. The issue will be fixed and a new version will be released once the problem is resolved.

## v0.1.0 Initial release

**date: 2026-3-26**

[source](https://github.com/yua134/cyclotrace/tree/v0.1.0)

### Added

- Initial implementation of a lock-free ring buffer supporting a single producer and multiple consumers.
- Basic API for adding items and peeking at the backward from the latest item.
- Support for `no_std` environments, allowing usage in embedded systems.

### Notes

- Comprehensive tests using the Loom framework to verify correctness under concurrent scenarios.
- Benchmarks to evaluate the performance of the ring buffer under various conditions.
- CI workflow set up to run tests and benchmarks on each commit.
