#![no_std]
#![feature(lang_items)]
#![allow(non_camel_case_types)]
#![feature(alloc_error_handler)]

#[macro_use]
extern crate alloc;
#[macro_use]
extern crate log;

mod buddy_alloc;
mod i915;
mod memory;
cfg_if::cfg_if! {
    if #[cfg(not(feature = "occlum"))] {
        mod sgx_no_std;
    }
}

pub use i915::pxp_ioctl;
