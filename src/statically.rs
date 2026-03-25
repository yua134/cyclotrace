use core::{cell::Cell, ops::RangeBounds, sync::atomic::Ordering};

use super::AtomicBool;

use crate::{buffer::Buffer, sink::Sink};

#[macro_export]
/// Creates a static buffer with the given name, type and size.
///
/// N must be greater than 1, because the writer must not write to the slot that the reader is reading.
///
/// N must be a power of 2 for better performance.
///
/// T must be Copy, because the reader can read the value while the writer is writing.
///
/// # Example
/// ```rust,ignore
/// # use cyclotrace::static_buffer;
///
/// static_buffer!(pub static MY_BUF::<u32, 1024>);
///
/// let writer = MY_BUF.get_writer().unwrap();
/// writer.write(42);
///
/// assert_eq!(MY_BUF.get(0), Some(42));
/// ```
macro_rules! static_buffer {
    ($vis:vis static $name:ident::<$ty:ty, $lit:literal>) => {
        $vis static $name: $crate::StaticBuffer<$ty, $lit> = $crate::StaticBuffer::new();
    };
}

/// A static buffer that can be used in a static context.
pub struct StaticBuffer<T, const N: usize> {
    buf: Buffer<T, N>,
    writer: AtomicBool,
}
impl<T: Copy, const N: usize> StaticBuffer<T, N> {
    cfg_if::cfg_if!(
    if #[cfg(feature = "loom")] {
        pub fn new() -> Self {
            Self {
                buf: Buffer::new(),
                writer: AtomicBool::new(false),
            }
        }
    } else {
        #[allow(clippy::new_without_default)]
        #[inline]
        /// Creates a new static buffer.
        ///
        /// As an alternative to this function, a macro is also available. See [`static_buffer!`](static_buffer!) for more details.
        pub const fn new() -> Self {
            Self {
                buf: Buffer::new(),
                writer: AtomicBool::new(false),
            }
        }
    });

    #[inline]
    /// Checks if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    #[inline]
    /// Gets a writer for the buffer. This function returns `None` if there is already a writer.
    pub fn get_writer(&'static self) -> Option<Writer<T, N>> {
        if self
            .writer
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            Some(Writer {
                buf: self,
                _marker: core::marker::PhantomData,
            })
        } else {
            None
        }
    }

    #[must_use]
    #[inline]
    /// Gets the value at the n-th position from the latest value. n must be less than N - 1.
    ///
    /// Returns None if there is already a writer.
    ///
    /// This function is not recommended for use in hot paths, as it performs a CAS (Compare-And-Swap) for every write. We recommend using [Self::get_writer](Self::get_writer) instead.
    pub fn write(&self, value: T) -> Option<()> {
        if self
            .writer
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            self.buf.write(value);
            self.writer.store(false, Ordering::Release);
            Some(())
        } else {
            None
        }
    }

    #[inline]
    /// Gets the value at the n-th position from the latest value. n must be less than N - 1.
    ///
    /// Returns None if the buffer is empty, now overwritten or if n is out of bounds
    ///
    /// This function is wait-free, so it will never block the thread.
    pub fn get(&self, n: usize) -> Option<T> {
        self.buf.get(n)
    }

    #[inline]
    /// Gets the latest value. Returns None if the buffer is empty or now overwritten.
    ///
    /// This function is wait-free, so it will never block the thread.
    ///
    /// This is a convenience method for [`Self::get(0)`](Self::get).
    pub fn get_latest(&self) -> Option<T> {
        self.get(0)
    }

    #[inline]
    #[must_use]
    /// Gets the values in the specified range. The range is specified by the n-th position from the latest value, and the range must be less than N - 1.
    ///
    /// Returns None if the buffer is empty, now overwritten or if the range is invalid.
    ///
    /// This function is wait-free, so it will never block the thread.
    ///
    /// Provides an atomic snapshot of the specified range. Unlike calling [`Self::get`](Self::get) multiple times, this ensures all returned values are consistent with the same buffer state, preventing temporal inconsistency (tearing) across different slots.
    ///
    /// Attempting to read a range equivalent to the entire buffer size may increase the frequency of returning None. This is because the reader is more likely to be preempted by the writer while reading, which increases the chance of the buffer being overwritten during the read operation.
    ///
    /// # Panics
    /// This function panics if the Sink implementation panics, for example, if the buffer is a slice or array and the range is out of bounds. See implementations of [Sink](crate::Sink) for more details.
    pub fn get_range<R, S>(&self, range: R, buf: &mut S) -> Option<()>
    where
        S: Sink<Item = T> + ?Sized,
        R: RangeBounds<usize>,
    {
        self.buf.get_range(range, buf)
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

pub struct Writer<T: 'static, const N: usize> {
    buf: &'static StaticBuffer<T, N>,
    _marker: core::marker::PhantomData<Cell<()>>,
}
impl<T: Copy + 'static, const N: usize> Writer<T, N> {
    #[inline]
    pub fn write(&self, value: T) {
        self.buf.buf.write(value);
    }
}
impl<T: 'static, const N: usize> Drop for Writer<T, N> {
    #[inline]
    fn drop(&mut self) {
        self.buf.writer.store(false, Ordering::Release);
    }
}
