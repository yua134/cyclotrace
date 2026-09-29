use bytemuck::NoUninit;

use crate::sink::SinkInner;
use crate::util::Storage;
use crate::{ATOMIC_ALIGN, ATOMIC_SIZE, AtomicBase, Base, Ordering};

pub(crate) struct PerByteLoad<'slot, 'return_buffer, Sink: ?Sized, St: Storage> {
    buf: &'slot St,
    index: usize,
    offset: isize,
    remaining: usize,
    count: usize,
    dst: *mut u8,
    _marker: core::marker::PhantomData<&'return_buffer mut Sink>,
}
impl<'slot, 'return_buffer, Sink: SinkInner + ?Sized, St: Storage>
    PerByteLoad<'slot, 'return_buffer, Sink, St>
where
    St::Item: NoUninit,
{
    /// Creates a new PerByteLoad instance.
    ///
    /// SAFETY:
    /// - buf must be aligned to the size of ATOMIC_ALIGN.
    /// - dst must be reserved for len * size_of::<T>() bytes.
    /// - The memory pointed to by buf and dst must not overlap.
    /// - The size of the buffer must be a multiple of the size of ATOMIC_SIZE.
    /// - The index must be less than the elements in the buffer.
    #[inline(always)]
    pub(crate) unsafe fn new(
        buf: &'slot St,
        index: usize,
        len: usize,
        dst: &'return_buffer mut Sink,
    ) -> Self {
        debug_assert!(len <= St::SIZE, "LEN must be less than or equal to N");
        debug_assert!(len > 0, "LEN must be greater than 0");
        debug_assert!(index < St::SIZE, "index must be less than N");
        debug_assert!(
            St::TOTAL_SIZE.is_multiple_of(ATOMIC_SIZE),
            "St::TOTAL_SIZE must be a multiple of the size of {}",
            ATOMIC_SIZE
        );
        debug_assert!(
            buf.is_aligned_atomic_base(),
            "buf must be aligned to the size of {}",
            ATOMIC_ALIGN
        );

        Self {
            buf,
            index,
            offset: Self::init_offset(index),
            remaining: core::mem::size_of::<St::Item>() * len,
            count: Self::count_regions(index, len),
            dst: dst.as_mut_ptr(),
            _marker: core::marker::PhantomData,
        }
    }

    #[inline(always)]
    const fn init_offset(index: usize) -> isize {
        let offset = index * core::mem::size_of::<St::Item>();
        -((offset % ATOMIC_ALIGN) as isize)
    }

    #[inline(always)]
    const fn count_regions(index: usize, len: usize) -> usize {
        let start = index * core::mem::size_of::<St::Item>();
        let end = start + core::mem::size_of::<St::Item>() * len - 1;
        (end / ATOMIC_SIZE) - (start / ATOMIC_SIZE) + 1
    }

    #[inline(always)]
    fn buf_ptr(&self) -> &AtomicBase {
        let offset = ((self.index * core::mem::size_of::<St::Item>())
            .wrapping_add_signed(self.offset))
            % (core::mem::size_of::<St::Item>() * St::SIZE);
        self.buf.cast(offset)
    }

    #[inline(always)]
    fn dst_write(&mut self, val: Base) {
        let skip = self.offset.min(0).unsigned_abs();
        let len = (ATOMIC_SIZE - skip).min(self.remaining);

        unsafe {
            let src = &val as *const Base as *const u8;
            let dst = self.dst.offset(self.offset.max(0));

            // SAFETY: dst is reserved for least len bytes, and src isn't overlapping with dst.
            core::ptr::copy_nonoverlapping(src.add(skip), dst, len);
        }
        self.offset += (skip + len) as isize;
        self.remaining -= len;
    }

    #[inline]
    pub(crate) fn load(&mut self) {
        for _ in 0..self.count {
            let ptr = self.buf_ptr();
            let val = ptr.load(Ordering::Relaxed);
            self.dst_write(val);
        }
    }
}

