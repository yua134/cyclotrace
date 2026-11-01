use bytemuck::NoUninit;

pub struct Cursor<T> {
    last: u64,
    buffer: T
}

impl<T: Clone> Clone for Cursor<T> {
    fn clone(&self) -> Self {
        Self { last: self.last, buffer: self.buffer.clone() }
    }
}

impl<'a, T: NoUninit, const N: usize> Cursor<&'a crate::statically::StaticBuffer<T, N>> {
    #[inline(always)]
    pub(crate) fn as_buffer(&self) -> &crate::buffer::Buffer<T, N> {
        self.buffer.buf()
    }
}

#[cfg(feature = "alloc")]
impl<T: NoUninit, const N: usize> Cursor<crate::ringbuf::BufReader<T, N>> {
    #[inline(always)]
    pub(crate) fn as_buffer(&self) -> &crate::buffer::Buffer<T, N> {
        &self.buffer.buf
    }
}

macro_rules! cursor_impl {
    (
        impl $([$($tt:tt)*])? Cursor<$buffer:ty> {
            type Item = $item:ty;
        }
    ) => {
        impl $(<$($tt)*>)? Cursor<$buffer> {
            #[inline]
            pub const fn new(buffer: $buffer) -> Self {
                Self { last: 0, buffer }
            }

            #[inline]
            pub fn try_recv(&mut self) -> Option<$item> {
                let count = self.last;
                let buffer = self.as_buffer();
                let (value, read) = buffer.try_read_reduction(count)?;
                self.last += read;
                Some(value)
            }

            #[inline]
            pub fn recv(&mut self) -> $item {
                let count = self.last;
                let buffer = self.as_buffer();
                let (value, read) = buffer.read_reduction(count);
                self.last += read;
                value
            }
        }
    };
}

cursor_impl!(
    impl ['a, T: NoUninit, const N: usize] Cursor<&'a crate::statically::StaticBuffer<T, N>> {
        type Item = T;
    }
);

#[cfg(feature = "alloc")]
cursor_impl!(
    impl [T: NoUninit, const N: usize] Cursor<crate::ringbuf::BufReader<T, N>> {
        type Item = T;
    }
);