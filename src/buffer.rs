use core::{mem::MaybeUninit, ops::RangeBounds};

use crate::{
    ATOMIC_SIZE, AtomicU64, Ordering,
    per_byte_atomic::{PerByteLoad, PerByteStore},
    sink::Sink,
    spin_loop,
};

use bytemuck::NoUninit;
use crossbeam_utils::CachePadded;

#[cfg(not(feature = "loom"))]
pub(crate) struct Buffer<T, const N: usize> {
    data: core::cell::UnsafeCell<
        crate::buffer::slots::AtomicBaseAligned<[crate::buffer::slots::Slot<T>; N]>,
    >,
    tail: CachePadded<AtomicU64>,
}
#[cfg(feature = "loom")]
pub(crate) struct Buffer<T, const N: usize> {
    data: crate::util::LoomStorage<T, N>,
    tail: CachePadded<AtomicU64>,
}
impl<T: NoUninit, const N: usize> Buffer<T, N> {
    #[inline(always)]
    #[cfg(not(feature = "loom"))]
    pub(crate) const fn new() -> Self {
        assert!(N > 2, "Buffer size must be greater than 2");
        assert!(
            N.is_power_of_two(),
            "Buffer size must be a power of 2 for better performance"
        );
        assert!(
            (N * core::mem::size_of::<T>()).is_multiple_of(ATOMIC_SIZE),
            "Buffer size must be a multiple of the size of ATOMIC_SIZE for better performance"
        );
        assert!(
            core::mem::size_of::<T>() > 0,
            "Buffer item size must be greater than 0"
        );
        Self {
            data: core::cell::UnsafeCell::new(crate::buffer::slots::AtomicBaseAligned::new(
                crate::util::init_array::<T, N>(),
            )),
            tail: CachePadded::new(AtomicU64::new(0)),
        }
    }

    #[track_caller]
    #[cfg(feature = "loom")]
    pub(crate) fn new() -> Self {
        assert!(N > 2, "Buffer size must be greater than 2");
        assert!(
            N.is_power_of_two(),
            "Buffer size must be a power of 2 for better performance"
        );
        assert!(
            (N * core::mem::size_of::<T>()).is_multiple_of(ATOMIC_SIZE),
            "Buffer size must be a multiple of the size of ATOMIC_SIZE for better performance"
        );
        assert!(
            core::mem::size_of::<T>() > 0,
            "Buffer item size must be greater than 0"
        );
        Self {
            data: crate::util::LoomStorage::new(),
            tail: CachePadded::new(AtomicU64::new(0)),
        }
    }

    #[inline(always)]
    pub(crate) fn is_empty(&self) -> bool {
        self.tail.load(Ordering::Acquire) == 0
    }

    #[inline(always)]
    /// This function must be called only one thread.
    pub(crate) unsafe fn write(&self, value: T, tail: u64) -> u64 {
        // N is a power of 2, so we can use bitwise AND instead of modulo to get the index.
        // tail as usize % N is equivalent to tail & (N - 1) as u64, but the latter is much faster.
        // SAFETY:
        // - self.data is aligned to ATOMIC_ALIGN.
        // - The size of the buffer is a multiple of ATOMIC_SIZE.
        // - This function called only one thread, so no concurrent write is possible.
        unsafe {
            PerByteStore::new(&self.data, tail as usize & (N - 1), value).store();
        }
        // Release ordering is sufficient because the reader must read tail before reading the value, and the writer writes the value before updating tail.
        self.tail.store(tail + 1, Ordering::Release);

        tail + 1
    }

