use bytemuck::NoUninit;

use core::ops::RangeBounds;

use crate::{Arc, buffer::Buffer, sink::Sink};

#[inline]
/// Creates a new buffer and returns a writer and a reader.
///
/// - N must be greater than 2, because the writer must not write to the slot that the reader is reading.
///
/// - N must be a power of 2 for better performance.
///
/// - T must be [`bytemuck::NoUninit`], because the reader can read the value while the writer is writing and the value mustn't be uninitialized.
///
/// - `size_of::<T>()` * N must be a multiple of the size of [`crate::ATOMIC_SIZE`].
///
/// - `size_of::<T>()` must be greater than 0.
///
/// # Panics
/// This function panics if any of the above conditions are not met.
///
/// # Example
/// ```rust,no_run
/// # use cyclotrace::create_buffer;
/// let (mut writer, reader) = create_buffer::<u32, 1024>();
/// writer.write(42);
/// assert_eq!(reader.get(0), Some(42));
/// ```
pub fn create_buffer<T: NoUninit, const N: usize>() -> (BufWriter<T, N>, BufReader<T, N>) {
    let buf = Arc::new(Buffer::new());
    (
        BufWriter {
            buf: buf.clone(),
            tail: 0,
            _marker: core::marker::PhantomData,
        },
        BufReader { buf },
    )
}

/// A writer for the buffer. This struct is not thread-safe, so it must be used by only one thread.
pub struct BufWriter<T, const N: usize> {
    buf: Arc<Buffer<T, N>>,
    tail: u64,
    _marker: core::marker::PhantomData<*const ()>,
}
impl<T: NoUninit, const N: usize> BufWriter<T, N> {
    #[inline]
    /// Writes a value to the buffer. This function must be called only one thread.
    ///
    /// If the buffer is full, the oldest value will be overwritten.
    ///
    /// This function is wait-free, so it will never block the thread.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use cyclotrace::{create_buffer, BufWriter};
    /// let (mut writer, _reader) = create_buffer::<u32, 1024>();
    /// writer.write(42);
    /// ```
    pub fn write(&mut self, value: T) {
        // SAFETY: only one thread can call this function, so no concurrent write is possible.
        unsafe {
            self.tail = self.buf.write(value, self.tail);
        }
    }

    #[inline]
    /// Reads the value at the n-th position from the latest value. n must be less than N.
    ///
    /// Returns None if the buffer is empty, or if n is out of bounds.
    ///
    /// This function is always wait-free, even when the atomic operations aren't wait-free, so it will never block the thread.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use cyclotrace::{create_buffer, BufWriter};
    /// let (mut writer, _reader) = create_buffer::<u32, 1024>();
    /// writer.write(42);
    /// assert_eq!(writer.read(0), Some(42));
    /// ```
    pub fn read(&self, n: usize) -> Option<T> {
        self.buf.read_by_writer(n, self.tail)
    }

    #[inline]
    /// Gets the latest value. Returns None if the buffer is empty.
    /// This is a convenience method for [`Self::read(0)`](Self::read).
    pub fn read_latest(&self) -> Option<T> {
        self.read(0)
    }

    #[inline]
    /// Gets the values in the specified range. The range is specified by the n-th position from the latest value, and the range must be less than N.
    ///
    /// Returns None if the range is invalid.
    ///
    /// This function is always wait-free, even when the atomic operations aren't wait-free, so it will never block the thread.
    pub fn read_range<S, R>(&self, range: R, buf: &mut S) -> Option<()>
    where
        S: Sink<Item = T> + ?Sized,
        R: RangeBounds<usize>,
    {
        self.buf.read_range_by_writer(self.tail, range, buf)
    }
}
// SAFETY: BufWriter is not thread-safe, but it can be sent to another thread.
unsafe impl<T: NoUninit + Send, const N: usize> Send for BufWriter<T, N> {}

#[derive(Clone)]
/// A reader for the buffer. This struct is thread-safe, so it can be used by multiple threads.
///
/// [`BufReader::clone`] creates a new reader that shares the same buffer, and clone is cheap.
pub struct BufReader<T, const N: usize> {
    buf: Arc<Buffer<T, N>>,
}
impl<T: NoUninit, const N: usize> BufReader<T, N> {
    #[inline]
    /// Returns true if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    #[inline]
    /// Gets the value at the n-th position from the latest value. n must be less than N - 1.
    ///
    /// Returns None if the buffer is empty, now overwritten or if n is out of bounds.
    ///
    /// This function is wait-free, so it will never block the thread.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use cyclotrace::{create_buffer, BufReader};
    /// let (mut writer, reader) = create_buffer::<u32, 1024>();
    /// writer.write(42);
    /// assert_eq!(reader.try_read(0), Some(42));
    /// ```
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
    /// Gets the value at the n-th position from the latest value. n must be less than N - 1.
    ///
    /// Returns None if n is out of bounds.
    ///
    /// This function is lock-free, so it will never block the thread, but it may loop if the value is overwritten while reading.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use cyclotrace::{create_buffer, BufReader};
    /// let (mut writer, reader) = create_buffer::<u32, 1024>();
    ///
    /// let h = std::thread::spawn(move || {
    ///     writer.write(42);
    /// });
    ///
    /// assert_eq!(reader.read(0), Some(42));
    ///
    /// h.join().unwrap();
    /// ```
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
    ///
    /// # Example
    /// ```rust,no_run
    /// # use cyclotrace::{create_buffer, BufReader};
    /// let (mut writer, reader) = create_buffer::<u32, 1024>();
    /// writer.write(1);
    /// writer.write(2);
    /// writer.write(3);
    /// let mut buf = Vec::new();
    /// let _ = reader.try_read_range(.., &mut buf);
    /// assert_eq!(buf, vec![1, 2, 3]);
    /// ```
    pub fn try_read_range<S, R>(&self, range: R, buf: &mut S) -> Option<()>
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
    pub fn get_range<S, R>(&self, range: R, buf: &mut S) -> Option<()>
    where
        S: Sink<Item = T> + ?Sized,
        R: RangeBounds<usize>,
    {
        self.buf.try_read_range(range, buf)
    }

    #[inline]
    #[must_use]
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
    pub fn read_range<S, R>(&self, range: R, buf: &mut S) -> Option<()>
    where
        S: Sink<Item = T> + ?Sized,
        R: RangeBounds<usize>,
    {
        self.buf.read_range(range, buf)
    }
}
