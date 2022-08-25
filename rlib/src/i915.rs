use crate::memory::{alloc, free};
use alloc::borrow::ToOwned;
use alloc::string::String;
use core::{mem, ptr};
use sgx_types::sgx_status_t;

const PRELIM_I915_USER_EXT_MASK: u32 = 0xffff;
const I915_CONTEXT_PARAM_ENGINES: u64 = 0xa;

const DRM_IOCTL_GEM_CLOSE: u32 = 1074291721;
const PRELIM_DRM_IOCTL_I915_PXP_OPS: u32 = 3222299794;
const DRM_IOCTL_I915_GEM_CREATE_EXT: u32 = 3222824027;
const DRM_IOCTL_I915_QUERY: u32 = 3222299769;
const DRM_IOCTL_I915_GEM_CONTEXT_GETPARAM: u32 = 3222824052;
const DRM_IOCTL_I915_GEM_CONTEXT_SETPARAM: u32 = 3222824053;
const DRM_IOCTL_GET_MAGIC: u32 = 2147771394;
const DRM_IOCTL_AUTH_MAGIC: u32 = 1074029585;
const DRM_IOCTL_VERSION: u32 = 3225445376;
const DRM_IOCTL_I915_GETPARAM: u32 = 3222299718;
const DRM_IOCTL_I915_GEM_CONTEXT_CREATE_EXT: u32 = 3222299757;
const DRM_IOCTL_I915_GEM_VM_CREATE: u32 = 3222299770;
const DRM_IOCTL_I915_GEM_VM_DESTROY: u32 = 1074816123;
const DRM_IOCTL_I915_GEM_MMAP_OFFSET: u32 = 3223348324;
const DRM_IOCTL_I915_GET_RESET_STATS: u32 = 3222824050;
const DRM_IOCTL_I915_GEM_GET_APERTURE: u32 = 2148557923;
const DRM_IOCTL_I915_GEM_SET_DOMAIN: u32 = 1074553951;
const DRM_IOCTL_I915_GEM_EXECBUFFER2_WR: u32 = 3225445481;
const DRM_IOCTL_I915_GEM_EXECBUFFER2: u32 = 1077961833;
const DRM_IOCTL_I915_GEM_USERPTR: u32 = 3222824051;
const DRM_IOCTL_I915_GEM_GET_TILING: u32 = 3222299746;
const DRM_IOCTL_I915_GEM_WAIT: u32 = 3222299756;
const DRM_IOCTL_I915_GEM_CONTEXT_DESTROY: u32 = 1074291822;
const DRM_IOCTL_I915_REG_READ: u32 = 3222299761;
const DRM_IOCTL_I915_GEM_BUSY: u32 = 3221775447;
const DRM_IOCTL_PRIME_HANDLE_TO_FD: u32 = 3222037549;
const DRM_IOCTL_PRIME_FD_TO_HANDLE: u32 = 3222037550;
const DRM_IOCTL_I915_GET_PIPE_FROM_CRTC_ID: u32 = 3221775461;
const DRM_IOCTL_I915_GEM_SW_FINISH: u32 = 1074029664;
const DRM_IOCTL_I915_GEM_MADVISE: u32 = 3222037606;
const DRM_IOCTL_I915_GEM_PREAD: u32 = 1075864668;
const DRM_IOCTL_I915_GEM_PWRITE: u32 = 1075864669;
const DRM_IOCTL_I915_GEM_MMAP: u32 = 3223872606;

trait DeepCopy<T> {
    fn alloc(&mut self, source: &T) -> Result<(), String>;
    fn copy(&mut self, source: &T) -> Result<(), String>;
    fn free(&mut self) -> Result<(), String>;
}

fn iterator<F>(mut src: u64, mut dst: &mut u64, f: F) -> Result<(), String>
where
    F: Fn(&mut u64, u64) -> Result<(), String>,
{
    while src != 0 {
        f(dst, src).unwrap();

        let ext_dst = unsafe { &mut *(*dst as *mut i915_user_extension) };
        let ext_src = unsafe { &*(src as *mut i915_user_extension) };
        dst = &mut ext_dst.next_extension;
        src = ext_src.next_extension;
    }
    Ok(())
}

fn ioctl(fd: i32, cmd: &u32, arg: *const u8) -> i32 {
    let mut ret: i32 = 0;
    let mut status = sgx_status_t::SGX_ERROR_UNEXPECTED;
    unsafe {
        cfg_if::cfg_if! {
            if #[cfg(feature = "occlum")] {
                status = occlum_ocall_device_ioctl(
                    &mut ret as *mut i32,
                    fd,
                    cmd.to_owned() as i32,
                    arg as u64,
                );
            } else {
                status = ocall_pxp_ioctl(
                    &mut ret as *mut i32,
                    fd,
                    cmd.to_owned() as i32,
                    arg as u64,
                );
            }
        }
    }
    assert!(status == sgx_status_t::SGX_SUCCESS);
    ret
}

fn exec<T>(fd: i32, cmd: &u32, arg: *const u8) -> Result<i32, String> {
    let ptr_t = arg as *mut T;
    let ptr_u = alloc(mem::size_of::<T>())?;
    unsafe {
        ptr::copy(ptr_t as *const u8, ptr_u as *mut u8, mem::size_of::<T>());
    }
    let ret = ioctl(fd, cmd, ptr_u);
    unsafe {
        ptr::copy(ptr_u as *const u8, ptr_t as *mut u8, mem::size_of::<T>());
    }
    free(ptr_u, mem::size_of::<T>())?;

    Ok(ret)
}