    #[inline(always)]
    /// Returns None if the buffer is empty, now overwritten or if n is out of bounds.
    pub(crate) fn try_read(&self, n: usize) -> Option<T> {
        // n >= N: n mustn't wrap around
        // n == N - 1: index n is writing locked
        if n >= N - 1 {
            return None;
        }

        let n = n as u64;

        let tail = self.tail.load(Ordering::Acquire);

        // tail <= n: the slot is uninitialized or writing locked
        let read = tail.checked_sub(n + 1)?;
        let slot = read as usize & (N - 1);

        let mut buf = MaybeUninit::<T>::uninit();
        // SAFETY:
        // - self.data is aligned to the size of ATOMIC_ALIGN.
        // - The size of the buffer is a multiple of the size of ATOMIC_SIZE.
        // - buf is reserved for size_of::<T>() bytes.
        // - index is less than the elements in the buffer.
        // - buf and self.data is not overlap.
        unsafe { PerByteLoad::new(&self.data, slot, 1, &mut buf) }.load();

        // If the value is overwritten while reading, the reader can be teared, so we must check if the value is overwritten after reading.
        if self.tail.load(Ordering::Relaxed) < read + N as u64 {
            // value is not overwritten
            let value = unsafe { buf.assume_init() };
            Some(value)
        } else {
            // value is overwritten while reading
            None
        }
    }

    /// Returns None if the buffer is empty, now overwritten or if the range is invalid.
    ///
    /// # Panics
    /// This function panics if the Sink implementation panics, for example, if the buffer is a slice or array and the range is out of bounds.
    #[inline(always)]
    pub(crate) fn try_read_range<S, R>(&self, range: R, buf: &mut S) -> Option<()>
    where
        S: Sink<Item = T> + ?Sized,
        R: RangeBounds<usize>,
    {
        let start = match range.start_bound() {
            core::ops::Bound::Included(&n) => n,
            core::ops::Bound::Excluded(&n) => n + 1,
            core::ops::Bound::Unbounded => 0,
        };

        let tail = self.tail.load(Ordering::Acquire);

        let end = match range.end_bound() {
            core::ops::Bound::Included(&n) => n + 1,
            core::ops::Bound::Excluded(&n) => n,
            core::ops::Bound::Unbounded => (N - 1).min(tail as usize),
        };

        // start >= end: range is empty or invalid
        // end > N: index end is out of bounds
        // end == N: index end is writing locked
        if start >= end || end >= N {
            return None;
        }

        let diff = end - start;

        let len = buf.len();

        let cap = buf.remain();

        // reserve additional capacity if needed.
        if let Some(lack) = diff.checked_sub(cap + 1) {
            buf.reserve(lack + 1);
        }

        // buffer is uninitialized
        // tail < end: the slots are uninitialized or writing locked
        let read_old = tail.checked_sub(end as u64)?;
        let slot_old = read_old as usize & (N - 1);

        // SAFETY:
        // - self.data is aligned to the size of ATOMIC_ALIGN.
        // - The size of the buffer is a multiple of the size of ATOMIC_SIZE.
        // - buf is reserved for size_of::<T>() * diff bytes.
        // - index is less than the elements in the buffer.
        // - buf and self.data is not overlap.
        unsafe { PerByteLoad::new(&self.data, slot_old, diff, buf) }.load();

        // If the value is overwritten while reading, the reader can be teared, so we must check if the value is overwritten after reading.
        if self.tail.load(Ordering::Relaxed) < read_old + N as u64 {
            // value is not overwritten
            unsafe {
                buf.set_len(len + diff);
            }
            Some(())
        } else {
            // value is overwritten while reading
            None
        }
    }

