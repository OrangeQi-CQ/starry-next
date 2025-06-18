use core::{
    ffi::{c_char, c_int}, panic
};

use alloc::{collections::btree_set::BTreeSet, format, string::String};
use alloc::vec;
use alloc::vec::Vec;
use alloc::collections::btree_map::BTreeMap;

use axerrno::{LinuxError, LinuxResult};
use axtask::{current, TaskExtRef};
use alloc::sync::Arc;
use axsync::Mutex;
use axerrno::AxError;
use axprocess::Pid;
use axhal::time::monotonic_time_nanos;
use axfs::fops::OpenOptions;

use lazy_static::lazy_static;
use linux_raw_sys::ctypes::{c_int, c_long, c_ushort};
use memory_addr::{PhysAddr, VirtAddr, VirtAddrRange, PAGE_SIZE_4K};
use page_table_entry::MappingFlags;
use page_table_multiarch::PageSize;
use linux_raw_sys::general::*;

use crate::{do_mmap, do_openat, ptr::{PtrWrapper, UserConstPtr, UserPtr}, sys_close, sys_ftruncate, sys_unlink, MmapFlags};
use crate::imp::ipc::IPCID_ALLOCATOR;
use crate::{sys_fstat, sys_mmap, sys_munmap, sys_open, sys_openat};
use crate::file::*;

bitflags::bitflags! {
    /// flags for sys_shmat
    #[derive(Debug)]
    struct ShmAtFlags: u32 {
        /* attach read-only else read-write */
        const SHM_RDONLY = 0o10000;
        /* round attach address to SHMLBA */
        const SHM_RND = 0o20000;
        /* take-over region on attach */
        const SHM_REMAP = 0o40000;
    }
}

/// flags for sys_shmget, sys_msgget, sys_semget
pub const IPC_PRIVATE: i32 = 0;
pub const IPC_RMID: u32 = 0;
pub const IPC_SET: u32 = 1;
pub const IPC_STAT: u32 = 2;

/// Data structure used to pass permission information to IPC operations.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct IpcPerm {
    pub key: __kernel_key_t,
    pub uid: __kernel_uid_t,                
    pub gid: __kernel_gid_t,         
    pub cuid: __kernel_uid_t,
    pub cgid: __kernel_gid_t, 
    pub mode: __kernel_mode_t, 
    pub seq: c_ushort,
    pad: c_ushort,      // for memory align
    unused0: c_long,    // for memory align
    unused1: c_long,    // for memory align
}

/// Data structure describing a shared memory segment.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ShmidDs {
    pub shm_perm: IpcPerm,  /* operation permission struct */
    pub shm_segsz: __kernel_size_t,   /* size of segment in bytes */
    pub shm_atime: __kernel_time_t,     /* time of last shmat() */
    pub shm_dtime: __kernel_time_t,     /* time of last shmdt() */
    pub shm_ctime: __kernel_time_t,     /* time of last change by shmctl() */
    pub shm_cpid: __kernel_pid_t,      /* pid of creator */
    pub shm_lpid: __kernel_pid_t,      /* pid of last shmop */
    pub shm_nattch: c_ushort,    /* number of current attaches */
}

impl ShmidDs {
    fn new(key: i32, size: usize, mode: __kernel_mode_t, pid: __kernel_pid_t) -> Self {
        Self {
            shm_perm: IpcPerm {
                key,
                uid: 0,
                gid: 0,
                cuid: 0,
                cgid: 0,
                mode,
                seq: 0,
                pad: 0,
                unused0: 0,
                unused1: 0,
            },
            shm_segsz: size as __kernel_size_t,
            shm_atime: 0,
            shm_dtime: 0,
            shm_ctime: 0,
            shm_cpid: pid,
            shm_lpid: pid,
            shm_nattch: 0,
        }
    }

    fn check_mode(&self, mode: usize) -> bool {

    }
}

fn shmpath_shmid(path: String) -> i32 {
    0
}

fn shmid_shmpath(shmid: i32) -> str {
    ""
}

struct ShmManager {
    next_id: ShmId,
    shmkey_shmid: BTreeMap<ShmKey, ShmId>,
    shmid_shmds: BTreeMap<ShmId, ShmidDs>,
}

type ShmKey = i32;
type ShmId = i32;

impl ShmManager {
    fn alloc_shmid(&mut self, key: ShmKey, size: usize, mode: usize, pid: u32) -> ShmId {
        let shmid = self.next_id;
        self.next_id += 1;
        self.shmkey_shmid.insert(key, shmid);
        self.shmid_shmds.insert(key, ShmidDs::new(key, size, mode, pid));
        return shmid;
    }

