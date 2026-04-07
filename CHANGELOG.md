# Changelog for `cyclotrace`

## v0.2.0 hotfix

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
