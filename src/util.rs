use crate::AtomicBase;

#[cfg(not(feature = "loom"))]
#[inline]
pub(crate) const fn init_array<T: bytemuck::NoUninit, const N: usize>()
-> [crate::buffer::slots::Slot<T>; N] {
    use crate::buffer::slots::Slot;
    use core::mem::MaybeUninit;

    let mut arr: MaybeUninit<[Slot<T>; N]> = MaybeUninit::uninit();
    let ptr = arr.as_mut_ptr() as *mut Slot<T>;
    let mut i = 0;

    while i < N {
        unsafe {
            ptr.add(i).write(Slot::new());
        }
        i += 1;
    }

    unsafe { arr.assume_init() }
}

pub(crate) trait Storage {
    const SIZE: usize;
    const TOTAL_SIZE: usize = Self::SIZE * core::mem::size_of::<Self::Item>();

    type Item: Sized;

    fn cast(&self, offset: usize) -> &AtomicBase;
    fn is_aligned_atomic_base(&self) -> bool;
}
#[cfg(not(feature = "loom"))]
impl<T, const N: usize> Storage
    for core::cell::UnsafeCell<
        crate::buffer::slots::AtomicBaseAligned<[crate::buffer::slots::Slot<T>; N]>,
    >
{
    const SIZE: usize = N;
    type Item = T;

    #[inline(always)]
    fn cast(&self, offset: usize) -> &AtomicBase {
        let ptr = self.get() as *mut u8;
        let ptr = unsafe { ptr.add(offset) };
        unsafe { &*(ptr as *const AtomicBase) }
    }

    fn is_aligned_atomic_base(&self) -> bool {
        (self.get() as usize).is_multiple_of(crate::ATOMIC_ALIGN)
    }
}
#[cfg(feature = "loom")]
pub(crate) struct LoomStorage<T, const N: usize> {
    usizes: alloc::vec::Vec<loom::sync::atomic::AtomicUsize>,
    _marker: core::marker::PhantomData<[T; N]>,
}
#[cfg(feature = "loom")]
impl<T, const N: usize> LoomStorage<T, N> {
    pub(crate) fn new() -> Self {
        let mut usizes =
            alloc::vec::Vec::with_capacity(N * core::mem::size_of::<T>() / crate::ATOMIC_SIZE);
        for _ in 0..(N * core::mem::size_of::<T>() / crate::ATOMIC_SIZE) {
            usizes.push(loom::sync::atomic::AtomicUsize::new(0));
        }
        Self {
            usizes,
            _marker: core::marker::PhantomData,
        }
    }
}
#[cfg(feature = "loom")]
impl<T, const N: usize> Storage for LoomStorage<T, N> {
    const SIZE: usize = N;
    type Item = T;

    fn cast(&self, offset: usize) -> &AtomicBase {
        let index = offset / crate::ATOMIC_SIZE;
        &self.usizes[index]
    }

    fn is_aligned_atomic_base(&self) -> bool {
        true
    }
}