    fn key2shmid(&self, key: i32) -> Option<i32> {

    }

    fn get_shmds(&self, shmid: i32) -> &ShmidDs {
        
    }

    fn shmat(&self, shmid: i32) {

    }
    fn shmdt(&self, shmid: i32) {

    }
}

lazy_static! {
    static ref SHM_MANAGER: Mutex<ShmManager> = Mutex::new(ShmManager::new());
}

/// 创建共享内存文件，设置文件大小
pub fn sys_shmget(key: ShmKey, size: usize, shmflg: usize) -> LinuxResult<isize> {
    if size == 0 {
        return Err(LinuxError::EINVAL);
    }

    let cur_pid = current().task_ext().thread.process().pid();
    let mode = shmflg2mode(shmflg);
    let mut shm_manager = SHM_MANAGER.lock();
    
    if key != IPC_PRIVATE {
        // This process has already created a shared memory segment with the same key
        if let Some(shmid) = shm_manager.key2shmid(key) {
            let shmds = shm_manager.get_shmds(shmid);
            if shmds.check_mode(mode) {
                return Ok(shmid as isize);
            } else {
                return Err(LinuxError::EINVAL);
            }
        }
    }

    let shmid = shm_manager.alloc_shmid(key, size, mode, cur_pid);
    let path = shmid_shmpath(shmid);
    let fd = do_openat(-1, &path, opts)?;
    sys_ftruncate(fd, size)?;
    Ok(shmid)
}

// 打开共享内存文件并 mmap
pub fn sys_shmat(shmid: i32, addr: usize, shmflg: u32) -> LinuxResult<isize> {
    let mut shm_manager = SHM_MANAGER.lock();
    let path = shmid_shmpath(shmid);
    let shmds = shm_manager.get_shmds(shmid);
    let length = shmds.shm_segsz as usize;
    let prot = {
        let shmflg = ShmAtFlags::from_bits(shmflg);
        if shmflg.contains(ShmAtFlags::SHM_RDONLY) {
            PROT_READ
        } else {
            PROT_READ | PROT_WRITE
        }
    };
    let flags = MAP_SHARED;
    
    shm_manager.shmat(shmid);
    let fd = do_openat(-1, &path, opts);
    sys_mmap(addr, length, prot, flags, fd, 0)
}

// 关闭共享内存文件并 munmap
pub fn sys_shmdt(addr: usize) -> LinuxResult<isize> {
    let mut shm_manager = SHM_MANAGER.lock();
    let curr = current();
    let vma_manager = curr.task_ext().process_data().vma_mnager();
    let vma = vma_manager.query(VirtAddr::from(addr));

    if vma.is_none() {
        error!("Invalid shmdt addr!");
        return Err(LinuxError::EINVAL);
    }

    let vma = vma.unwrap();
    let file = File::from_fd(fd)?;
    let path = file.path();
    let shmid = shmpath_shmid(&path);
    sys_close(vma.fd);
    let res = sys_munmap(addr, vma.unwrap().length);
    shm_manager.shmdt(shmid);
    res
}

pub fn sys_shmctl(shmid: i32, cmd: u32, buf: UserPtr<ShmidDs>) -> LinuxResult<isize> {

}

pub fn sys_shm_open(
    path: UserConstPtr<c_char>,
    flags: i32,
    mode: __kernel_mode_t,
) -> LinuxResult<isize> {
    let path = path.get_as_str()?;
    if !path.starts_with('/') {
        error!("Shm_open path must start with '/'");
        return Err(LinuxError::EINVAL);
    }    
    let shm_path_name = format!("{}{}", SHM_PATH_PREFIX_POSIX, path);
    let ptr = shm_path_name.as_ptr();
    let path = UserConstPtr::<c_char>::from(ptr as usize);
    sys_open(path, flags, mode)
}

pub fn sys_shm_unlink(path: UserConstPtr<c_char>) -> LinuxResult<isize> {
    let path = path.get_as_str()?;
    if !path.starts_with('/') {
        error!("Shm_open path must start with '/'");
        return Err(LinuxError::EINVAL);
    }    

    let shm_path_name = format!("{}{}", SHM_PATH_PREFIX_POSIX, path);
    let ptr = shm_path_name.as_ptr();
    let path = UserConstPtr::<c_char>::from(ptr as usize);
    sys_unlink(path)
}