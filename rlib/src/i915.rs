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

#[derive(PartialEq)]
enum Direction {
    u2t,
    t2u,
    none,
}

trait DeepCopy<T> {
    fn alloc(&mut self, source: &T) -> Result<(), String>;
    fn copy(&mut self, source: &T, direction: Direction) -> Result<(), String>;
    fn free(&mut self) -> Result<(), String>;
}

#[allow(unused_variables)]
fn iterator<F>(mut ext_src: u64, mut ext_dst: &mut u64, f: F) -> Result<(), String>
where
    F: Fn(u64, &mut u64) -> Result<(), String>,
{
    while ext_src != 0 {
        f(ext_src, ext_dst).unwrap();
        ext_src = unsafe { &*(ext_src as *const i915_user_extension) }.next_extension;
        ext_dst = &mut unsafe { &mut *(*ext_dst as *mut i915_user_extension) }.next_extension;
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
    let size = mem::size_of::<T>();
    let ptr_t = arg as *mut T;
    let ptr_u = alloc(size)?;
    unsafe { ptr::copy(ptr_t as *const u8, ptr_u as *mut u8, size); }
    let ret = ioctl(fd, cmd, ptr_u);
    unsafe { ptr::copy(ptr_u as *const u8, ptr_t as *mut u8, size); }
    free(ptr_u, size)?;
    Ok(ret)
}

fn exec2<T: DeepCopy<T>>(fd: i32, cmd: &u32, arg: *const u8) -> Result<i32, String> {
    let size = mem::size_of::<T>();
    let ptr_t = arg as *mut T;
    let ptr_u = alloc(size)?;
    let arg_t = unsafe { &mut *(ptr_t as *mut T) };
    let arg_u = unsafe { &mut *(ptr_u as *mut T) };
    arg_u.alloc(arg_t)?;
    arg_u.copy(arg_t, Direction::t2u)?;
    let ret = ioctl(fd, cmd, ptr_u);
    arg_t.copy(arg_u, Direction::u2t)?;
    arg_u.free()?;
    free(ptr_u, size)?;
    Ok(ret)
}

#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone)]
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
impl i915_user_extension {
    fn init(&mut self) {
        // Must init this next extension manually.
        self.next_extension = 0;
    }
    fn copy(&mut self, source: &i915_user_extension) {
        self.name = source.name;
        self.flags = source.flags;
        self.rsvd = source.rsvd;
    }
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct prelim_drm_i915_gem_create_ext_setparam {
    base: i915_user_extension,
    param: prelim_drm_i915_gem_object_param,
}
impl prelim_drm_i915_gem_create_ext_setparam {
    fn alloc(&mut self, source: &prelim_drm_i915_gem_create_ext_setparam) -> Result<(), String> {
        let size = mem::size_of::<prelim_drm_i915_gem_memory_class_instance>()
            .checked_mul(source.param.size as usize)
            .ok_or(format!("mul error"))?;
        self.param.data = alloc(size as usize)? as u64;
        Ok(())
    }
    fn copy(&mut self, source: &prelim_drm_i915_gem_create_ext_setparam) -> Result<(), String> {
        self.param.handle = source.param.handle;
        self.param.size = source.param.size;
        self.param.param = source.param.param;
        let size = mem::size_of::<prelim_drm_i915_gem_memory_class_instance>()
            .checked_mul(source.param.size as usize)
            .ok_or(format!("mul error"))?;
        unsafe {
            ptr::copy(source.param.data as *const u8, self.param.data as *mut u8, size);
        }
        Ok(())
    }
    fn free(&mut self) -> Result<(), String> {
        let size = mem::size_of::<prelim_drm_i915_gem_memory_class_instance>()
            .checked_mul(self.param.size as usize)
            .ok_or(format!("mul error"))?;
        free(self.param.data as *mut u8, size)
    }
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
    fn iterator(
        &mut self,
        source: &prelim_drm_i915_gem_create_ext,
        f: fn(u64, &mut u64) -> Result<(), String>,
    ) -> Result<(), String> {
        let ext_src = source.extensions;
        let ext_dst = &mut self.extensions;
        iterator::<_>(ext_src, ext_dst, f)
    }
}
impl DeepCopy<prelim_drm_i915_gem_create_ext> for prelim_drm_i915_gem_create_ext {
    fn alloc(&mut self, source: &prelim_drm_i915_gem_create_ext) -> Result<(), String> {
        self.extensions = 0;
        self.iterator(source, |src: u64, dst: &mut u64| -> Result<(), String> {
            let ext_src = unsafe { &*(src as *const i915_user_extension) };
            let size = Self::sizeof(ext_src.name & PRELIM_I915_USER_EXT_MASK)?;
            *dst = alloc(size as usize)? as u64;
            unsafe { &mut *(*dst as *mut i915_user_extension) }.init();
            if ext_src.name & PRELIM_I915_USER_EXT_MASK == 1 {
                let s = unsafe { &*(src as *const prelim_drm_i915_gem_create_ext_setparam) };
                let d =
                    unsafe { &mut *(*dst as *mut prelim_drm_i915_gem_create_ext_setparam) };
                d.alloc(s)?;
            }
            Ok(())
        })
    }
    fn copy(&mut self, source: &prelim_drm_i915_gem_create_ext, _: Direction) -> Result<(), String> {
        self.size = source.size;
        self.handle = source.handle;
        self.pad = source.pad;
        // Copy extensions
        self.iterator(source, |src: u64, dst: &mut u64| -> Result<(), String> {
            let ext_src = unsafe { &*(src as *const i915_user_extension) };
            let ext_dst = unsafe { &mut *(*dst as *mut i915_user_extension) };
            if ext_src.name & PRELIM_I915_USER_EXT_MASK == 1 {
                // Deep copy
                ext_dst.copy(ext_src);
                let s = unsafe { &*(src as *const prelim_drm_i915_gem_create_ext_setparam) };
                let d =
                    unsafe { &mut *(*dst as *mut prelim_drm_i915_gem_create_ext_setparam) };
                d.copy(s)?;
            } else {
                let next = ext_dst.next_extension;
                let size = Self::sizeof(ext_src.name & PRELIM_I915_USER_EXT_MASK)?;
                unsafe {
                    ptr::copy(src as *const u8, *dst as *mut u8, size);
                }
                ext_dst.next_extension = next;
            }
            Ok(())
        })
    }
    fn free(&mut self) -> Result<(), String> {
        let mut ext = self.extensions;
        while ext != 0 {
            let extension = unsafe { &*(ext as *const i915_user_extension) };
            if extension.name & PRELIM_I915_USER_EXT_MASK == 1 {
                unsafe { &mut *(ext as *mut prelim_drm_i915_gem_create_ext_setparam) }.free()?;
            }
            let next = extension.next_extension;
            free(
                ext as *mut u8,
                Self::sizeof(extension.name & PRELIM_I915_USER_EXT_MASK)?,
            )?;
            ext = next;
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
    fn sizeof(action: &u32) -> Result<usize, String> {
        match action.to_owned() {
            0 => Ok(mem::size_of::<prelim_drm_i915_pxp_set_session_status_params>()),
            1 => Ok(mem::size_of::<prelim_drm_i915_pxp_tee_io_message_params>()),
            2 => Ok(mem::size_of::<prelim_drm_i915_pxp_query_tag>()),
            _ => Err(format!("the action is illegal !!!")),
        }
    }
}
impl DeepCopy<prelim_drm_i915_pxp_ops> for prelim_drm_i915_pxp_ops {
    fn alloc(&mut self, source: &prelim_drm_i915_pxp_ops) -> Result<(), String> {
        let size = Self::sizeof(&source.action)?;
        self.params = alloc(size)? as u64;
        if source.action == 1 {
            let s =
                unsafe { &*(source.params as *const prelim_drm_i915_pxp_tee_io_message_params) };
            let d =
                unsafe { &mut *(self.params as *mut prelim_drm_i915_pxp_tee_io_message_params) };
            d.msg_in = alloc(s.msg_in_size as usize)? as u64;
            d.msg_out = alloc(s.msg_out_buf_size as usize)? as u64;
        }
        Ok(())
    }
    fn copy(&mut self, source: &prelim_drm_i915_pxp_ops, direction: Direction) -> Result<(), String> {
        self.action = source.action;
        self.status = source.status;
        // Deep copy
        match source.action {
            0 | 2 => unsafe {
                ptr::copy(
                    source.params as *const u8,
                    self.params as *mut u8,
                    Self::sizeof(&source.action)?,
                );
            },
            1 => {
                let s =
                    unsafe { &*(source.params as *const prelim_drm_i915_pxp_tee_io_message_params) };
                let d = unsafe {
                    &mut *(self.params as *mut prelim_drm_i915_pxp_tee_io_message_params)
                };
                d.msg_in_size = s.msg_in_size;
                d.msg_out_buf_size = s.msg_out_buf_size;
                d.msg_out_ret_size = s.msg_out_ret_size;
                if s.msg_in_size > 0 {
                    unsafe {
                        ptr::copy(
                            s.msg_in as *const u8,
                            d.msg_in as *mut u8,
                            s.msg_in_size as usize,
                        );
                    }
                }
                if direction == Direction::t2u {
                    // t2u
                    if s.msg_out_buf_size > 0 {
                        unsafe {
                            ptr::copy(
                                s.msg_out as *const u8,
                                d.msg_out as *mut u8,
                                s.msg_out_buf_size as usize,
                            );
                        }
                    }
                } else {
                    // u2t
                    if s.msg_out_ret_size > 0 {
                        unsafe {
                            ptr::copy(
                                s.msg_out as *const u8,
                                d.msg_out as *mut u8,
                                s.msg_out_ret_size as usize,
                            );
                        }
                    }
                }
            }
            _ => error!("the action is illegal, so do nonthing"),
        }
        Ok(())
    }
    fn free(&mut self) -> Result<(), String> {
        if self.action == 1 {
            let param =
                unsafe { &*(self.params as *mut prelim_drm_i915_pxp_tee_io_message_params) };
            free(param.msg_in as *mut u8, param.msg_in_size as usize)?;
            free(param.msg_out as *mut u8, param.msg_out_buf_size as usize)?;
        }
        free(self.params as *mut u8, Self::sizeof(&self.action)?)
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
impl drm_i915_query_item {
    fn alloc(&mut self, source: &drm_i915_query_item) -> Result<(), String> {
        self.data_ptr = alloc(source.length as usize)? as u64;
        Ok(())
    }
    fn copy(&mut self, source: &drm_i915_query_item) {
        self.query_id = source.query_id;
        self.length = source.length;
        self.flags = source.flags;
        if source.length > 0 {
            unsafe {
                ptr::copy(
                    source.data_ptr as *const u8,
                    self.data_ptr as *mut u8,
                    source.length as usize,
                );
            }
        }
    }
    fn free(&mut self) -> Result<(), String> {
        free(self.data_ptr as *mut u8, self.length as usize)
    }
}
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct drm_i915_query {
    num_items: u32,
    flags: u32,
    items_ptr: u64,
}
impl drm_i915_query {
    fn items_size(num_items: &u32) -> Result<usize, String> {
        mem::size_of::<drm_i915_query_item>()
            .checked_mul(num_items.to_owned() as usize)
            .ok_or(format!("mul error"))
    }
    fn iterator(
        &mut self,
        source: &drm_i915_query,
        direction: &Direction,
        f: fn(&drm_i915_query_item, &mut drm_i915_query_item, direction: &Direction) -> Result<(), String>,
    ) -> Result<(), String> {
        let offset = mem::size_of::<drm_i915_query_item>();
        for i in 0..source.num_items {
            let s = unsafe {
                &*((source.items_ptr as *mut u8)
                    .add(offset.checked_mul(i as usize).ok_or(format!("mul error"))?)
                    as *const drm_i915_query_item)
            };
            let d = unsafe {
                &mut *((self.items_ptr as *mut u8)
                    .add(offset.checked_mul(i as usize).ok_or(format!("mul error"))?)
                    as *mut drm_i915_query_item)
            };
            f(s, d, direction)?;
        }
        Ok(())
    }
}
impl DeepCopy<drm_i915_query> for drm_i915_query {
    fn alloc(&mut self, source: &drm_i915_query) -> Result<(), String> {
        let size = Self::items_size(&source.num_items)?;
        self.items_ptr = alloc(size)? as u64;
        self.iterator(
            source,
            &Direction::none,
            |src: &drm_i915_query_item, dst: &mut drm_i915_query_item, _: &Direction | {
                dst.alloc(src)
            },
        )
    }
    fn copy(&mut self, source: &drm_i915_query, direction: Direction) -> Result<(), String> {
        self.num_items = source.num_items;
        self.flags = source.flags;
        // Deep copy
        self.iterator(
            source,
            &direction,
            |src: &drm_i915_query_item, dst: &mut drm_i915_query_item, direction: &Direction| -> Result<(), String> {
                if direction == &Direction::u2t && dst.length == 0 {
                    dst.length = src.length;
                    return Ok(());
                }
                dst.copy(src);
                Ok(())
            }
        )
    }
    fn free(&mut self) -> Result<(), String> {
        let size = Self::items_size(&self.num_items)?;
        for i in 0..self.num_items {
            let item = unsafe {
                &mut *((self.items_ptr as *mut u8).add(
                    mem::size_of::<drm_i915_query_item>()
                        .checked_mul(i as usize)
                        .ok_or(format!("mul error"))?,
                ) as *mut drm_i915_query_item)
            };
            item.free()?;
        }
        free(self.items_ptr as *mut u8, size)
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
        self.value = alloc(source.size as usize)? as u64;
        f(self.value, source.value)
    }
    fn copy(
        &mut self,
        source: &drm_i915_gem_context_param,
        f: fn(dst: u64, src: u64) -> Result<(), String>,
    ) -> Result<(), String> {
        self.ctx_id = source.ctx_id;
        self.size = source.size;
        self.param = source.param;
        if source.size > 0 {
            unsafe {
                ptr::copy(
                    source.value as *const u8,
                    self.value as *mut u8,
                    source.size as usize,
                );
            }
        } else {
            self.value = source.value;
        }
        f(self.value, source.value)
    }
    fn free(&mut self, f: fn(dst: u64) -> Result<(), String>) -> Result<(), String> {
        f(self.value)?;
        free(self.value as *mut u8, self.size as usize)
    }
}
impl DeepCopy<drm_i915_gem_context_param> for drm_i915_gem_context_param {
    fn alloc(&mut self, source: &drm_i915_gem_context_param) -> Result<(), String> {
        self.alloc(source, |_src, _dst| -> Result<(), String> { Ok(()) })
    }
    fn copy(&mut self, source: &drm_i915_gem_context_param, direction: Direction) -> Result<(), String> {
        if direction == Direction::u2t && self.size == 0 {
            // Special case
            self.size = source.size;
            self.value = source.value;
            return Ok(());
        }
        self.copy(source, |_dst, _src| -> Result<(), String> { Ok(()) })
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
macro_rules! drm_version_copy {
    ($src:expr, $dst:expr, $size:ident, $ptr:ident, $direction:expr) => {
        if $direction == Direction::t2u {
            // t2u
            $dst.$size = $src.$size;
            if $src.$size > 0 {
                unsafe {
                    ptr::copy(
                        $src.$ptr as *const u8,
                        $dst.$ptr as *mut u8,
                        $src.$size as usize,
                    );
                }
            }
        } else {
            // u2t
            if $dst.$size == 0 {
                $dst.$size = $src.$size;
            } else {
                unsafe {
                    ptr::copy(
                        $src.$ptr as *const u8,
                        $dst.$ptr as *mut u8,
                        $dst.$size as usize,
                    );
                }
            }
        }
    }
}
impl DeepCopy<drm_version> for drm_version {
    fn alloc(&mut self, source: &drm_version) -> Result<(), String> {
        self.name = alloc(source.name_len as usize)?;
        self.date = alloc(source.date_len as usize)?;
        self.desc = alloc(source.desc_len as usize)?;
        Ok(())
    }
    fn copy(&mut self, source: &drm_version, direction: Direction) -> Result<(), String> {
        self.version_major = source.version_major;
        self.version_minor = source.version_minor;
        self.version_patchlevel = source.version_patchlevel;
        // Deep copy
        // Note: Must check self.name_len, self.date_len and self.desc_len values.
        drm_version_copy!(source, self, name_len, name, direction);
        drm_version_copy!(source, self, date_len, date, direction);
        drm_version_copy!(source, self, desc_len, desc, direction);
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
    fn copy(&mut self, source: &drm_i915_getparam, _: Direction) -> Result<(), String> {
        self.param = source.param;
        // Deep copy
        unsafe {
            *self.value = *source.value;
        }
        Ok(())
    }
    fn free(&mut self) -> Result<(), String> {
        free(self.value as *mut u8, mem::size_of::<i32>())
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
    fn sizeof(addr: u64) -> Result<usize, String> {
        let ext = unsafe { &*(addr as *const i915_user_extension) };
        match ext.name & PRELIM_I915_USER_EXT_MASK {
            0 => {
                let t = unsafe { &*(addr as *const i915_context_engines_load_balance) };
                let size = mem::size_of::<i915_context_engines_load_balance>()
                    + mem::size_of::<i915_engine_class_instance>()
                        .checked_mul(t.num_siblings as usize)
                        .ok_or(format!("mul error"))?;
                Ok(size)
            }
            1 => {
                let t = unsafe { &*(addr as *const i915_context_engines_bond) };
                let size = mem::size_of::<i915_context_engines_bond>()
                    + mem::size_of::<i915_engine_class_instance>()
                        .checked_mul(t.num_bonds as usize)
                        .ok_or(format!("mul error"))?;
                Ok(size)
            }
            2 | 3 => {
                let t = unsafe { &*(addr as *const i915_context_engines_parallel_submit) };
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
    fn iterator(
        &mut self,
        source: &i915_context_param_engines,
        f: fn(u64, &mut u64) -> Result<(), String>,
    ) -> Result<(), String> {
        let ext_dst = &mut self.extensions;
        let ext_src = source.extensions;
        iterator::<_>(ext_src, ext_dst, f)
    }
    fn alloc(&mut self, source: &i915_context_param_engines) -> Result<(), String> {
        self.iterator(source, |src: u64, dst: &mut u64| -> Result<(), String> {
            let size = Self::sizeof(src)?;
            *dst = alloc(size)? as u64;
            unsafe { &mut *(*dst as *mut i915_user_extension) }.init();
            Ok(())
        })
    }
    fn copy(&mut self, source: &i915_context_param_engines) -> Result<(), String> {
        self.iterator(source, |src: u64, dst: &mut u64| -> Result<(), String> {
            let next = unsafe { &*(*dst as *const i915_user_extension) }.next_extension;
            // Deep copy extension
            let size = Self::sizeof(src)?;
            unsafe {
                ptr::copy(src as *const u8, *dst as *mut u8, size);
            }
            unsafe { &mut *(*dst as *mut i915_user_extension) }.next_extension = next;
            Ok(())
        })
    }
    fn free(&mut self) -> Result<(), String> {
        let mut ext = self.extensions;
        while ext != 0 {
            let next = unsafe { &*(ext as *const i915_user_extension) }.next_extension;
            let size = Self::sizeof(ext)?;
            free(ext as *mut u8, size)?;
            ext = next;
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
impl drm_i915_gem_context_create_ext_setparam {
    fn alloc(&mut self, source: &drm_i915_gem_context_create_ext_setparam) -> Result<(), String> {
        if source.param.size > 0 {
            self.param.value = alloc(source.param.size as usize)? as u64;
            if source.param.param == I915_CONTEXT_PARAM_ENGINES {
                info!("I915_CONTEXT_PARAM_ENGINES is called ...");
                let context_src = unsafe {
                    &mut *(source.param.value as *mut drm_i915_gem_context_param)
                };
                let context_dst = unsafe {
                    &mut *(self.param.value as *mut drm_i915_gem_context_param)
                };
                context_dst.alloc(context_src, |src, dst| -> Result<(), String> {
                    let engines_src = unsafe { &*(src as *const i915_context_param_engines) };
                    let engines_dst =
                        unsafe { &mut *(dst as *mut i915_context_param_engines) };
                    engines_dst.alloc(engines_src)
                })?;
            }
        } else {
            self.param.value = crate::memory::PTR_NULL;
        }
        Ok(())
    }
    fn copy(&mut self, source: &drm_i915_gem_context_create_ext_setparam) -> Result<(), String> {
        self.param.ctx_id = source.param.ctx_id;
        self.param.size = source.param.size;
        self.param.param = source.param.param;
        if source.param.size > 0 {
            unsafe {
                ptr::copy(
                    source.param.value as *const u8,
                    self.param.value as *mut u8,
                    source.param.size as usize,
                );
            }
            if source.param.param == I915_CONTEXT_PARAM_ENGINES {
                let context_src = unsafe {
                    &*(source.param.value as *const drm_i915_gem_context_param)
                };
                let context_dst = unsafe {
                    &mut *(self.param.value as *mut drm_i915_gem_context_param)
                };
                context_dst.copy(context_src, |src, dst| -> Result<(), String> {
                    let engines_src = unsafe { &*(src as *const i915_context_param_engines) };
                    let engines_dst =
                        unsafe { &mut *(dst as *mut i915_context_param_engines) };
                    engines_dst.copy(engines_src)
                })?;
            }
        }
        Ok(())
    }
    fn free(&mut self) -> Result<(), String> {
        let context =
            unsafe { &mut *(self.param.value as *mut drm_i915_gem_context_param) };
        context.free(|dst| -> Result<(), String> {
            let engines = unsafe { &mut *(dst as *mut i915_context_param_engines) };
            engines.free()
        })?;

        free(self.param.value as *mut u8, self.param.size as usize)?;
        Ok(())
    }
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
        f: fn(u64, &mut u64) -> Result<(), String>,
    ) -> Result<(), String> {
        let ext_dst = &mut self.extensions;
        let ext_src = source.extensions;
        iterator::<_>(ext_src, ext_dst, f)
    }
}
impl DeepCopy<drm_i915_gem_context_create_ext> for drm_i915_gem_context_create_ext {
    fn alloc(&mut self, source: &drm_i915_gem_context_create_ext) -> Result<(), String> {
        //info!("drm_i915_gem_context_create_ext: extensions:{:?}", source.extensions);
        self.extensions = 0;
        self.iterator(source, |src: u64, dst: &mut u64| -> Result<(), String> {
            let ext_src = unsafe { &*(src as *const i915_user_extension) };
            let size = Self::sizeof(ext_src.name)?;
            *dst = alloc(size)? as u64;
            unsafe { &mut *(*dst as *mut i915_user_extension) }.init();
            if ext_src.name == 0 {
                let param_src = unsafe { &*(src as *const drm_i915_gem_context_create_ext_setparam) };
                let param_dst =
                    unsafe { &mut *(*dst as *mut drm_i915_gem_context_create_ext_setparam) };
                param_dst.alloc(param_src)?;
            }
            Ok(())
        })
    }
    fn copy(&mut self, source: &drm_i915_gem_context_create_ext, _: Direction) -> Result<(), String> {
        self.ctx_id = source.ctx_id;
        self.flags = source.flags;
        // Extensions
        self.iterator(source, |src: u64, dst: &mut u64| -> Result<(), String> {
            let ext_src = unsafe { &*(src as *const i915_user_extension) };
            let ext_dst = unsafe { &mut *(*dst as *mut i915_user_extension) };
            ext_dst.copy(ext_src);
            if ext_src.name == 0 {
                // Deep copy extension
                let s = unsafe { &*(src as *mut drm_i915_gem_context_create_ext_setparam) };
                let d =
                    unsafe { &mut *(*dst as *mut drm_i915_gem_context_create_ext_setparam) };
                d.copy(s)?;
            }
            Ok(())
        })
    }
    fn free(&mut self) -> Result<(), String> {
        let mut ext = self.extensions;
        while ext != 0 {
            let base = unsafe { &mut *(ext as *mut i915_user_extension) };
            // Free other sub-items here.
            if base.name == 0 {
                let param = unsafe { &mut *(ext as *mut drm_i915_gem_context_create_ext_setparam) };
                param.free()?;
            }
            let next = base.next_extension;
            let size = Self::sizeof(base.name)?;
            free(ext as *mut u8, size)?;
            ext = next;
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
        f: fn(u64, &mut u64) -> Result<(), String>,
    ) -> Result<(), String> {
        let ext_dst = &mut self.extensions;
        let ext_src = source.extensions;
        iterator::<_>(ext_src, ext_dst, f)
    }
}
impl DeepCopy<drm_i915_gem_vm_control> for drm_i915_gem_vm_control {
    fn alloc(&mut self, source: &drm_i915_gem_vm_control) -> Result<(), String> {
        self.extensions = 0;
        self.iterator(source, |src: u64, dst: &mut u64| -> Result<(), String> {
            let ext_src = unsafe { &*(src as *const i915_user_extension) };
            let size = Self::sizeof(ext_src.name & PRELIM_I915_USER_EXT_MASK)?;
            *dst = alloc(size)? as u64;
            unsafe { &mut *(*dst as *mut i915_user_extension) }.init();
            Ok(())
        })
    }
    fn copy(&mut self, source: &drm_i915_gem_vm_control, _: Direction) -> Result<(), String> {
        self.vm_id = source.vm_id;
        self.flags = source.flags;
        // Deep copy
        self.iterator(source, |src: u64, dst: &mut u64| -> Result<(), String> {
            let ext_src = unsafe { &*(src as *const i915_user_extension) };
            let next = unsafe { &*(*dst as *const i915_user_extension) }.next_extension;
            let size = Self::sizeof(ext_src.name & PRELIM_I915_USER_EXT_MASK)?;
            unsafe {
                ptr::copy(src as *const u8, *dst as *mut u8, size);
            }
            unsafe { &mut *(*dst as *mut i915_user_extension) }.next_extension = next;
            /*let ext_src = unsafe { &*(src as *const i915_user_extension) };
            let ext_dst = unsafe { &mut *(*dst as *mut i915_user_extension) };
            ext_dst.copy(ext_src);
            if ext_src.name & PRELIM_I915_USER_EXT_MASK == 0 {
                let src = unsafe { &*(src as *const prelim_drm_i915_gem_vm_region_ext) };
                let dst =
                    unsafe { &mut *(*dst as *mut prelim_drm_i915_gem_vm_region_ext) };
                dst.copy(src);
            }*/
            Ok(())
        })
    }
    fn free(&mut self) -> Result<(), String> {
        let mut ext = self.extensions;
        while ext != 0 {
            let extension = unsafe { &*(ext as *const i915_user_extension) };
            let size = Self::sizeof(extension.name & PRELIM_I915_USER_EXT_MASK)?;
            let next = extension.next_extension;
            free(ext as *mut u8, size)?;
            ext = next;
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
        } else {
            self.buffers_ptr = crate::memory::PTR_NULL;
        }
        if source.num_cliprects > 0 {
            let size = mem::size_of::<drm_i915_gem_exec_fence>()
                .checked_mul(source.num_cliprects as usize)
                .ok_or(format!("mul error"))?;
            self.cliprects_ptr = alloc(size)? as u64;
        } else {
            self.cliprects_ptr = crate::memory::PTR_NULL;
        }
        Ok(())
    }
    fn copy(&mut self, source: &drm_i915_gem_execbuffer2, _: Direction) -> Result<(), String> {
        self.batch_start_offset = source.batch_start_offset;
        self.batch_len = source.batch_len;
        self.dr1 = source.dr1;
        self.dr4 = source.dr4;
        self.flags = source.flags;
        self.rsvd1 = source.rsvd1;
        self.rsvd2 = source.rsvd2;
        // Deep copy
        self.buffer_count = source.buffer_count;
        if source.buffer_count > 0 {
            let size = mem::size_of::<drm_i915_gem_exec_object2>()
                .checked_mul(source.buffer_count as usize)
                .ok_or(format!("mul error"))?;
            unsafe {
                ptr::copy(
                    source.buffers_ptr as *const u8,
                    self.buffers_ptr as *mut u8,
                    size,
                );
            }
        }
        self.num_cliprects = source.num_cliprects;
        if source.num_cliprects > 0 {
            let size = mem::size_of::<drm_i915_gem_exec_fence>()
                .checked_mul(source.num_cliprects as usize)
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
    fn alloc(&mut self, source: &drm_i915_gem_pread) -> Result<(), String> {
        self.data_ptr = alloc(source.size as usize)? as u64;
        Ok(())
    }
    fn copy(&mut self, source: &drm_i915_gem_pread, _: Direction) -> Result<(), String> {
        self.handle = source.handle;
        self.pad = source.pad;
        self.offset = source.offset;
        self.size = source.size;
        // Deep copy
        if source.size > 0 {
            unsafe {
                ptr::copy(
                    source.data_ptr as *mut u8,
                    self.data_ptr as *mut u8,
                    source.size as usize,
                );
            }
        }
        Ok(())
    }
    fn free(&mut self) -> Result<(), String> {
        free(self.data_ptr as *mut u8, self.size as usize)
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
        self.data_ptr = alloc(source.size as usize)? as u64;
        Ok(())
    }
    fn copy(&mut self, source: &drm_i915_gem_pwrite, _: Direction) -> Result<(), String> {
        self.handle = source.handle;
        self.pad = source.pad;
        self.offset = source.offset;
        self.size = source.size;
        // Deep copy
        if source.size > 0 {
            unsafe {
                ptr::copy(
                    source.data_ptr as *mut u8, 
                    self.data_ptr as *mut u8, 
                    source.size as usize
                );
            }
        }
        Ok(())
    }
    fn free(&mut self) -> Result<(), String> {
        free(self.data_ptr as *mut u8, self.size as usize)
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
    //Ok(ioctl(_fd, cmd, _arg))
}

#[no_mangle]
pub fn pxp_ioctl(fd: i32, cmd: u32, arg: *const u8) -> i32 {
    //info!("PXP cmd: {:?} Enter", &cmd);
    let ret = match cmd {
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
        DRM_IOCTL_I915_GEM_VM_DESTROY => exec2::<drm_i915_gem_vm_control>(fd, &cmd, arg),
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
    .unwrap();
    //info!("PXP cmd: {:?} Exit", &cmd);
    ret
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
