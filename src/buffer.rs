use core::{
    mem::MaybeUninit,
    ops::RangeBounds,
};

use super::{AtomicU64, Ordering, UnsafeCell, spin_loop};

use crossbeam_utils::CachePadded;

use crate::{sink::Sink, util::init_array};

pub(crate) struct Buffer<T, const N: usize> {
    data: [CachePadded<Slot<T>>; N],
    tail: CachePadded<AtomicU64>,
}
impl<T: Copy, const N: usize> Buffer<T, N> {
    #[cfg_attr(feature = "loom", maybe_const::maybe_const)]
    #[inline(always)]
    pub(crate) const fn new() -> Self {
        const {
            assert!(N > 1, "Buffer size must be greater than 1");
            assert!(N.is_power_of_two(), "Buffer size should be a power of 2 for better performance");
        }
        Self {
            data: init_array(),
            tail: CachePadded::new(AtomicU64::new(0)),
        }
    }

    #[inline(always)]
    pub(crate) fn is_empty(&self) -> bool {
        self.tail.load(Ordering::Acquire) == 0
    }

    #[inline(always)]
    /// This function must be called only one thread.
    pub(crate) fn write(&self, value: T) {
        // SAFETY: only one thread can call this function, so no concurrent write is possible.
        let tail = self.tail.load(Ordering::Relaxed);
        // N is a power of 2, so we can use bitwise AND instead of modulo to get the index.
        // tail as usize % N is equivalent to tail & (N - 1) as u64, but the latter is much faster.
        self.data[tail as usize & (N - 1)].write(value);
        // Release ordering is sufficient because the reader must read tail before reading the value, and the writer writes the value before updating tail.
        self.tail.store(tail + 1, Ordering::Release);
    }

