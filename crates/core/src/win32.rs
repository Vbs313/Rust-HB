//! Win32 API 公共绑定
//!
//! 直接使用 FFI 调用 kernel32.dll（避免 windows crate 版本兼容问题）
//! 覆盖: 进程管理、内存读写、快照遍历

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::RawHandle;

// ============================================================
// Win32 API 类型定义
// ============================================================

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PROCESSENTRY32W {
    pub dwSize: u32,
    pub cntUsage: u32,
    pub th32ProcessID: u32,
    pub th32DefaultHeapID: usize,
    pub th32ModuleID: u32,
    pub cntThreads: u32,
    pub th32ParentProcessID: u32,
    pub pcPriClassBase: i32,
    pub dwFlags: u32,
    pub szExeFile: [u16; 260],
}

impl Default for PROCESSENTRY32W {
    fn default() -> Self {
        Self {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            cntUsage: 0,
            th32ProcessID: 0,
            th32DefaultHeapID: 0,
            th32ModuleID: 0,
            cntThreads: 0,
            th32ParentProcessID: 0,
            pcPriClassBase: 0,
            dwFlags: 0,
            szExeFile: [0u16; 260],
        }
    }
}

#[repr(C)]
pub struct MEMORY_BASIC_INFORMATION {
    pub BaseAddress: *mut std::ffi::c_void,
    pub AllocationBase: *mut std::ffi::c_void,
    pub AllocationProtect: u32,
    pub RegionSize: usize,
    pub State: u32,
    pub Protect: u32,
    pub Type: u32,
}

// ============================================================
// FFI 函数声明
// ============================================================

#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(dwDesiredAccess: u32, bInheritHandle: i32, dwProcessId: u32) -> RawHandle;

    fn CloseHandle(hObject: RawHandle) -> i32;

    fn ReadProcessMemory(
        hProcess: RawHandle,
        lpBaseAddress: *const std::ffi::c_void,
        lpBuffer: *mut std::ffi::c_void,
        nSize: usize,
        lpNumberOfBytesRead: *mut usize,
    ) -> i32;

    fn WriteProcessMemory(
        hProcess: RawHandle,
        lpBaseAddress: *const std::ffi::c_void,
        lpBuffer: *const std::ffi::c_void,
        nSize: usize,
        lpNumberOfBytesWritten: *mut usize,
    ) -> i32;

    fn CreateToolhelp32Snapshot(dwFlags: u32, th32ProcessID: u32) -> RawHandle;

    fn Process32FirstW(hSnapshot: RawHandle, lppe: *mut PROCESSENTRY32W) -> i32;

    fn Process32NextW(hSnapshot: RawHandle, lppe: *mut PROCESSENTRY32W) -> i32;

    fn VirtualQueryEx(
        hProcess: RawHandle,
        lpAddress: *const std::ffi::c_void,
        lpBuffer: *mut MEMORY_BASIC_INFORMATION,
        dwLength: usize,
    ) -> usize;
}

const PROCESS_VM_READ: u32 = 0x0010;
const PROCESS_VM_WRITE: u32 = 0x0020;
const PROCESS_VM_OPERATION: u32 = 0x0008;
const PROCESS_QUERY_INFORMATION: u32 = 0x0400;
const TH32CS_SNAPPROCESS: u32 = 0x0002;

// ============================================================
// 进程句柄封装
// ============================================================

/// 进程句柄封装
#[derive(Debug)]
pub struct ProcessHandle {
    pub pid: u32,
    pub handle: RawHandle,
}

unsafe impl Send for ProcessHandle {}
unsafe impl Sync for ProcessHandle {}

impl ProcessHandle {
    /// 按进程名查找并打开进程
    pub fn find_by_name(name: &str) -> Result<Self, crate::error::Error> {
        let pid = find_process_id(name)
            .ok_or_else(|| crate::error::Error::ProcessNotFound(name.to_string()))?;

        let handle = unsafe {
            OpenProcess(
                PROCESS_VM_READ
                    | PROCESS_VM_WRITE
                    | PROCESS_VM_OPERATION
                    | PROCESS_QUERY_INFORMATION,
                0,
                pid,
            )
        };

        if handle.is_null() {
            return Err(crate::error::Error::Runtime(format!(
                "OpenProcess failed for PID {pid}"
            )));
        }

        Ok(Self { pid, handle })
    }

    /// 读取进程内存（泛型）
    pub fn read_memory<T>(&self, address: usize) -> Result<T, crate::error::Error> {
        let mut buffer: T = unsafe { std::mem::zeroed() };
        let size = std::mem::size_of::<T>();
        let mut bytes_read: usize = 0;

        let success = unsafe {
            ReadProcessMemory(
                self.handle,
                address as *const _,
                &mut buffer as *mut T as *mut _,
                size,
                &mut bytes_read,
            )
        };

        if success == 0 || bytes_read != size {
            return Err(crate::error::Error::Memory(format!(
                "ReadProcessMemory failed at 0x{address:x}, read {bytes_read}/{size} bytes"
            )));
        }

        Ok(buffer)
    }

