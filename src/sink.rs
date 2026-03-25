#[allow(clippy::len_without_is_empty)]
/// A trait for writing data into a buffer. This is used to abstract over different types of buffers, such as `Vec<T>`, `&mut [T]`, and `[T; N]`.
pub trait Sink {
    type Item: Copy;

    /// Returns the current length of the buffer. For a `Vec<T>`, this is `len()`. For a slice or array, this is the length of the slice or array.
    fn len(&self) -> usize;

    /// Returns the remaining capacity of the buffer. For a `Vec<T>`, this is `capacity() - len()`. For a slice or array, this is the length of the slice or array.
    fn remain(&self) -> usize;

    /// Reserves additional capacity for the buffer. This is only applicable for `Vec<T>`, and should panic for slices and arrays.
    fn reserve(&mut self, additional: usize);

    /// Truncates the buffer to the specified length.
    fn truncate(&mut self, len: usize);

    /// Extends the buffer with the given slice, starting at the specified index. The caller must ensure that the buffer has enough capacity to hold the new data.
    ///
    /// # Safety
    /// This ensures the following invariants, so we can assume them when implementing this method:
    /// - The buffer has enough capacity to hold the new data.
    /// - The source and destination must not overlap.
    unsafe fn extend(&mut self, start: usize, src: &[Self::Item]);
}

#[cfg(feature = "alloc")]
impl<T: Copy> Sink for alloc::vec::Vec<T> {
    type Item = T;

    #[inline(always)]
    fn len(&self) -> usize {
        self.len()
    }

    #[inline(always)]
    fn remain(&self) -> usize {
        self.capacity() - self.len()
    }

    #[inline(always)]
    fn reserve(&mut self, additional: usize) {
        self.reserve(additional);
    }

    #[inline(always)]
    fn truncate(&mut self, len: usize) {
        // SAFETY: T: Copy, so we can safely set the length to a smaller value without dropping any elements.
        unsafe {
            self.set_len(len);
        }
    }

    #[inline(always)]
    unsafe fn extend(&mut self, start: usize, src: &[Self::Item]) {
        // SAFETY: the caller must ensure that the buffer has enough capacity to hold the new data, and that the source and destination do not overlap.
        unsafe {
            let dst = self.as_mut_ptr().add(start);

            dst.copy_from_nonoverlapping(src.as_ptr(), src.len());

            self.set_len(start + src.len());
        }
    }
}

impl<T: Copy + Default> Sink for [T] {
    type Item = T;

    #[inline(always)]
    fn len(&self) -> usize {
        0
    }

    #[inline(always)]
    fn remain(&self) -> usize {
        self.len()
    }

    #[cold]
    fn reserve(&mut self, _additional: usize) {
        unreachable!("cannot reserve additional capacity for a slice");
    }

    #[inline(always)]
    fn truncate(&mut self, len: usize) {
        self[len..].fill(T::default());
    }

    #[inline(always)]
    unsafe fn extend(&mut self, start: usize, src: &[Self::Item]) {
        // SAFETY: the caller must ensure that the buffer has enough capacity to hold the new data, and that the source and destination do not overlap.
        unsafe {
            let dst = self.as_mut_ptr().add(start);

            dst.copy_from_nonoverlapping(src.as_ptr(), src.len());
        }
    }
}

impl<T: Copy + Default, const N: usize> Sink for [T; N] {
    type Item = T;

    #[inline(always)]
    fn len(&self) -> usize {
        0
    }

    #[inline(always)]
    fn remain(&self) -> usize {
        N
    }

    #[cold]
    fn reserve(&mut self, _additional: usize) {
        unreachable!("cannot reserve additional capacity for an array");
    }

    #[inline(always)]
    fn truncate(&mut self, len: usize) {
        self[len..].fill(T::default());
    }

    #[inline(always)]
    unsafe fn extend(&mut self, start: usize, src: &[Self::Item]) {
        // SAFETY: the caller must ensure that the buffer has enough capacity to hold the new data, and that the source and destination do not overlap.
        unsafe {
            let dst = self.as_mut_ptr().add(start);

            dst.copy_from_nonoverlapping(src.as_ptr(), src.len());
        }
    }
}

#[cfg(feature = "heapless")]
impl<const N: usize, T: Copy> Sink for heapless::Vec<T, N> {
    type Item = T;

    #[inline(always)]
    fn len(&self) -> usize {
        self.as_slice().len()
    }

    #[inline(always)]
    fn remain(&self) -> usize {
        self.capacity() - self.len()
    }

    #[cold]
    fn reserve(&mut self, _additional: usize) {
        unreachable!("heapless::Vec does not support reserving additional capacity");
    }

    #[inline(always)]
    fn truncate(&mut self, len: usize) {
        // SAFETY: T: Copy, so we can safely set the length to a smaller value without dropping any elements.
        unsafe {
            self.set_len(len);
        }
    }

    #[inline(always)]
    unsafe fn extend(&mut self, start: usize, src: &[Self::Item]) {
        // SAFETY: the caller must ensure that the buffer has enough capacity to hold the new data, and that the source and destination do not overlap.
        unsafe {
            let dst = self.as_mut_ptr().add(start);

            dst.copy_from_nonoverlapping(src.as_ptr(), src.len());

            self.set_len(start + src.len());
        }
    }
}

#[cfg(feature = "arrayvec")]
impl<T: Copy, const N: usize> Sink for arrayvec::ArrayVec<T, N> {
    type Item = T;

    #[inline(always)]
    fn len(&self) -> usize {
        self.len()
    }

    #[inline(always)]
    fn remain(&self) -> usize {
        self.capacity() - self.len()
    }

    #[cold]
    fn reserve(&mut self, _additional: usize) {
        unreachable!("arrayvec::ArrayVec does not support reserving additional capacity");
    }

    #[inline(always)]
    fn truncate(&mut self, len: usize) {
        // SAFETY: T: Copy, so we can safely set the length to a smaller value without dropping any elements.
        unsafe {
            self.set_len(len);
        }
    }

    #[inline(always)]
    unsafe fn extend(&mut self, start: usize, src: &[Self::Item]) {
        // SAFETY: the caller must ensure that the buffer has enough capacity to hold the new data, and that the source and destination do not overlap.
        unsafe {
            let dst = self.as_mut_ptr().add(start);

            dst.copy_from_nonoverlapping(src.as_ptr(), src.len());

            self.set_len(start + src.len());
        }
    }
}
