#![no_std]
#![doc = include_str!("../README.md")]

#[cfg(feature = "alloc")]
extern crate alloc;

cfg_if::cfg_if!(
    if #[cfg(feature = "loom")] {
        pub(crate) use loom::sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}};
        pub(crate) use loom::hint::spin_loop;
    } else {
        pub(crate) use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
        pub(crate) use core::hint::spin_loop;

        #[cfg(feature = "alloc")]
        pub(crate) use alloc::sync::Arc;
    }
);

#[cfg(feature = "alloc")]
mod ringbuf;

mod buffer;
mod sink;
mod statically;
mod util;

#[cfg(feature = "alloc")]
pub use ringbuf::{BufReader, BufWriter, create_buffer};

pub use {
    sink::Sink,
    statically::{StaticBuffer, Writer},
};