    #[inline(always)]
    /// Returns None if n is out of bounds.
    /// This function may loop if the value is overwritten while reading, but it will never block the thread.
    pub(crate) fn read(&self, n: usize) -> Option<T> {
        // n >= N: n mustn't wrap around
        // n == N - 1: index n is writing locked
        if n >= N - 1 {
            return None;
        }

        let n = n as u64;

        let mut tail = self.tail.load(Ordering::Acquire);

        let mut read = loop {
            // tail <= n: the slot is uninitialized or writing locked
            if let Some(r) = tail.checked_sub(n + 1) {
                break r;
            } else {
                // buffer is uninitialized, so we need to wait until the writer writes the first value.
                tail = self.tail.load(Ordering::Acquire);

                spin_loop();
            }
        };

        loop {
            let slot = read as usize & (N - 1);

            let mut buf = MaybeUninit::<T>::uninit();
            // SAFETY:
            // - self.data is aligned to the size of ATOMIC_ALIGN.
            // - The size of the buffer is a multiple of the size of ATOMIC_SIZE.
            // - buf is reserved for size_of::<T>() bytes.
            // - index is less than the elements in the buffer.
            // - buf and self.data is not overlap.
            unsafe { PerByteLoad::new(&self.data, slot, 1, &mut buf) }.load();

            tail = self.tail.load(Ordering::Acquire);

            // If the value is overwritten while reading, the reader can be teared, so we must check if the value is overwritten after reading.
            if tail < read + N as u64 {
                // value is not overwritten
                let value = unsafe { buf.assume_init() };
                return Some(value);
            } else {
                // value is overwritten while reading, so we need to read the next value.

                // tail >= n + 1 is already checked, so tail - (n + 1) won't underflow.
                read = tail - (n + 1);
            }
        }
    }

    #[inline(always)]
    /// Returns None if the range is invalid.
    /// This function may loop if the value is overwritten while reading, but it will never block the thread.
    ///
    /// # Panics
    /// This function panics if the Sink implementation panics, for example, if the buffer is a slice or array and the range is out of bounds.
    pub(crate) fn read_range<R, S>(&self, range: R, buf: &mut S) -> Option<()>
    where
        S: Sink<Item = T> + ?Sized,
        R: RangeBounds<usize>,
    {
        let start = match range.start_bound() {
            core::ops::Bound::Included(&n) => n,
            core::ops::Bound::Excluded(&n) => n + 1,
            core::ops::Bound::Unbounded => 0,
        };

        let mut tail = self.tail.load(Ordering::Acquire);

        let end = match range.end_bound() {
            core::ops::Bound::Included(&n) => n + 1,
            core::ops::Bound::Excluded(&n) => n,
            core::ops::Bound::Unbounded => (N - 1).min((tail as usize).max(1)),
        };

        // start > end: range is empty or invalid
        // end >= N: index end is out of bounds
        // end == N - 1: index end is writing locked
        if start >= end || end >= N {
            return None;
        }

        let diff = end - start;

        let len = buf.len();

        let cap = buf.remain();

        // reserve additional capacity if needed.
        if let Some(lack) = diff.checked_sub(cap) {
            buf.reserve(lack);
        }

        let end = end as u64;

        // buffer is uninitialized
        // tail <= end: the slots are uninitialized
        // tail < end + 1: tail is writing slot index
        let mut read_old = loop {
            if let Some(r) = tail.checked_sub(end) {
                break r;
            } else {
                // buffer is uninitialized, so we need to wait until the writer writes the first value.
                tail = self.tail.load(Ordering::Acquire);

                spin_loop();
            }
        };

        loop {
            let slot_old = read_old as usize & (N - 1);

            // SAFETY:
            // - self.data is aligned to the size of ATOMIC_ALIGN.
            // - The size of the buffer is a multiple of the size of ATOMIC_SIZE.
            // - buf is reserved for size_of::<T>() * diff bytes.
            // - index is less than the elements in the buffer.
            // - buf and self.data is not overlap.
            unsafe { PerByteLoad::new(&self.data, slot_old, diff, buf) }.load();

            tail = self.tail.load(Ordering::Acquire);

            // If the value is overwritten while reading, the reader can be teared, so we must check if the value is overwritten after reading.
            if tail < read_old + N as u64 {
                // value is not overwritten
                unsafe {
                    buf.set_len(len + diff);
                }
                return Some(());
            } else {
                // value is overwritten while reading, so we need to read the next value.

                read_old = tail - end;
            }
        }
    }

