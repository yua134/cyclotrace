use core::mem::MaybeUninit;

use crossbeam_utils::CachePadded;

use crate::buffer::Slot;

#[cfg_attr(feature = "loom", maybe_const::maybe_const)]
#[inline]
pub(crate) const fn init_array<T: Copy, const N: usize>() -> [CachePadded<Slot<T>>; N] {
    let mut arr: MaybeUninit<[CachePadded<Slot<T>>; N]> = MaybeUninit::uninit();
    let ptr = arr.as_mut_ptr() as *mut CachePadded<Slot<T>>;
    let mut i = 0;

    while i < N {
        unsafe {
            ptr.add(i).write(CachePadded::new(Slot::new()));
        }
        i += 1;
    }

    unsafe { arr.assume_init() }
    }