fn exec2<T: DeepCopy<T>>(fd: i32, cmd: &u32, arg: *const u8) -> Result<i32, String> {
    let ptr_t = arg as *mut T;
    let ptr_u = alloc(mem::size_of::<T>())?;
    unsafe {
        ptr::copy(ptr_t as *const u8, ptr_u as *mut u8, mem::size_of::<T>());
    }
    let arg_t = unsafe { &mut *(ptr_t as *mut T) };
    let arg_u = unsafe { &mut *(ptr_u as *mut T) };
    arg_u.alloc(arg_t)?;
    let ret = ioctl(fd, cmd, ptr_u);
    arg_t.copy(arg_u)?;
    arg_u.free()?;
    free(ptr_u, mem::size_of::<T>())?;
    Ok(ret)
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct prelim_drm_i915_gem_memory_class_instance {
    memory_class: u16,
    memory_instance: u16,
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct prelim_drm_i915_gem_object_param {
    handle: u32,
    size: u32,
    param: u64,
    data: u64,
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct i915_user_extension {
    next_extension: u64,
    name: u32,
    flags: u32,
    rsvd: [u32; 4],
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct prelim_drm_i915_gem_create_ext_setparam {
    base: i915_user_extension,
    param: prelim_drm_i915_gem_object_param,
}
#[repr(C)]
#[allow(non_camel_case_types)]
struct prelim_drm_i915_gem_create_ext_vm_private {
    base: i915_user_extension,
    vm_id: u32,
}
#[repr(C)]
#[allow(non_camel_case_types)]
struct prelim_drm_i915_gem_create_ext_protected_content {
    base: i915_user_extension,
    flags: u32,
}
#[repr(C)]
#[allow(non_camel_case_types)]
struct prelim_drm_i915_gem_create_ext {
    size: u64,
    handle: u32,
    pad: u32,
    extensions: u64,
}
impl prelim_drm_i915_gem_create_ext {
    fn sizeof(name: u32) -> Result<usize, String> {
        match name {
            1 => Ok(mem::size_of::<prelim_drm_i915_gem_create_ext_setparam>()),
            2 => Ok(mem::size_of::<prelim_drm_i915_gem_create_ext_vm_private>()),
            3 => Ok(mem::size_of::<
                prelim_drm_i915_gem_create_ext_protected_content,
            >()),
            _ => Err(format!("the name:{:?} is illegal !!!", name)),
        }
    }
}
impl DeepCopy<prelim_drm_i915_gem_create_ext> for prelim_drm_i915_gem_create_ext {
    fn alloc(&mut self, source: &prelim_drm_i915_gem_create_ext) -> Result<(), String> {
        let mut ext_src = source.extensions;
        let mut ext_dst = &mut self.extensions;
        while ext_src != 0 {
            let base_src = unsafe { &mut *(ext_src as *mut i915_user_extension) };
            let size = Self::sizeof(base_src.name & PRELIM_I915_USER_EXT_MASK)?;
            *ext_dst = alloc(size as usize)? as u64;
            unsafe {
                ptr::copy(ext_src as *const u8, *ext_dst as *mut u8, size);
            }
            if base_src.name & PRELIM_I915_USER_EXT_MASK == 1 {
                let src = unsafe { &*(ext_src as *mut prelim_drm_i915_gem_create_ext_setparam) };
                let dst =
                    unsafe { &mut *(*ext_dst as *mut prelim_drm_i915_gem_create_ext_setparam) };
                let size = mem::size_of::<prelim_drm_i915_gem_memory_class_instance>()
                    .checked_mul(src.param.size as usize)
                    .ok_or(format!("mul error"))?;
                dst.param.data = alloc(size as usize)? as u64;
                unsafe {
                    ptr::copy(src.param.data as *const u8, dst.param.data as *mut u8, size);
                }
            }

            ext_src = base_src.next_extension;
            let base_dst = unsafe { &mut *(*ext_dst as *mut i915_user_extension) };
            ext_dst = &mut base_dst.next_extension;
        }
        Ok(())
    }
    fn copy(&mut self, source: &prelim_drm_i915_gem_create_ext) -> Result<(), String> {
        self.handle = source.handle;
        self.size = source.size;
        self.pad = source.pad;
        Ok(())
    }
    fn free(&mut self) -> Result<(), String> {
        let mut ext = self.extensions;
        while ext != 0 {
            let base = unsafe { &mut *(ext as *mut i915_user_extension) };
            if base.name & PRELIM_I915_USER_EXT_MASK == 1 {
                let param = unsafe { &mut *(ext as *mut prelim_drm_i915_gem_create_ext_setparam) };
                let size = mem::size_of::<prelim_drm_i915_gem_memory_class_instance>()
                    .checked_mul(param.param.size as usize)
                    .ok_or(format!("mul error"))?;
                free(param.param.data as *mut u8, size)?;
            }
            let next_ext = base.next_extension;
            free(
                ext as *mut u8,
                Self::sizeof(base.name & PRELIM_I915_USER_EXT_MASK)?,
            )?;
            ext = next_ext;
        }
        Ok(())
    }
}

#[repr(C)]
#[repr(packed)]
#[allow(non_camel_case_types)]
struct prelim_drm_i915_pxp_set_session_status_params {
    pxp_tag: u32,
    session_type: u32,
    session_mode: u32,
    req_session_state: u32,
}
#[repr(C)]
#[repr(packed)]
#[allow(non_camel_case_types)]
struct prelim_drm_i915_pxp_tee_io_message_params {
    msg_in: u64,
    msg_in_size: u32,
    msg_out: u64,
    msg_out_buf_size: u32,
    msg_out_ret_size: u32,
}
#[repr(C)]
#[repr(packed)]
#[allow(non_camel_case_types)]
struct prelim_drm_i915_pxp_query_tag {
    session_is_alive: u32,
    pxp_tag: u32,
}
#[repr(C)]
#[repr(packed)]
#[allow(non_camel_case_types)]
struct prelim_drm_i915_pxp_ops {
    action: u32,
    status: u32,
    params: u64,
}
impl prelim_drm_i915_pxp_ops {
    fn sizeof(&self) -> Result<usize, String> {
        match self.action {
            0 => Ok(mem::size_of::<prelim_drm_i915_pxp_set_session_status_params>()),
            1 => Ok(mem::size_of::<prelim_drm_i915_pxp_tee_io_message_params>()),
            2 => Ok(mem::size_of::<prelim_drm_i915_pxp_query_tag>()),
            _ => Err(format!("the action is illegal !!!")),
        }
    }
}
impl DeepCopy<prelim_drm_i915_pxp_ops> for prelim_drm_i915_pxp_ops {
    fn alloc(&mut self, source: &prelim_drm_i915_pxp_ops) -> Result<(), String> {
        let size = self.sizeof()?;
        self.params = alloc(size)? as u64;
        unsafe {
            ptr::copy(source.params as *const u8, self.params as *mut u8, size);
        }
        if self.action == 1 {
            let src =
                unsafe { &*(source.params as *mut prelim_drm_i915_pxp_tee_io_message_params) };
            let dst =
                unsafe { &mut *(self.params as *mut prelim_drm_i915_pxp_tee_io_message_params) };
            if src.msg_in_size > 0 {
                dst.msg_in = alloc(src.msg_in_size as usize)? as u64;
                unsafe {
                    ptr::copy(
                        src.msg_in as *const u8,
                        dst.msg_in as *mut u8,
                        src.msg_in_size as usize,
                    );
                }
            }
            if src.msg_out_buf_size > 0 {
                dst.msg_out = alloc(src.msg_out_buf_size as usize)? as u64;
                unsafe {
                    ptr::copy(
                        src.msg_out as *const u8,
                        dst.msg_out as *mut u8,
                        src.msg_out_buf_size as usize,
                    );
                }
            }
        }
        Ok(())
    }
    fn copy(&mut self, source: &prelim_drm_i915_pxp_ops) -> Result<(), String> {
        match self.action {
            0 | 2 => unsafe {
                ptr::copy(
                    source.params as *const u8,
                    self.params as *mut u8,
                    self.sizeof()?,
                );
            },
            1 => {
                let src =
                    unsafe { &*(source.params as *mut prelim_drm_i915_pxp_tee_io_message_params) };
                let dst = unsafe {
                    &mut *(self.params as *mut prelim_drm_i915_pxp_tee_io_message_params)
                };
                if src.msg_out_ret_size > 0 {
                    unsafe {
                        ptr::copy(
                            src.msg_out as *const u8,
                            dst.msg_out as *mut u8,
                            src.msg_out_ret_size as usize,
                        );
                    }
                }
                dst.msg_out_ret_size = src.msg_out_ret_size;
            }
            _ => error!("the action is illegal, so do nonthing"),
        }
        Ok(())
    }
    fn free(&mut self) -> Result<(), String> {
        if self.action == 1 {
            let param =
                unsafe { &*(self.params as *mut prelim_drm_i915_pxp_tee_io_message_params) };
            if param.msg_in_size > 0 {
                free(param.msg_in as *mut u8, param.msg_in_size as usize)?;
            }
            if param.msg_out_buf_size > 0 {
                free(param.msg_out as *mut u8, param.msg_out_buf_size as usize)?;
            }
        }
        free(self.params as *mut u8, self.sizeof()?)?;
        Ok(())
    }
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct drm_i915_query_item {
    query_id: u64,
    length: i32,
    flags: u32,
    data_ptr: u64,
}
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct drm_i915_query {
    num_items: u32,
    flags: u32,
    items_ptr: u64,
}
impl drm_i915_query {
    fn items_size(&self) -> Result<usize, String> {
        mem::size_of::<drm_i915_query_item>()
            .checked_mul(self.num_items as usize)
            .ok_or(format!("mul error"))
    }
    fn iterator(
        &mut self,
        source: &drm_i915_query,
        f: fn(&mut drm_i915_query_item, &drm_i915_query_item) -> Result<(), String>,
    ) -> Result<(), String> {
        let offset = mem::size_of::<drm_i915_query_item>();
        for i in 0..self.num_items {
            let d = unsafe {
                &mut *((self.items_ptr as *mut u8)
                    .add(offset.checked_mul(i as usize).ok_or(format!("mul error"))?)
                    as *mut drm_i915_query_item)
            };
            let s = unsafe {
                &*((source.items_ptr as *mut u8)
                    .add(offset.checked_mul(i as usize).ok_or(format!("mul error"))?)
                    as *mut drm_i915_query_item)
            };
            f(d, s)?;
        }
        Ok(())
    }
}
impl DeepCopy<drm_i915_query> for drm_i915_query {
    fn alloc(&mut self, source: &drm_i915_query) -> Result<(), String> {
        let size = self.items_size()?;
        if size == 0 {
            return Ok(());
        }
        self.items_ptr = alloc(size)? as u64;
        unsafe {
            ptr::copy(
                source.items_ptr as *const u8,
                self.items_ptr as *mut u8,
                size,
            );
        }
        self.iterator(
            source,
            |dst: &mut drm_i915_query_item, src: &drm_i915_query_item| {
                dst.query_id = src.query_id;
                dst.length = src.length;
                dst.flags = src.flags;
                if src.length > 0 {
                    dst.data_ptr = alloc(src.length as usize)? as u64;
                    unsafe {
                        ptr::copy(
                            src.data_ptr as *const u8,
                            dst.data_ptr as *mut u8,
                            src.length as usize,
                        );
                    }
                } else {
                    dst.data_ptr = crate::memory::PTR_NULL;
                }
                Ok(())
            },
        )?;
        Ok(())
    }
    fn copy(&mut self, source: &drm_i915_query) -> Result<(), String> {
        self.iterator(
            source,
            |dst: &mut drm_i915_query_item, src: &drm_i915_query_item| {
                if dst.length == 0 {
                    dst.length = src.length;
                } else {
                    unsafe {
                        ptr::copy(
                            src.data_ptr as *const u8,
                            dst.data_ptr as *mut u8,
                            dst.length as usize,
                        );
                    }
                }
                Ok(())
            },
        )?;
        Ok(())
    }
    fn free(&mut self) -> Result<(), String> {
        let size = self.items_size()?;
        if size == 0 {
            return Ok(());
        }
        for i in 0..self.num_items {
            let item = unsafe {
                &mut *((self.items_ptr as *mut u8).add(
                    mem::size_of::<drm_i915_query_item>()
                        .checked_mul(i as usize)
                        .ok_or(format!("mul error"))?,
                ) as *mut drm_i915_query_item)
            };
            free(item.data_ptr as *mut u8, item.length as usize)?;
        }
        free(self.items_ptr as *mut u8, size)?;
        Ok(())
    }
}

#[repr(C)]
#[allow(non_camel_case_types)]
pub struct drm_i915_gem_context_param {
    ctx_id: u32,
    size: u32,
    param: u64,
    value: u64,
}
impl drm_i915_gem_context_param {
    fn alloc(
        &mut self,
        source: &drm_i915_gem_context_param,
        f: fn(src: u64, dst: u64) -> Result<(), String>,
    ) -> Result<(), String> {
        if self.size > 0 {
            self.value = alloc(self.size as usize)? as u64;
            unsafe {
                ptr::copy(
                    source.value as *const u8,
                    self.value as *mut u8,
                    self.size as usize,
                );
            }
        }
        f(self.value, source.value)
    }
    fn free(&mut self, f: fn(dst: u64) -> Result<(), String>) -> Result<(), String> {
        f(self.value)?;
        if self.size > 0 {
            free(self.value as *mut u8, self.size as usize)?;
        }
        Ok(())
    }
}
impl DeepCopy<drm_i915_gem_context_param> for drm_i915_gem_context_param {
    fn alloc(&mut self, source: &drm_i915_gem_context_param) -> Result<(), String> {
        self.alloc(source, |_src, _dst| -> Result<(), String> { Ok(()) })
    }
    fn copy(&mut self, source: &drm_i915_gem_context_param) -> Result<(), String> {
        //arg_t.value = arg_u.value;
        match self.size {
            0 => self.value = source.value,
            _ => unsafe {
                ptr::copy(
                    source.value as *const u8,
                    self.value as *mut u8,
                    self.size as usize,
                );
            },
        };
        Ok(())
    }
    fn free(&mut self) -> Result<(), String> {
        self.free(|_dst| -> Result<(), String> { Ok(()) })
    }
}

#[repr(C)]
#[allow(non_camel_case_types)]
pub struct drm_version {
    version_major: i32,
    version_minor: i32,
    version_patchlevel: i32,
    name_len: u64,
    name: *const u8,
    date_len: u64,
    date: *const u8,
    desc_len: u64,
    desc: *const u8,
}
impl DeepCopy<drm_version> for drm_version {
    fn alloc(&mut self, _: &drm_version) -> Result<(), String> {
        self.name = alloc(self.name_len as usize)?;
        self.date = alloc(self.date_len as usize)?;
        self.desc = alloc(self.desc_len as usize)?;
        Ok(())
    }
    fn copy(&mut self, source: &drm_version) -> Result<(), String> {
        if self.name_len == 0 {
            self.name_len = source.name_len;
        } else {
            unsafe {
                ptr::copy(
                    source.name as *const u8,
                    self.name as *mut u8,
                    self.name_len as usize,
                );
            }
        }
        if self.date_len == 0 {
            self.date_len = source.date_len;
        } else {
            unsafe {
                ptr::copy(
                    source.date as *const u8,
                    self.date as *mut u8,
                    self.date_len as usize,
                );
            }
        }
        if self.desc_len == 0 {
            self.desc_len = source.desc_len;
        } else {
            unsafe {
                ptr::copy(
                    source.desc as *const u8,
                    self.desc as *mut u8,
                    self.desc_len as usize,
                );
            }
        }
        Ok(())
    }
    fn free(&mut self) -> Result<(), String> {
        free(self.name as *mut u8, self.name_len as usize)?;
        free(self.date as *mut u8, self.date_len as usize)?;
        free(self.desc as *mut u8, self.desc_len as usize)?;
        Ok(())
    }
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct drm_auth {
    magic: u32,
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct drm_i915_getparam {
    param: u32,
    value: *mut i32,
}
impl DeepCopy<drm_i915_getparam> for drm_i915_getparam {
    fn alloc(&mut self, _: &drm_i915_getparam) -> Result<(), String> {
        self.value = alloc(mem::size_of::<i32>())? as *mut i32;
        Ok(())
    }
    fn copy(&mut self, source: &drm_i915_getparam) -> Result<(), String> {
        unsafe {
            *self.value = *source.value;
        }
        Ok(())
    }
    fn free(&mut self) -> Result<(), String> {
        free(self.value as *mut u8, mem::size_of::<i32>())?;
        Ok(())
    }
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct i915_engine_class_instance {
    engine_class: u16,
    engine_instance: u16,
}
#[repr(C)]
#[repr(packed)]
#[allow(non_camel_case_types)]
struct i915_context_engines_parallel_submit {
    base: i915_user_extension,
    engine_index: u16,
    width: u16,
    num_siblings: u16,
    mbz16: u16,
    flags: u64,
    mbz64: [u64; 3],
    //struct i915_engine_class_instance engines[0];
}
#[repr(C)]
#[repr(packed)]
#[allow(non_camel_case_types)]
struct i915_context_engines_bond {
    base: i915_user_extension,
    master: i915_engine_class_instance,
    virtual_index: u16,
    num_bonds: u16,
    flags: u64,
    mbz64: [u64; 4],
    //struct i915_engine_class_instance engines[0];
}
#[repr(C)]
#[repr(packed)]
#[allow(non_camel_case_types)]
struct i915_context_engines_load_balance {
    base: i915_user_extension,
    engine_index: u16,
    num_siblings: u16,
    flags: u32,
    mbz64: u64,
    //struct i915_engine_class_instance engines[0];
}
#[repr(C)]
#[allow(non_camel_case_types)]
struct i915_context_param_engines {
    extensions: u64,
    //engines: [i915_engine_class_instance; 0],
}
impl i915_context_param_engines {
    fn sizeof(ext: u64) -> Result<usize, String> {
        let base = unsafe { &*(ext as *mut i915_user_extension) };
        match base.name & PRELIM_I915_USER_EXT_MASK {
            0 => {
                let t = unsafe { &*(ext as *mut i915_context_engines_load_balance) };
                let size = mem::size_of::<i915_context_engines_load_balance>()
                    + mem::size_of::<i915_engine_class_instance>()
                        .checked_mul(t.num_siblings as usize)
                        .ok_or(format!("mul error"))?;
                Ok(size)
            }
            1 => {
                let t = unsafe { &*(ext as *mut i915_context_engines_bond) };
                let size = mem::size_of::<i915_context_engines_bond>()
                    + mem::size_of::<i915_engine_class_instance>()
                        .checked_mul(t.num_bonds as usize)
                        .ok_or(format!("mul error"))?;
                Ok(size)
            }
            2 | 3 => {
                let t = unsafe { &*(ext as *mut i915_context_engines_parallel_submit) };
                let size = mem::size_of::<i915_context_engines_parallel_submit>()
                    + mem::size_of::<i915_engine_class_instance>()
                        .checked_mul(t.num_siblings as usize)
                        .ok_or(format!("mul error"))?
                        .checked_mul(t.width as usize)
                        .ok_or(format!("mul error"))?;
                Ok(size)
            }
            _ => Err(format!("base.name is not supported")),
        }
    }
    fn alloc(&mut self, source: &i915_context_param_engines) -> Result<(), String> {
        let mut ext_dst = &mut self.extensions;
        let mut ext_src = source.extensions;
        while ext_src != 0 {
            let size = Self::sizeof(ext_src)?;
            *ext_dst = alloc(size)? as u64;
            unsafe {
                ptr::copy(ext_src as *const u8, *ext_dst as *mut u8, size);
            }
            let base_src = unsafe { &mut *(ext_src as *mut i915_user_extension) };
            ext_src = base_src.next_extension;
            let base_dst = unsafe { &mut *(*ext_dst as *mut i915_user_extension) };
            ext_dst = &mut base_dst.next_extension;
        }
        Ok(())
    }
    fn free(&mut self) -> Result<(), String> {
        let mut ext = self.extensions;
        while ext != 0 {
            let base = unsafe { &mut *(ext as *mut i915_user_extension) };
            let next_ext = base.next_extension;
            let size = Self::sizeof(ext)?;
            free(ext as *mut u8, size)?;
            ext = next_ext;
        }
        Ok(())
    }
}
#[repr(C)]
#[allow(non_camel_case_types)]
struct drm_i915_gem_context_create_ext_setparam {
    base: i915_user_extension,
    param: drm_i915_gem_context_param,
}
#[repr(C)]
#[allow(non_camel_case_types)]
struct drm_i915_gem_context_create_ext {
    ctx_id: u32,
    flags: u32,
    extensions: u64,
}
impl drm_i915_gem_context_create_ext {
    fn sizeof(name: u32) -> Result<usize, String> {
        match name {
            // I915_CONTEXT_CREATE_EXT_SETPARAM
            0 => Ok(mem::size_of::<drm_i915_gem_context_create_ext_setparam>()),
            _ => Err(format!("name is not supported")),
        }
    }
    fn iterator(
        &mut self,
        source: &drm_i915_gem_context_create_ext,
        f: fn(&mut u64, u64) -> Result<(), String>,
    ) -> Result<(), String> {
        let ext_dst = &mut self.extensions;
        let ext_src = source.extensions;
        iterator::<_>(ext_src, ext_dst, f)
    }
}
impl DeepCopy<drm_i915_gem_context_create_ext> for drm_i915_gem_context_create_ext {
    fn alloc(&mut self, source: &drm_i915_gem_context_create_ext) -> Result<(), String> {
        /*info!(
            "drm_i915_gem_context_create_ext: ctx_id:{:?} flags:0x{:x} extensions:{:?}",
            self.ctx_id, self.flags, self.extensions
        );*/
        self.iterator(source, |dst: &mut u64, src: u64| -> Result<(), String> {
            let base_src = unsafe { &*(src as *mut i915_user_extension) };
            let size = Self::sizeof(base_src.name)?;
            info!("drm_i915_gem_context_create_ext: name:{:?}", base_src.name);
            *dst = alloc(size)? as u64;
            unsafe {
                ptr::copy(src as *const u8, *dst as *mut u8, size);
            }
            if base_src.name == 0 {
                let param_src = unsafe { &*(src as *mut drm_i915_gem_context_create_ext_setparam) };
                let param_dst =
                    unsafe { &mut *(*dst as *mut drm_i915_gem_context_create_ext_setparam) };
                info!(
                    "setparam: size:{:?} param:{:?}",
                    param_src.param.size, param_dst.param.param
                );
                if param_src.param.size > 0 {
                    param_dst.param.value = alloc(param_src.param.size as usize)? as u64;
                    unsafe {
                        ptr::copy(
                            param_src.param.value as *const u8,
                            param_dst.param.value as *mut u8,
                            param_src.param.size as usize,
                        );
                    }
                    if param_src.param.param == I915_CONTEXT_PARAM_ENGINES {
                        info!("I915_CONTEXT_PARAM_ENGINES is called ...");
                        let context_src = unsafe {
                            &mut *(param_src.param.value as *mut drm_i915_gem_context_param)
                        };
                        let context_dst = unsafe {
                            &mut *(param_dst.param.value as *mut drm_i915_gem_context_param)
                        };
                        context_dst.alloc(context_src, |src, dst| -> Result<(), String> {
                            let engines_src = unsafe { &*(src as *mut i915_context_param_engines) };
                            let engines_dst =
                                unsafe { &mut *(dst as *mut i915_context_param_engines) };
                            engines_dst.alloc(engines_src)
                        })?;
                    }
                } else {
                    param_dst.param.value = crate::memory::PTR_NULL;
                }
            }
            Ok(())
        })?;
        Ok(())
    }
    fn copy(&mut self, source: &drm_i915_gem_context_create_ext) -> Result<(), String> {
        self.ctx_id = source.ctx_id;
        self.flags = source.flags;
        self.iterator(source, |dst: &mut u64, src: u64| -> Result<(), String> {
            let base_dst = unsafe { &*(*dst as *mut i915_user_extension) };
            if base_dst.name == 0 {
                let param_dst =
                    unsafe { &mut *(*dst as *mut drm_i915_gem_context_create_ext_setparam) };
                let param_src = unsafe { &*(src as *mut drm_i915_gem_context_create_ext_setparam) };
                param_dst.param.ctx_id = param_src.param.ctx_id;
                param_dst.param.size = param_src.param.size;
            }
            Ok(())
        })?;
        Ok(())
    }
    fn free(&mut self) -> Result<(), String> {
        let mut ext = self.extensions;
        while ext != 0 {
            let base = unsafe { &mut *(ext as *mut i915_user_extension) };
            // Free other sub-items here.
            if base.name == 0 {
                let param = unsafe { &mut *(ext as *mut drm_i915_gem_context_create_ext_setparam) };
                /*unsafe {
                    ptr::copy(
                        param_u.param.value as *const u8,
                        param_t.param.value as *mut u8,
                        param_t.param.size as usize,
                    );
                }*/
                let context_dst =
                    unsafe { &mut *(param.param.value as *mut drm_i915_gem_context_param) };
                context_dst.free(|dst| -> Result<(), String> {
                    let engines = unsafe { &mut *(dst as *mut i915_context_param_engines) };
                    engines.free()
                })?;

                free(param.param.value as *mut u8, param.param.size as usize)?;
            }

            let next_ext = base.next_extension;
            let size = Self::sizeof(base.name)?;
            free(ext as *mut u8, size)?;

            ext = next_ext;
        }
        Ok(())
    }
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct drm_i915_gem_wait {
    bo_handle: u32,
    flags: u32,
    timeout_ns: i64,
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct drm_i915_gem_set_domain {
    handle: u32,
    read_domains: u32,
    write_domain: u32,
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct drm_i915_gem_get_aperture {
    aper_size: u64,
    aper_available_size: u64,
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct drm_i915_gem_busy {
    handle: u32,
    busy: u32,
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct drm_i915_gem_madvise {
    handle: u32,
    madv: u32,
    retained: u32,
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct drm_i915_gem_get_tiling {
    handle: u32,
    tiling_mode: u32,
    swizzle_mode: u32,
    phys_swizzle_mode: u32,
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct drm_i915_gem_sw_finish {
    handle: u32,
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct drm_i915_get_pipe_from_crtc_id {
    crtc_id: u32,
    pipe: u32,
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct prelim_drm_i915_gem_vm_region_ext {
    base: i915_user_extension,
    region: prelim_drm_i915_gem_memory_class_instance,
    pad: u32,
}
#[repr(C)]
#[allow(non_camel_case_types)]
struct drm_i915_gem_vm_control {
    extensions: u64,
    flags: u32,
    vm_id: u32,
}
impl drm_i915_gem_vm_control {
    fn sizeof(name: u32) -> Result<usize, String> {
        match name {
            0 => Ok(mem::size_of::<prelim_drm_i915_gem_vm_region_ext>()), // PRELIM_I915_GEM_VM_CONTROL_EXT_REGION
            _ => Err(format!("name is not supported")),
        }
    }
    fn iterator(
        &mut self,
        source: &drm_i915_gem_vm_control,
        f: fn(&mut u64, u64) -> Result<(), String>,
    ) -> Result<(), String> {
        let ext_dst = &mut self.extensions;
        let ext_src = source.extensions;
        iterator::<_>(ext_src, ext_dst, f)
    }
}
impl DeepCopy<drm_i915_gem_vm_control> for drm_i915_gem_vm_control {
    fn alloc(&mut self, source: &drm_i915_gem_vm_control) -> Result<(), String> {
        self.iterator(source, |dst: &mut u64, src: u64| -> Result<(), String> {
            let base_src = unsafe { &*(src as *mut i915_user_extension) };
            let size = Self::sizeof(base_src.name & PRELIM_I915_USER_EXT_MASK)?;
            *dst = alloc(size)? as u64;
            unsafe {
                ptr::copy(src as *const u8, *dst as *mut u8, size);
            }
            Ok(())
        })?;
        Ok(())
    }
    fn copy(&mut self, source: &drm_i915_gem_vm_control) -> Result<(), String> {
        self.vm_id = source.vm_id;
        //self.flags = source.flags;
        Ok(())
    }
    fn free(&mut self) -> Result<(), String> {
        let mut ext = self.extensions;
        while ext != 0 {
            let base = unsafe { &mut *(ext as *mut i915_user_extension) };
            let size = Self::sizeof(base.name & PRELIM_I915_USER_EXT_MASK)?;
            let next_ext = base.next_extension;
            free(ext as *mut u8, size)?;
            ext = next_ext;
        }
        Ok(())
    }
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct drm_i915_gem_mmap_offset {
    handle: u32,
    pad: u32,
    offset: u64,
    flags: u64,
    extensions: u64,
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct drm_i915_reset_stats {
    ctx_id: u32,
    flags: u32,
    reset_count: u32,
    batch_active: u32,
    batch_pending: u32,
    pad: u32,
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct drm_i915_reg_read {
    offset: u64,
    val: u64,
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct drm_prime_handle {
    handle: u32,
    flags: u32,
    fd: i32,
}

#[repr(C)]
#[allow(non_camel_case_types)]
pub struct drm_gem_close_t {
    handle: u32,
    pad: u32,
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct drm_i915_gem_exec_fence {
    handle: u32,
    flags: u32,
}
#[repr(C)]
#[allow(non_camel_case_types)]
struct drm_i915_gem_exec_object2 {
    handle: u32,
    relocation_count: u32,
    relocs_ptr: u64,
    alignment: u64,
    offset: u64,
    flags: u64,
    rsvd1: u64,
    rsvd2: u64,
}
#[repr(C)]
#[allow(non_camel_case_types)]
struct drm_i915_gem_execbuffer2 {
    buffers_ptr: u64,
    buffer_count: u32,
    batch_start_offset: u32,
    batch_len: u32,
    dr1: u32,
    dr4: u32,
    num_cliprects: u32,
    cliprects_ptr: u64,
    flags: u64,
    rsvd1: u64,
    rsvd2: u64,
}
impl DeepCopy<drm_i915_gem_execbuffer2> for drm_i915_gem_execbuffer2 {
    fn alloc(&mut self, source: &drm_i915_gem_execbuffer2) -> Result<(), String> {
        if source.buffer_count > 0 {
            let size = mem::size_of::<drm_i915_gem_exec_object2>()
                .checked_mul(source.buffer_count as usize)
                .ok_or(format!("mul error"))?;
            self.buffers_ptr = alloc(size)? as u64;
            unsafe {
                ptr::copy(
                    source.buffers_ptr as *const u8,
                    self.buffers_ptr as *mut u8,
                    size,
                );
            }
        }
        if source.num_cliprects > 0 {
            let size = mem::size_of::<drm_i915_gem_exec_fence>()
                .checked_mul(source.num_cliprects as usize)
                .ok_or(format!("mul error"))?;
            self.cliprects_ptr = alloc(size)? as u64;
            unsafe {
                ptr::copy(
                    source.cliprects_ptr as *const u8,
                    self.cliprects_ptr as *mut u8,
                    size,
                );
            }
        }
        Ok(())
    }
    fn copy(&mut self, source: &drm_i915_gem_execbuffer2) -> Result<(), String> {
        if self.buffer_count > 0 {
            let size = mem::size_of::<drm_i915_gem_exec_object2>()
                .checked_mul(self.buffer_count as usize)
                .ok_or(format!("mul error"))?;
            unsafe {
                ptr::copy(
                    source.buffers_ptr as *const u8,
                    self.buffers_ptr as *mut u8,
                    size,
                );
            }
        }
        if self.num_cliprects > 0 {
            let size = mem::size_of::<drm_i915_gem_exec_fence>()
                .checked_mul(self.num_cliprects as usize)
                .ok_or(format!("mul error"))?;
            unsafe {
                ptr::copy(
                    source.cliprects_ptr as *const u8,
                    self.cliprects_ptr as *mut u8,
                    size,
                );
            }
        }
        Ok(())
    }
    fn free(&mut self) -> Result<(), String> {
        if self.buffer_count > 0 {
            let size = mem::size_of::<drm_i915_gem_exec_object2>()
                .checked_mul(self.buffer_count as usize)
                .ok_or(format!("mul error"))?;
            free(self.buffers_ptr as *mut u8, size)?;
        }
        if self.num_cliprects > 0 {
            let size = mem::size_of::<drm_i915_gem_exec_fence>()
                .checked_mul(self.num_cliprects as usize)
                .ok_or(format!("mul error"))?;
            free(self.cliprects_ptr as *mut u8, size)?;
        }
        Ok(())
    }
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct drm_i915_gem_userptr {
    user_ptr: u64,
    user_size: u64,
    flags: u32,
    handle: u32,
}
fn i915_gem_userptr_ioctl(fd: i32, cmd: &u32, arg: *const u8) -> Result<i32, String> {
    let ptr_t = arg as *mut u8;
    let _arg_t = unsafe { &mut *(ptr_t as *mut drm_i915_gem_userptr) };

    /*let outside_enclave = sgx_trts::trts::rsgx_raw_is_outside_enclave(
        arg_t.user_ptr as *const u8,
        arg_t.user_size as usize,
    );
    // The user_ptr must outside enclave and should be set by applcation.
    info!("<drm_i915_gem_userptr> is outside enclave:{:?}", outside_enclave);*/

    exec::<drm_i915_gem_userptr>(fd, cmd, arg)
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct drm_i915_gem_context_destroy {
    ctx_id: u32,
    pad: u32,
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct drm_i915_gem_pread {
    handle: u32,
    pad: u32,
    offset: u64,
    size: u64,
    data_ptr: u64,
}
impl DeepCopy<drm_i915_gem_pread> for drm_i915_gem_pread {
    fn alloc(&mut self, _source: &drm_i915_gem_pread) -> Result<(), String> {
        if self.size > 0 {
            self.data_ptr = alloc(self.size as usize)? as u64;
        }
        Ok(())
    }
    fn copy(&mut self, source: &drm_i915_gem_pread) -> Result<(), String> {
        if self.size > 0 {
            unsafe {
                ptr::copy(
                    source.data_ptr as *mut u8,
                    self.data_ptr as *mut u8,
                    self.size as usize,
                );
            }
        }
        Ok(())
    }
    fn free(&mut self) -> Result<(), String> {
        if self.size > 0 {
            free(self.data_ptr as *mut u8, self.size as usize)?;
        }
        Ok(())
    }
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct drm_i915_gem_pwrite {
    handle: u32,
    pad: u32,
    offset: u64,
    size: u64,
    data_ptr: u64,
}
impl DeepCopy<drm_i915_gem_pwrite> for drm_i915_gem_pwrite {
    fn alloc(&mut self, source: &drm_i915_gem_pwrite) -> Result<(), String> {
        if self.size > 0 {
            self.data_ptr = alloc(self.size as usize)? as u64;
            unsafe {
                ptr::copy(
                    source.data_ptr as *mut u8,
                    self.data_ptr as *mut u8,
                    self.size as usize,
                );
            }
        }
        Ok(())
    }
    fn copy(&mut self, _source: &drm_i915_gem_pwrite) -> Result<(), String> {
        /*if self.size > 0 {
            unsafe {
                ptr::copy(source.data_ptr as *mut u8, self.data_ptr as *mut u8, self.size as usize);
            }
        }*/
        Ok(())
    }
    fn free(&mut self) -> Result<(), String> {
        if self.size > 0 {
            free(self.data_ptr as *mut u8, self.size as usize)?;
        }
        Ok(())
    }
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct drm_i915_gem_mmap {
    handle: u32,
    pad: u32,
    offset: u64,
    size: u64,
    addr_ptr: u64,
    flags: u64,
}

fn drm_default_ioctl(_fd: i32, cmd: &u32, _arg: *const u8) -> Result<i32, String> {
    info!("unsupported ioctl:{:?} !!!", cmd);
    Err(format!("unsupported ioctl: {:?}", cmd))
    /*let mut ret: i32 = 0;
    unsafe {
        let status = occlum_ocall_device_ioctl(
            &mut ret as *mut i32,
            fd,
            cmd.to_owned() as c_int,
            arg as u64,
        );
        assert!(status == sgx_status_t::SGX_SUCCESS);
    }
    Ok(ret)*/
}

#[no_mangle]
pub fn pxp_ioctl(fd: i32, cmd: u32, arg: *const u8) -> i32 {
    match cmd {
        // Consumed by i915 driver's drm_gem_close_ioctl()
        DRM_IOCTL_GEM_CLOSE => exec::<drm_gem_close_t>(fd, &cmd, arg),
        // Consumed by i915 driver's drm_getmagic()
        DRM_IOCTL_GET_MAGIC => exec::<drm_auth>(fd, &cmd, arg),
        // Consumed by i915 driver's drm_authmagic()
        DRM_IOCTL_AUTH_MAGIC => exec::<drm_auth>(fd, &cmd, arg),
        // Consumed by i915 driver's i915_pxp_ops_ioctl()
        PRELIM_DRM_IOCTL_I915_PXP_OPS => exec2::<prelim_drm_i915_pxp_ops>(fd, &cmd, arg),
        // Consumed by i915 driver's i915_gem_create_ioctl()
        DRM_IOCTL_I915_GEM_CREATE_EXT => exec2::<prelim_drm_i915_gem_create_ext>(fd, &cmd, arg),
        // Consumed by i915 driver's i915_query_ioctl()
        DRM_IOCTL_I915_QUERY => exec2::<drm_i915_query>(fd, &cmd, arg),
        // Consumed by i915 driver's i915_gem_param_ioctl()
        DRM_IOCTL_I915_GEM_CONTEXT_GETPARAM => exec2::<drm_i915_gem_context_param>(fd, &cmd, arg),
        // Consumed by i915 driver's i915_gem_param_ioctl()
        DRM_IOCTL_I915_GEM_CONTEXT_SETPARAM => exec2::<drm_i915_gem_context_param>(fd, &cmd, arg),
        // Consumed by i915 driver's drm_version()
        DRM_IOCTL_VERSION => exec2::<drm_version>(fd, &cmd, arg),
        // Consumed by i915 driver's i915_getparam_ioctl()
        DRM_IOCTL_I915_GETPARAM => exec2::<drm_i915_getparam>(fd, &cmd, arg),
        // Consumed by i915 driver's i915_gem_context_create_ioctl()
        DRM_IOCTL_I915_GEM_CONTEXT_CREATE_EXT => {
            exec2::<drm_i915_gem_context_create_ext>(fd, &cmd, arg)
        }
        // Consumed by i915 driver's i915_gem_vm_create_ioctl()
        DRM_IOCTL_I915_GEM_VM_CREATE => exec2::<drm_i915_gem_vm_control>(fd, &cmd, arg),
        // Consumed by i915 driver's i915_gem_vm_destroy_ioctl()
        DRM_IOCTL_I915_GEM_VM_DESTROY => exec::<drm_i915_gem_vm_control>(fd, &cmd, arg),
        // Consumed by i915 driver's i915_gem_mmap_offset_ioctl()
        DRM_IOCTL_I915_GEM_MMAP_OFFSET => exec::<drm_i915_gem_mmap_offset>(fd, &cmd, arg),
        // Consumed by i915 driver's i915_gem_context_reset_stats_ioctl()
        DRM_IOCTL_I915_GET_RESET_STATS => exec::<drm_i915_reset_stats>(fd, &cmd, arg),
        // Consumed by i915 driver's i915_gem_get_aperture_ioctl()
        DRM_IOCTL_I915_GEM_GET_APERTURE => exec::<drm_i915_gem_get_aperture>(fd, &cmd, arg),
        // Consumed by i915 driver's i915_gem_set_domain_ioctl()
        DRM_IOCTL_I915_GEM_SET_DOMAIN => exec::<drm_i915_gem_set_domain>(fd, &cmd, arg),
        // Consumed by i915 driver's i915_gem_execbuffer2_ioctl()
        DRM_IOCTL_I915_GEM_EXECBUFFER2_WR => exec2::<drm_i915_gem_execbuffer2>(fd, &cmd, arg),
        // Consumed by i915 driver's i915_gem_execbuffer2_ioctl()
        DRM_IOCTL_I915_GEM_EXECBUFFER2 => exec2::<drm_i915_gem_execbuffer2>(fd, &cmd, arg),
        // Consumed by i915 driver's i915_gem_userptr_ioctl()
        DRM_IOCTL_I915_GEM_USERPTR => exec::<drm_i915_gem_userptr>(fd, &cmd, arg),
        // Consumed by i915 driver's i915_gem_get_tiling_ioctl()
        DRM_IOCTL_I915_GEM_GET_TILING => exec::<drm_i915_gem_get_tiling>(fd, &cmd, arg),
        // Consumed by i915 driver's i915_gem_wait_ioctl()
        DRM_IOCTL_I915_GEM_WAIT => exec::<drm_i915_gem_wait>(fd, &cmd, arg),
        // Consumed by i915 driver's i915_gem_context_destroy_ioctl()
        DRM_IOCTL_I915_GEM_CONTEXT_DESTROY => exec::<drm_gem_close_t>(fd, &cmd, arg),
        // Consumed by i915 driver's i915_reg_read_ioctl()
        DRM_IOCTL_I915_REG_READ => exec::<drm_i915_reg_read>(fd, &cmd, arg),
        // Consumed by i915 driver's i915_gem_busy_ioctl()
        DRM_IOCTL_I915_GEM_BUSY => exec::<drm_i915_gem_busy>(fd, &cmd, arg),
        // Consumed by i915 driver's drm_prime_handle_to_fd_ioctl()
        DRM_IOCTL_PRIME_HANDLE_TO_FD => exec::<drm_prime_handle>(fd, &cmd, arg),
        // Consumed by i915 driver's drm_prime_fd_to_handle_ioctl()
        DRM_IOCTL_PRIME_FD_TO_HANDLE => exec::<drm_prime_handle>(fd, &cmd, arg),
        // Consumed by i915 driver's intel_get_pipe_from_crtc_id_ioctl()
        DRM_IOCTL_I915_GET_PIPE_FROM_CRTC_ID => {
            exec::<drm_i915_get_pipe_from_crtc_id>(fd, &cmd, arg)
        }
        // Consumed by i915 driver's i915_gem_sw_finish_ioctl()
        DRM_IOCTL_I915_GEM_SW_FINISH => exec::<drm_i915_gem_sw_finish>(fd, &cmd, arg),
        // Consumed by i915 driver's i915_gem_madvise_ioctl()
        DRM_IOCTL_I915_GEM_MADVISE => exec::<drm_i915_gem_madvise>(fd, &cmd, arg),
        // Consumed by i915 driver's i915_gem_pread_ioctl()
        DRM_IOCTL_I915_GEM_PREAD => exec2::<drm_i915_gem_pread>(fd, &cmd, arg),
        // Consumed by i915 driver's i915_gem_pwrite_ioctl()
        DRM_IOCTL_I915_GEM_PWRITE => exec2::<drm_i915_gem_pwrite>(fd, &cmd, arg),
        // Consumed by i915 driver's i915_gem_mmap_ioctl()
        DRM_IOCTL_I915_GEM_MMAP => exec::<drm_i915_gem_mmap>(fd, &cmd, arg),
        _ => drm_default_ioctl(fd, &cmd, arg),
    }
    .unwrap()
}

cfg_if::cfg_if! {
    if #[cfg(feature = "occlum")] {
        extern "C" {
            fn occlum_ocall_device_ioctl(
                ret: *mut i32,
                fd: i32,  //c_int
                request: i32,  //c_int
                arg: u64,
            ) -> sgx_status_t;
        }
    } else {
        extern "C" {
            fn ocall_pxp_ioctl(
                ret: *mut i32,
                fd: i32,  //c_int
                request: i32,  //c_int
                arg: u64,
            ) -> sgx_status_t;
        }
    }
}