pub struct PerByteStore<'slot, St: Storage> {
    buf: &'slot St,
    index: usize,
    offset: isize,
    remaining: usize,
    src: St::Item,
}
impl<'slot, St: Storage> PerByteStore<'slot, St>
where
    St::Item: NoUninit,
{
    /// Creates a new PerByteStore instance.
    ///
    /// SAFETY:
    /// - buf must be aligned to the size of ATOMIC_ALIGN.
    /// - The size of the buffer must be a multiple of the size of ATOMIC_SIZE.
    /// - The index must be less than the elements in the buffer.
    #[inline(always)]
    pub(crate) unsafe fn new(buf: &'slot St, index: usize, src: St::Item) -> Self {
        debug_assert!(index < St::SIZE, "index must be less than N");
        debug_assert!(
            St::TOTAL_SIZE.is_multiple_of(ATOMIC_SIZE),
            "St::TOTAL_SIZE must be a multiple of the size of {}",
            ATOMIC_SIZE
        );
        debug_assert!(
            buf.is_aligned_atomic_base(),
            "buf must be aligned to the size of {}",
            ATOMIC_ALIGN
        );

        Self {
            buf,
            index,
            offset: Self::init_offset(index),
            remaining: core::mem::size_of::<St::Item>(),
            src,
        }
    }

    #[inline(always)]
    const fn init_offset(index: usize) -> isize {
        let offset = index * core::mem::size_of::<St::Item>();
        -((offset % ATOMIC_ALIGN) as isize)
    }

    #[inline(always)]
    const fn count_regions(index: usize) -> usize {
        let start = index * core::mem::size_of::<St::Item>();
        let end = start + core::mem::size_of::<St::Item>() - 1;
        (end / ATOMIC_SIZE) - (start / ATOMIC_SIZE) + 1
    }

    #[inline(always)]
    fn buf_ptr(&self) -> &AtomicBase {
        // It is the same as `self.buf.as_ptr() as *const u8 as usize`(this is safety), but this does not allowed in const fn, so we have to use transmute.
        let offset = ((self.index * core::mem::size_of::<St::Item>())
            .wrapping_add_signed(self.offset))
            % (St::TOTAL_SIZE);
        self.buf.cast(offset)
    }

    #[inline(always)]
    fn buf_store(&mut self, val: Base) {
        let skip = self.offset.min(0).unsigned_abs();

        let slot = self.buf_ptr();
        let before = slot.load(Ordering::Relaxed);

        let len = self.remaining.min(ATOMIC_SIZE - skip);
        let bits = len * 8;

        let mask = (Base::MAX >> (ATOMIC_SIZE * 8 - bits)) << (skip * 8);

        let after = (before & !mask) | ((val << (skip * 8)) & mask);

        slot.store(after, Ordering::Relaxed);

        self.remaining -= len;
        self.offset += (len + skip) as isize;
    }

    #[inline(always)]
    fn buf_full_store(&mut self, val: Base) {
        let slot = self.buf_ptr();
        slot.store(val, Ordering::Relaxed);

        self.remaining -= ATOMIC_SIZE;
        self.offset += ATOMIC_SIZE as isize;
    }

    #[inline(always)]
    fn src_byte(&self) -> Base {
        let from = self.offset.max(0) as usize;

        let mut buf = unsafe { core::mem::zeroed() };

        let len = (core::mem::size_of::<St::Item>() - from).min(ATOMIC_SIZE);

        unsafe {
            core::ptr::copy_nonoverlapping(
                (&self.src as *const St::Item as *const u8).add(from),
                &mut buf as *mut Base as *mut u8,
                len,
            );
        }

        buf
    }

    #[inline(always)]
    /// SAFETY:
    /// - This function must be called only one thread.
    pub(crate) unsafe fn store(&mut self) {
        if is_always_multiple_of_atomic_word::<St::Item>() {
            let count = Self::count_regions(self.index);
            for _ in 0..count {
                let bytes = self.src_byte();
                self.buf_full_store(bytes);
            }
        } else if is_always_one_atomic_word::<St::Item>() {
            let bytes = self.src_byte();
            self.buf_store(bytes);
        } else if core::mem::size_of::<St::Item>() < ATOMIC_SIZE {
            let count = Self::count_regions(self.index);

            match count {
                1 => {
                    let bytes = self.src_byte();
                    self.buf_store(bytes);
                }
                2 => {
                    let bytes = self.src_byte();
                    self.buf_store(bytes);

                    let bytes = self.src_byte();
                    self.buf_store(bytes);
                }
                _ => unsafe { core::hint::unreachable_unchecked() },
            }
        } else {
            let count = Self::count_regions(self.index);

            // This is the first region, which may be unaligned, so we have to use buf_store() to store it.
            let bytes = self.src_byte();
            self.buf_store(bytes);

            // The count is at least 2, because the size of St::Item is larger than ATOMIC_SIZE.
            for _ in 0..(count - 2) {
                let bytes = self.src_byte();
                self.buf_full_store(bytes);
            }

            // This is the last region, which may be unaligned, so we have to use buf_store() to store it.
            let bytes = self.src_byte();
            self.buf_store(bytes);
        }
    }
}

#[inline(always)]
const fn is_always_one_atomic_word<T>() -> bool {
    let size = core::mem::size_of::<T>();

    size <= ATOMIC_SIZE && size.is_power_of_two()
}

#[inline(always)]
const fn is_always_multiple_of_atomic_word<T>() -> bool {
    let size = core::mem::size_of::<T>();

    size >= ATOMIC_SIZE && size.is_multiple_of(ATOMIC_SIZE)
}