    #[inline(always)]
    /// Returns None if the buffer is empty, now overwritten or if n is out of bounds.
    pub(crate) fn get(&self, n: usize) -> Option<T> {
        // n >= N: n mustn't wrap around
        // n == N - 1: index n is writing locked
        if n >= N - 1 {
            return None;
        }

        let n = n as u64;

        let tail = self.tail.load(Ordering::Acquire);

        // tail <= n: the slot is uninitialized
        // tail < n + 1: tail is writing slot index
        let read = tail.checked_sub(n + 1)?;
        let slot = read as usize & (N - 1);

        // SAFETY: this slot is not writing locked because previous branches ensure that n < N - 1, so the writer is writing to the next slot.
        let value = self.data[slot].read();

        // If the value is overwritten while reading, the reader can be teared, so we must check if the value is overwritten after reading.
        if self.tail.load(Ordering::Acquire) < read + N as u64 {
            // value is not overwritten
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
    pub(crate) fn get_range<S, R>(&self, range: R, buf: &mut S) -> Option<()>
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
            core::ops::Bound::Unbounded => N - 2,
        };

        // start > end: range is empty or invalid
        // end >= N: index end is out of bounds
        // end == N - 1: index end is writing locked
        if start > end || end >= N - 1 {
            return None;
        }

        let diff = end - start;

        let len = buf.len();

        let cap = buf.remain();

        // reserve additional capacity if needed.
        if let Some(lack) = diff.checked_sub(cap) {
            buf.reserve(lack);
        }

        let tail = self.tail.load(Ordering::Acquire);

        // buffer is uninitialized
        // tail <= end: the slots are uninitialized
        // tail < end + 1: tail is writing slot index
        let read_old = tail.checked_sub(end as u64 + 1)?;
        let read_latest = tail as usize - (start + 1);
        let slot_old = read_old as usize & (N - 1);
        let slot_latest = read_latest & (N - 1);

        // ring buffer is not wrapped around
        if slot_latest < slot_old {
            let slice1 = &self.data[slot_old..];
            let slice2 = &self.data[..=slot_latest];

            for (i, slot) in slice1.iter().chain(slice2.iter()).enumerate() {
                buf.push(slot.read(), i);
            }
        } else {
            // SAFETY: Slot<T> is #[repr(transparent)], so &[Slot<T>] can be transmuted to &[T].
            let slice = &self.data[slot_old..=slot_latest];

            for (i, slot) in slice.iter().enumerate() {
                buf.push(slot.read(), i);
            }
        }

        // If the value is overwritten while reading, the reader can be teared, so we must check if the value is overwritten after reading.
        if self.tail.load(Ordering::Acquire) < read_old + N as u64 {
            // value is not overwritten
            Some(())
        } else {
            // value is overwritten while reading

            // truncate the buffer to the original length, because the buffer may be partially extended.
            buf.truncate(len);
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
            // tail <= n: the slot is uninitialized
            // tail < n + 1: tail is writing slot index
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

            // SAFETY: this slot is not writing locked because previous branches ensure that n < N - 1, so the writer is writing to the next slot.
            let value = self.data[slot].read();

            tail = self.tail.load(Ordering::Acquire);

            // If the value is overwritten while reading, the reader can be teared, so we must check if the value is overwritten after reading.
            if tail < read + N as u64 {
                // value is not overwritten
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
        let end = match range.end_bound() {
            core::ops::Bound::Included(&n) => n + 1,
            core::ops::Bound::Excluded(&n) => n,
            core::ops::Bound::Unbounded => N - 2,
        };

        // start > end: range is empty or invalid
        // end >= N: index end is out of bounds
        // end == N - 1: index end is writing locked
        if start > end || end >= N - 1 {
            return None;
        }

        let diff = end - start;

        let len = buf.len();

        let cap = buf.remain();

        // reserve additional capacity if needed.
        if let Some(lack) = diff.checked_sub(cap) {
            buf.reserve(lack);
        }

        let mut tail = self.tail.load(Ordering::Acquire);

        let end = end as u64;

        // buffer is uninitialized
        // tail <= end: the slots are uninitialized
        // tail < end + 1: tail is writing slot index
        let mut read_old = loop {
            if let Some(r) = tail.checked_sub(end + 1) {
                break r;
            } else {
                // buffer is uninitialized, so we need to wait until the writer writes the first value.
                tail = self.tail.load(Ordering::Acquire);

                spin_loop();
            }
        };

        loop {
            let read_latest = tail as usize - (start + 1);
            let slot_old = read_old as usize & (N - 1);
            let slot_latest = read_latest & (N - 1);

            // ring buffer is not wrapped around
            if slot_latest < slot_old {
                // SAFETY: Slot<T> is #[repr(transparent)], so &[Slot<T>] can be transmuted to &[T].
                let slice1 = &self.data[slot_old..];
                let slice2 = &self.data[..=slot_latest];

                for (i, slot) in slice1.iter().chain(slice2.iter()).enumerate() {
                    buf.push(slot.read(), i);
                }
            } else {
                // SAFETY: Slot<T> is #[repr(transparent)], so &[Slot<T>] can be transmuted to &[T].
                let slice = &self.data[slot_old..=slot_latest];

                for (i, slot) in slice.iter().enumerate() {
                    buf.push(slot.read(), i);
                }
            }

            tail = self.tail.load(Ordering::Acquire);

            // If the value is overwritten while reading, the reader can be teared, so we must check if the value is overwritten after reading.
            if tail < read_old + N as u64 {
                // value is not overwritten
                return Some(());
            } else {
                // value is overwritten while reading, so we need to read the next value.

                read_old = tail - (end + 1);

                buf.truncate(len);
            }
        }
    }
}

#[repr(transparent)]
pub(crate) struct Slot<T>(UnsafeCell<MaybeUninit<T>>);

impl<T: Copy> Slot<T> {
    #[cfg_attr(feature = "loom", maybe_const::maybe_const)]
    #[inline(always)]
    pub(crate) const fn new() -> Self {
        Self(UnsafeCell::new(MaybeUninit::uninit()))
    }

    #[inline(always)]
    #[cfg(not(feature = "loom"))]
    const fn write(&self, value: T) {
        unsafe {
            self.0.get().write(MaybeUninit::new(value));
        }
    }

    #[inline(always)]
    #[cfg(feature = "loom")]
    fn write(&self, value: T) {
        unsafe {
            self.0.get_mut().deref().write(value);
        }
    }


    #[inline(always)]
    #[cfg(not(feature = "loom"))]
    const fn read(&self) -> T {
        unsafe { (*self.0.get()).assume_init() }
    }

    #[inline(always)]
    #[cfg(feature = "loom")]
    fn read(&self) -> T {
        unsafe { self.0.get().deref().assume_init_read() }
    }
}
unsafe impl<T: Send> Send for Slot<T> {}
unsafe impl<T: Send + Copy> Sync for Slot<T> {}
