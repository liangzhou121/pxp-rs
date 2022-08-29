use crate::buddy_alloc::BuddyAllocatorManager;
use alloc::alloc::Layout;
use alloc::string::String;
use cfg_if;
use core::ffi::c_void;
//use ctor::dtor;
use sgx_types::sgx_status_t;
cfg_if::cfg_if! {
    if #[cfg(feature = "occlum")] {
        use sgx_trts;
    }
}

pub const PTR_NULL: u64 = 0;
static MANAGER: BuddyAllocatorManager = BuddyAllocatorManager::new();

fn untrusted_mem_alloc(size: usize) -> Result<(), String> {
    let chunk = if size <= 1024 {
        1024
    } else {
        size.next_power_of_two()
    }
    .checked_mul(2)
    .unwrap();

    let layout = Layout::from_size_align(chunk, 1)
        .unwrap()
        .align_to(core::mem::size_of::<*const c_void>())
        .unwrap();

    let mut mem_ptr: *mut c_void = core::ptr::null_mut();
    cfg_if::cfg_if! {
        if #[cfg(feature = "occlum")] {
            info!(
                "pxp-rs:v1: allocate untrusted memory: [ 0x{:x} ]",
                &chunk
            );
            let sgx_status = unsafe {
                occlum_ocall_posix_memalign(&mut mem_ptr as *mut _, layout.align(), layout.size())
            };
            assert!(sgx_status == sgx_status_t::SGX_SUCCESS);
            assert!(sgx_trts::trts::rsgx_raw_is_outside_enclave(
                mem_ptr as *const u8,
                layout.size()
            ));
        } else {
            let sgx_status = unsafe {
                u_malloc(&mut mem_ptr as *mut _, layout.size())
            };
            assert!(sgx_status == sgx_status_t::SGX_SUCCESS);
            assert!(unsafe { sgx_is_outside_enclave(mem_ptr, layout.size()) } != 0);
        }
    }
    unsafe {
        MANAGER.init(mem_ptr as usize, chunk, 16);
    }
    Ok(())
}

// This provides module teardown function attribute similar with `__attribute__((destructor))` in C/C++ and will
// be called after the main function. Static variables are still safe to visit at this time.
// According to: https://github.com/mmastrac/rust-ctor The [dtor] is work as expected in both `bin` and `cdylib` outputs.
/*#[dtor]
fn untrusted_mem_free() {
    for range in MANAGER.fetch_memory_ranges().unwrap() {
        info!("Free untrusted memory: {:x}", range);
        cfg_if::cfg_if! {
            if #[cfg(feature = "occlum")] {
                let sgx_status = unsafe { occlum_ocall_free(range as *mut c_void) };
                assert!(sgx_status == sgx_status_t::SGX_SUCCESS);
            } else {
                let sgx_status = unsafe { u_free(range as *mut c_void) };
                assert!(sgx_status == sgx_status_t::SGX_SUCCESS);
            }
        }
    }
}*/

pub fn alloc(size: usize) -> Result<*mut u8, String> {
    let ptr = if size > 0 {
        //info!("alloc: size:{:?}", size);
        let layout = Layout::from_size_align(size, 1).unwrap();
        let ptr = MANAGER.alloc(layout);
        match ptr {
            Ok(ptr) => ptr.as_ptr() as *mut u8,
            Err(_) => {
                untrusted_mem_alloc(size.clone()).unwrap();
                MANAGER.alloc(layout).unwrap().as_ptr() as *mut u8
            }
        }
    } else {
        PTR_NULL as *mut u8
    };
    Ok(ptr)
}

pub fn free(ptr: *mut u8, size: usize) -> Result<(), String> {
    if ptr as u64 != PTR_NULL {
        //info!("free: size:{:?}", size);
        let layout = Layout::from_size_align(size, 1).unwrap();
        MANAGER.dealloc(core::ptr::NonNull::new(ptr).unwrap(), layout);
    }
    Ok(())
}

// External functions
cfg_if::cfg_if! {
    if #[cfg(feature = "occlum")] {
        extern "C" {
            fn occlum_ocall_posix_memalign(
                ptr: *mut *mut c_void,
                align: usize, // must be power of two and a multiple of sizeof(void*)
                size: usize,
            ) -> sgx_status_t;
            //fn occlum_ocall_free(ptr: *mut c_void) -> sgx_status_t;
        }
    } else {
        extern "C" {
            fn u_malloc(ptr: *mut *mut c_void, size: usize) -> sgx_status_t;
            fn u_free(ptr: *mut c_void) -> sgx_status_t;
            fn sgx_is_outside_enclave(ptr: *mut c_void, size: usize) -> i32;
        }
    }
}
