#![no_std]
#![doc = include_str!("../README.md")]

#[cfg(feature = "alloc")]
extern crate alloc;

cfg_if::cfg_if!(
    if #[cfg(feature = "loom")] {
        pub(crate) use loom::sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering, fence}};
        pub(crate) use loom::hint::spin_loop;

        pub(crate) type AtomicBase = loom::sync::atomic::AtomicUsize;
        pub(crate) type Base = usize;
    } else if #[cfg(feature = "portable-atomic")] {
        pub(crate) use portable_atomic::{AtomicBool, AtomicU64, Ordering, fence};
        pub(crate) use portable_atomic::hint::spin_loop;

        const U128_LOCK_FREE: bool = portable_atomic::AtomicU128::is_always_lock_free();
        const U64_LOCK_FREE: bool = portable_atomic::AtomicU64::is_always_lock_free();
        const U32_LOCK_FREE: bool = portable_atomic::AtomicU32::is_always_lock_free();
        const U16_LOCK_FREE: bool = portable_atomic::AtomicU16::is_always_lock_free();

        pub(crate) type AtomicBase = condtype::CondType<
            U128_LOCK_FREE,
            portable_atomic::AtomicU128,
            condtype::CondType<
                U64_LOCK_FREE,
                portable_atomic::AtomicU64,
                condtype::CondType<
                    U32_LOCK_FREE,
                    portable_atomic::AtomicU32,
                    condtype::CondType<
                        U16_LOCK_FREE,
                        portable_atomic::AtomicU16,
                        portable_atomic::AtomicU8,
                    >,
                >,
            >,
        >;
        pub(crate) type Base = condtype::CondType<
            U128_LOCK_FREE,
            u128,
            condtype::CondType<
                U64_LOCK_FREE,
                u64,
                condtype::CondType<
                    U32_LOCK_FREE,
                    u32,
                    condtype::CondType<
                        U16_LOCK_FREE,
                        u16,
                        u8,
                    >,
                >,
            >,
        >;

        #[cfg(feature = "alloc")]
        pub(crate) use alloc::sync::Arc;
    } else {
        pub(crate) use core::sync::atomic::{AtomicBool, AtomicU64, Ordering, fence};
        pub(crate) use core::hint::spin_loop;

        cfg_if::cfg_if!(
            if #[cfg(target_has_atomic = "64")] {
                pub(crate) type AtomicBase = core::sync::atomic::AtomicU64;
                pub(crate) type Base = u64;
            } else if #[cfg(target_has_atomic = "32")] {
                pub(crate) type AtomicBase = core::sync::atomic::AtomicU32;
                pub(crate) type Base = u32;
            } else if #[cfg(target_has_atomic = "16")] {
                pub(crate) type AtomicBase = core::sync::atomic::AtomicU16;
                pub(crate) type Base = u16;
            } else if #[cfg(target_has_atomic = "8")] {
                pub(crate) type AtomicBase = core::sync::atomic::AtomicU8;
                pub(crate) type Base = u8;
            } else {
                compile_error!("This target does not support atomic operations.");
            }
        );

        #[cfg(feature = "alloc")]
        pub(crate) use alloc::sync::Arc;
    }
);

/// This is a constant that represents the size of the atomic type used in this crate.
/// [`crate::create_buffer`], [`crate::StaticBuffer::new`] will panic if the size of the buffer is not a multiple of this value.
pub const ATOMIC_SIZE: usize = core::mem::size_of::<AtomicBase>();
pub(crate) const ATOMIC_ALIGN: usize = core::mem::align_of::<AtomicBase>();

#[cfg(feature = "alloc")]
mod ringbuf;

mod buffer;
mod per_byte_atomic;
mod sink;
mod statically;
mod util;

#[cfg(feature = "alloc")]
pub use ringbuf::{BufReader, BufWriter, create_buffer};

pub use {
    sink::Sink,
    statically::{StaticBuffer, Writer},
};
