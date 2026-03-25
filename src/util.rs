use core::mem::MaybeUninit;

use crate::buffer::Slot;

cfg_if::cfg_if!(
if #[cfg(feature = "loom")] {
    pub(crate) fn init_array<T: Copy, const N: usize>() -> [Slot<T>; N] {
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
} else {
    #[inline]
    pub(crate) const fn init_array<T: Copy, const N: usize>() -> [Slot<T>; N] {
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
}
);
