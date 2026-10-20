use core::{alloc::Allocator, ops::Deref, ptr::NonNull};
use crate::{AtomicUsize, Ordering, fence};

pub(crate) struct Arc<T: ?Sized, A: Allocator> {
    inner: NonNull<ArcInner<T>>,
    alloc: A,
}
impl<T: ?Sized> Arc<T, alloc::alloc::Global> {
    #[cfg(feature = "alloc")]
    pub(crate) fn new(data: T) -> Self
    where
        T: Sized,
    {
        Self::new_in(data, alloc::alloc::Global)
    }
}

impl<T: ?Sized, A: Allocator> Arc<T, A> {
    /// Creates a new `Arc` with the given data and allocator.
    pub(crate) fn new_in(data: T, alloc: A) -> Self
    where
        T: Sized,
    {
        let inner = ArcInner {
            data,
            rc: AtomicUsize::new(1),
        };
        let layout = core::alloc::Layout::new::<ArcInner<T>>();
        let ptr = alloc.allocate(layout).unwrap().cast();
        unsafe { ptr.write(inner) };

        Self {
            inner: ptr,
            alloc,
        }
    }
}

impl<T: ?Sized, A: Allocator> Deref for Arc<T, A> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &self.inner.as_ref().data }
    }
}

impl<T: ?Sized, A: Allocator> Drop for Arc<T, A> {
    fn drop(&mut self) {
        let inner = unsafe { self.inner.as_ref() };
        if inner.rc.fetch_sub(1, Ordering::Release) == 1 {
            fence(Ordering::Acquire);
            let layout = core::alloc::Layout::for_value(inner);
            unsafe {
                core::ptr::drop_in_place(self.inner.as_ptr());
                self.alloc.deallocate(self.inner.cast(), layout);
            }
        }
    }
}

impl<T: ?Sized, A: Allocator + Clone> Clone for Arc<T, A> {
    /// It will cause UB if rc is wrapped around and unwind is enabled, but it is very unlikely to happen in practice.
    /// We have no way to abort the process in a portable way, so we just let it UB probably.
    fn clone(&self) -> Self {
        let inner = unsafe { self.inner.as_ref() };
        let old = inner.rc.fetch_add(1, Ordering::Relaxed);

        if old == usize::MAX {
            panic!("Arc reference count overflow");
        }

        Self {
            inner: self.inner,
            alloc: self.alloc.clone(),
        }
    }
}

unsafe impl<T: ?Sized + Send + Sync, A: Allocator + Send + Sync> Send for Arc<T, A> {}
unsafe impl<T: ?Sized + Send + Sync, A: Allocator + Send + Sync> Sync for Arc<T, A> {}

struct ArcInner<T: ?Sized> {
    rc: AtomicUsize,
    data: T,
}