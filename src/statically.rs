use core::{ops::RangeBounds, sync::atomic::Ordering};

use bytemuck::NoUninit;

use crate::{AtomicBool, buffer::Buffer, sink::Sink};

/// A static buffer that can be used in a static context.
pub struct StaticBuffer<T, const N: usize> {
    buf: Buffer<T, N>,
    writer: AtomicBool,
}
impl<T: NoUninit, const N: usize> StaticBuffer<T, N> {
    /// Creates a new statical buffer.
    ///
    /// N must be greater than 2, because the writer must not write to the slot that the reader is reading.
    ///
    /// N must be a power of 2 for better performance.
    ///
    /// T must be [`bytemuck::NoUninit`], because the reader can read the value while the writer is writing and the value mustn't be uninitialized.
    ///
    /// `size_of::<T>()` * N must be a multiple of the size of [`crate::ATOMIC_SIZE`].
    ///
    /// # Panics
    /// This function panics if any of the above conditions are not met.
    #[allow(clippy::new_without_default)]
    #[inline]
    #[cfg(not(feature = "loom"))]
    pub const fn new() -> Self {
        Self {
            buf: Buffer::new(),
            writer: AtomicBool::new(false),
        }
    }

    #[allow(clippy::new_without_default)]
    #[cfg(feature = "loom")]
    pub fn new() -> Self {
        Self {
            buf: Buffer::new(),
            writer: AtomicBool::new(false),
        }
    }

    #[inline]
    /// Checks if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    #[inline]
    /// Gets a writer for the buffer. This function returns `None` if there is already a writer.
    pub fn get_writer(&self) -> Option<Writer<'_, T, N>> {
        // Buffer::write() requires a synchronization with the previous write, so a happens-before relationship is necessary to synchronize the writer's drop or the write.
        let writer = self.writer.swap(true, Ordering::Acquire);
        if writer {
            None
        } else {
            let tail = self.buf.tail();
            Some(Writer {
                buf: self,
                tail,
                _marker: core::marker::PhantomData,
            })
        }
    }

    #[must_use]
    #[inline]
    /// Gets the value at the n-th position from the latest value. n must be less than N - 1.
    ///
    /// Returns None if there is already a writer.
    ///
    /// This function is not recommended for use in hot paths, as it performs a atomic swap for every write. We recommend using [Self::get_writer](Self::get_writer) instead.
    pub fn write(&self, value: T) -> Option<()> {
        // Buffer::write() requires a synchronization with the previous write, so a happens-before relationship is necessary to synchronize the writer's drop or the write.
        let writer = self.writer.swap(true, Ordering::Acquire);
        if writer {
            None
        } else {
            // SAFETY: only one thread can call this function, so no concurrent write is possible.
            unsafe {
                let tail = self.buf.tail();
                let _ = self.buf.write(value, tail);
            }
            self.writer.store(false, Ordering::Release);
            Some(())
        }
    }

    #[inline]
    /// Gets the value at the n-th position from the latest value. n must be less than N - 1.
    ///
    /// Returns None if the buffer is empty, now overwritten or if n is out of bounds
    ///
    /// This function is wait-free, so it will never block the thread.
    pub fn try_read(&self, n: usize) -> Option<T> {
        self.buf.try_read(n)
    }

    #[deprecated(
        since = "0.2.0",
        note = "This function will be removed in the next version. Use `try_read` instead."
    )]
    #[inline]
    /// This function is deprecated. Use [`Self::try_read`](Self::try_read) instead.
    pub fn get(&self, n: usize) -> Option<T> {
        self.buf.try_read(n)
    }

    #[inline]
    /// Gets the latest value. Returns None if the buffer is empty or now overwritten.
    ///
    /// This function is wait-free, so it will never block the thread.
    ///
    /// This is a convenience method for [`Self::try_read(0)`](Self::try_read).
    pub fn try_read_latest(&self) -> Option<T> {
        self.try_read(0)
    }

    #[deprecated(
        since = "0.2.0",
        note = "This function will be removed in the next version. Use `try_read_latest` instead."
    )]
    #[inline]
    /// This function is deprecated. Use [`Self::try_read_latest`](Self::try_read_latest) instead.
    pub fn get_latest(&self) -> Option<T> {
        self.try_read_latest()
    }

    #[inline]
    #[must_use]
    /// Gets the values in the specified range. The range is specified by the n-th position from the latest value, and the range must be less than N - 1.
    ///
    /// Returns None if the buffer is empty, now overwritten or if the range is invalid.
    ///
    /// This function is wait-free, so it will never block the thread.
    ///
    /// Provides an atomic snapshot of the specified range. Unlike calling [`Self::try_read`](Self::try_read) multiple times, this ensures all returned values are consistent with the same buffer state, preventing temporal inconsistency (tearing) across different slots.
    ///
    /// Attempting to read a range equivalent to the entire buffer size may increase the frequency of returning None. This is because the reader is more likely to be preempted by the writer while reading, which increases the chance of the buffer being overwritten during the read operation.
    ///
    /// # Panics
    /// This function panics if the Sink implementation panics, for example, if the buffer is a slice or array and the range is out of bounds. See implementations of [Sink](crate::Sink) for more details.
    pub fn try_read_range<R, S>(&self, range: R, buf: &mut S) -> Option<()>
    where
        S: Sink<Item = T> + ?Sized,
        R: RangeBounds<usize>,
    {
        self.buf.try_read_range(range, buf)
    }

    #[deprecated(
        since = "0.2.0",
        note = "This function will be removed in the next version. Use `try_read_range` instead."
    )]
    #[inline]
    /// This function is deprecated. Use [`Self::try_read_range`](Self::try_read_range) instead.
    pub fn get_range<R, S>(&self, range: R, buf: &mut S) -> Option<()>
    where
        S: Sink<Item = T> + ?Sized,
        R: RangeBounds<usize>,
    {
        self.buf.try_read_range(range, buf)
    }

    #[inline]
    /// Gets the value at the n-th position from the latest value. n must be less than N - 1.
    ///
    /// Returns None if n is out of bounds.
    ///
    /// This function is lock-free, so it will never block the thread, but it may loop if the value is overwritten while reading.
    pub fn read(&self, n: usize) -> Option<T> {
        self.buf.read(n)
    }

    #[inline]
    /// Gets the latest value.
    ///
    /// This function is lock-free, so it will never block the thread, but it may loop if the value is overwritten while reading.
    ///
    /// This is a convenience method for [`Self::read(0)`](Self::read).
    pub fn read_latest(&self) -> T {
        // SAFETY: 0 is always less than N - 1, so this function won't return None.
        self.read(0).unwrap()
    }

    #[must_use]
    #[inline]
    /// Gets the values in the specified range. The range is specified by the n-th position from the latest value, and the range must be less than N - 1.
    ///
    /// Returns None if the range is invalid.
    ///
    /// This function is lock-free, so it will never block the thread, but it may loop if the value is overwritten while reading.
    ///
    /// Provides an atomic snapshot of the specified range. Unlike calling [`Self::read`](Self::read) multiple times, this ensures all returned values are consistent with the same buffer state, preventing temporal inconsistency (tearing) across different slots.
    ///
    /// Attempting to read a range equivalent to the entire buffer size may increase the frequency of returning None. This is because the reader is more likely to be preempted by the writer while reading, which increases the chance of the buffer being overwritten during the read operation.
    ///
    /// # Panics
    /// This function panics if the Sink implementation panics, for example, if the buffer is a slice or array and the range is out of bounds. See implementations of [Sink](crate::Sink) for more details.
    pub fn read_range<R, S>(&self, range: R, buf: &mut S) -> Option<()>
    where
        S: Sink<Item = T> + ?Sized,
        R: RangeBounds<usize>,
    {
        self.buf.read_range(range, buf)
    }
}

