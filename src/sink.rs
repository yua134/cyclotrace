use bytemuck::NoUninit;

#[allow(clippy::len_without_is_empty)]
/// A trait for writing data into a buffer. This is used to abstract over different types of buffers, like `Vec<T>`.
pub trait Sink {
    type Item: NoUninit;

    /// Returns the current length of the buffer. For a `Vec<T>`, this is `len()`.
    fn len(&self) -> usize;

    /// Returns the remaining capacity of the buffer. For a `Vec<T>`, this is `capacity() - len()`.
    fn remain(&self) -> usize;

    /// Reserves additional capacity for the buffer.
    fn reserve(&mut self, additional: usize);

    /// Returns a raw pointer to the beginning of the buffer.
    fn as_mut_ptr(&mut self) -> *mut Self::Item;

    /// Sets the length of the buffer. For a `Vec<T>`, this is `set_len()`.
    /// # Safety
    /// The caller must ensure that the new elements are initialized and new length does not exceed the capacity of the buffer.
    unsafe fn set_len(&mut self, len: usize);
}

pub(crate) trait SinkInner {
    fn as_mut_ptr(&mut self) -> *mut u8;
}
impl<T: Sink + ?Sized> SinkInner for T {
    #[inline(always)]
    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.as_mut_ptr() as *mut u8
    }
}
impl<T> SinkInner for core::mem::MaybeUninit<T> {
    #[inline(always)]
    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.as_mut_ptr() as *mut u8
    }
}

#[cfg(feature = "alloc")]
impl<T: NoUninit> Sink for alloc::vec::Vec<T> {
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
    fn as_mut_ptr(&mut self) -> *mut Self::Item {
        self.as_mut_ptr()
    }

    #[inline(always)]
    unsafe fn set_len(&mut self, len: usize) {
        unsafe {
            self.set_len(len);
        }
    }
}

#[cfg(feature = "heapless")]
impl<const N: usize, T: NoUninit> Sink for heapless::Vec<T, N> {
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
    fn as_mut_ptr(&mut self) -> *mut Self::Item {
        self.as_mut_ptr()
    }

    #[inline(always)]
    unsafe fn set_len(&mut self, len: usize) {
        unsafe {
            self.set_len(len);
        }
    }
}

#[cfg(feature = "arrayvec")]
impl<T: NoUninit, const N: usize> Sink for arrayvec::ArrayVec<T, N> {
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
    fn as_mut_ptr(&mut self) -> *mut Self::Item {
        self.as_mut_ptr()
    }

    #[inline(always)]
    unsafe fn set_len(&mut self, len: usize) {
        unsafe {
            self.set_len(len);
        }
    }
}