    #[inline(always)]
    /// This function must be called only the writer thread.
    pub(crate) fn read_by_writer(&self, n: usize, tail: u64) -> Option<T> {
        // n >= N: n mustn't wrap around
        if n >= N {
            return None;
        }

        let read = tail.checked_sub(n as u64 + 1)?;
        let slot = read as usize & (N - 1);

        let mut buf = MaybeUninit::<T>::uninit();
        // SAFETY:
        // - self.data is aligned to the size of ATOMIC_ALIGN.
        // - The size of the buffer is a multiple of the size of ATOMIC_SIZE.
        // - buf is reserved for size_of::<T>() bytes.
        // - index is less than the elements in the buffer.
        // - buf and self.data is not overlap.
        unsafe { PerByteLoad::new(&self.data, slot, 1, &mut buf) }.load();

        let value = unsafe { buf.assume_init() };
        Some(value)
    }

    #[inline(always)]
    /// This function must be called only the writer thread.
    pub(crate) fn read_range_by_writer<R, S>(&self, tail: u64, range: R, buf: &mut S) -> Option<()>
    where
        S: Sink<Item = T> + ?Sized,
        R: RangeBounds<usize>,
    {
        let start = match range.start_bound() {
            core::ops::Bound::Included(&n) => n,
            core::ops::Bound::Excluded(&n) => n + 1,
            core::ops::Bound::Unbounded => 0,
        };

        let end = match range.end_bound() {
            core::ops::Bound::Included(&n) => n + 1,
            core::ops::Bound::Excluded(&n) => n,
            core::ops::Bound::Unbounded => N.min((tail as usize).max(1)),
        };

        // start >= end: range is empty or invalid
        // end > N: index end is out of bounds
        if start >= end || end > N {
            return None;
        }

        let diff = end - start;

        let len = buf.len();

        let cap = buf.remain();

        // reserve additional capacity if needed.
        if let Some(lack) = diff.checked_sub(cap + 1) {
            buf.reserve(lack + 1);
        }

        let read_old = tail.checked_sub(end as u64)?;
        let slot_old = read_old as usize & (N - 1);

        // SAFETY:
        // - self.data is aligned to the size of ATOMIC_ALIGN.
        // - The size of the buffer is a multiple of the size of ATOMIC_SIZE.
        // - buf is reserved for size_of::<T>() * diff bytes.
        // - index is less than the elements in the buffer.
        // - buf and self.data is not overlap.
        unsafe { PerByteLoad::new(&self.data, slot_old, diff, buf) }.load();

        // Writer calls this function, so the value is not overwritten while reading.
        unsafe {
            buf.set_len(len + diff);
        }

        Some(())
    }

    #[inline(always)]
    pub(crate) fn tail(&self) -> u64 {
        self.tail.load(Ordering::Relaxed)
    }
}
unsafe impl<T: Send, const N: usize> Send for Buffer<T, N> {}
unsafe impl<T: Send + Copy, const N: usize> Sync for Buffer<T, N> {}

#[cfg(not(feature = "loom"))]
pub(crate) mod slots {
    use core::mem::{ManuallyDrop, MaybeUninit};

    use bytemuck::NoUninit;

    #[repr(transparent)]
    /// Slot\<T> is initialized to zero, so it is safe to cast it to integer types if T does not contain any uninitialized bytes.
    pub(crate) struct Slot<T>(MaybeUninit<T>);

    impl<T: NoUninit> Slot<T> {
        #[inline(always)]
        pub(crate) const fn new() -> Self {
            Self(MaybeUninit::<T>::zeroed())
        }
    }
    unsafe impl<T: Send> Send for Slot<T> {}
    unsafe impl<T: Send + Copy> Sync for Slot<T> {}

    pub(crate) union AtomicBaseAligned<T> {
        _align: ManuallyDrop<MaybeUninit<crate::AtomicBase>>,
        data: ManuallyDrop<T>,
    }
    impl<T> AtomicBaseAligned<T> {
        #[inline(always)]
        pub(crate) const fn new(data: T) -> Self {
            Self {
                data: ManuallyDrop::new(data),
            }
        }
    }
}