/// A writer for the buffer. This struct is not thread-safe, so it must be used by only one thread.
pub struct Writer<'a, T: 'a, const N: usize> {
    buf: &'a StaticBuffer<T, N>,
    tail: u64,
    _marker: core::marker::PhantomData<core::cell::UnsafeCell<()>>,
}
impl<'a, T: NoUninit + 'a, const N: usize> Writer<'a, T, N> {
    #[inline]
    /// Writes a value to the buffer. This function must be called only one thread.
    ///
    /// If the buffer is full, the oldest value will be overwritten.
    ///
    /// This function is wait-free, so it will never block the thread.
    pub fn write(&mut self, value: T) {
        // SAFETY: only one thread can call this function, so no concurrent write is possible.
        unsafe {
            self.tail = self.buf.buf.write(value, self.tail);
        }
    }

    #[inline]
    pub fn read(&self, n: usize) -> Option<T> {
        self.buf.buf.read_by_writer(n, self.tail)
    }

    #[inline]
    pub fn read_latest(&self) -> Option<T> {
        self.read(0)
    }

    #[inline]
    pub fn read_range<S, R>(&self, range: R, buf: &mut S) -> Option<()>
    where
        S: Sink<Item = T> + ?Sized,
        R: RangeBounds<usize>,
    {
        self.buf.buf.read_range_by_writer(self.tail, range, buf)
    }
}
impl<'a, T: 'a, const N: usize> Drop for Writer<'a, T, N> {
    #[inline]
    fn drop(&mut self) {
        self.buf.writer.store(false, Ordering::Release);
    }
}
unsafe impl<'a, T: 'a + NoUninit + Send, const N: usize> Send for Writer<'a, T, N> {}