    /// 读取内存到字节缓冲区
    pub fn read_bytes(&self, address: usize, size: usize) -> Result<Vec<u8>, crate::error::Error> {
        let mut buffer = vec![0u8; size];
        let mut bytes_read: usize = 0;

        let success = unsafe {
            ReadProcessMemory(
                self.handle,
                address as *const _,
                buffer.as_mut_ptr() as *mut _,
                size,
                &mut bytes_read,
            )
        };

        if success == 0 {
            return Err(crate::error::Error::Memory(format!(
                "ReadProcessMemory failed at 0x{address:x}"
            )));
        }

        buffer.truncate(bytes_read);
        Ok(buffer)
    }

    /// 写入进程内存
    pub fn write_memory<T>(&self, address: usize, value: &T) -> Result<(), crate::error::Error> {
        let size = std::mem::size_of::<T>();
        let mut bytes_written: usize = 0;

        let success = unsafe {
            WriteProcessMemory(
                self.handle,
                address as *const _,
                value as *const T as *const _,
                size,
                &mut bytes_written,
            )
        };

        if success == 0 || bytes_written != size {
            return Err(crate::error::Error::Memory(format!(
                "WriteProcessMemory failed at 0x{address:x}"
            )));
        }

        Ok(())
    }

    /// 获取模块基址
    pub fn get_module_base(&self, _module_name: &str) -> Option<usize> {
        // TODO: 通过 PEB 遍历模块列表
        None
    }

    /// 复制进程句柄（通过再次 OpenProcess）
    pub fn duplicate(&self) -> Result<Self, crate::error::Error> {
        Self::find_by_pid(self.pid)
    }

    /// 通过 PID 打开进程
    pub fn find_by_pid(pid: u32) -> Result<Self, crate::error::Error> {
        let handle = unsafe {
            OpenProcess(
                PROCESS_VM_READ
                    | PROCESS_VM_WRITE
                    | PROCESS_VM_OPERATION
                    | PROCESS_QUERY_INFORMATION,
                0,
                pid,
            )
        };
        if handle.is_null() {
            return Err(crate::error::Error::Runtime(format!(
                "OpenProcess failed for PID {pid}"
            )));
        }
        Ok(Self { pid, handle })
    }
}

impl ProcessHandle {
    /// 创建空的测试实例（无需真实进程）
    #[doc(hidden)]
    pub fn dummy() -> Self {
        Self {
            pid: 0,
            handle: std::ptr::null_mut(),
        }
    }
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }
}

// ============================================================
// 辅助函数
// ============================================================

/// 按进程名查找进程 ID
fn find_process_id(name: &str) -> Option<u32> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot.is_null() {
        return None;
    }

    let mut entry = PROCESSENTRY32W::default();
    if unsafe { Process32FirstW(snapshot, &mut entry) } == 0 {
        unsafe { CloseHandle(snapshot) };
        return None;
    }

    let target_name = name.to_lowercase();
    let target_name_exe = format!("{name}.exe").to_lowercase();

    loop {
        let exe_name = String::from_utf16_lossy(&entry.szExeFile)
            .trim_matches('\0')
            .to_lowercase();

        if exe_name == target_name || exe_name == target_name_exe {
            let pid = entry.th32ProcessID;
            unsafe { CloseHandle(snapshot) };
            return Some(pid);
        }

        if unsafe { Process32NextW(snapshot, &mut entry) } == 0 {
            break;
        }
    }

    unsafe { CloseHandle(snapshot) };
    None
}

/// 查询内存区域信息
pub fn query_memory_info(
    handle: &ProcessHandle,
    address: usize,
) -> Result<MEMORY_BASIC_INFORMATION, crate::error::Error> {
    let mut mbi: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<MEMORY_BASIC_INFORMATION>();

    let result = unsafe { VirtualQueryEx(handle.handle, address as *const _, &mut mbi, size) };

    if result == 0 {
        return Err(crate::error::Error::Memory(format!(
            "VirtualQueryEx failed at 0x{address:x}"
        )));
    }

    Ok(mbi)
}

// ============================================================
// 字符串转换辅助
// ============================================================

/// 将 Rust 字符串转为以 null 结尾的 UTF-16 缓冲区
pub fn to_utf16(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_process_nonexistent() {
        let pid = find_process_id("THIS_PROCESS_SHOULD_NOT_EXIST_12345");
        assert!(pid.is_none());
    }

    #[test]
    fn test_to_utf16() {
        let buf = to_utf16("test");
        assert_eq!(
            buf,
            &[b't' as u16, b'e' as u16, b's' as u16, b't' as u16, 0]
        );
    }
}
