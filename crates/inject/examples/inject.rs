//! Hearthstone DLL Injector
//! Injects hs_ipc_server.dll via CreateRemoteThread + LoadLibraryA

#![allow(non_snake_case)]

use std::ffi::CString;
use std::ptr;

fn main() {
    println!("=== Hearthstone DLL Injector ===\n");

    let pid = match find_hs_pid() {
        Some(p) => {
            println!("✅ Hearthstone PID={}", p);
            p
        }
        None => {
            println!("❌ Hearthstone not running");
            return;
        }
    };

    let dll_path =
        std::path::PathBuf::from("E:/Next/target/i686-pc-windows-msvc/debug/hs_ipc_server.dll");

    if !dll_path.exists() {
        println!("❌ DLL not found: {:?}", dll_path);
        return;
    }
    println!("✅ DLL: {:?}", dll_path);

    let process = unsafe {
        // Open with VM_READ | VM_WRITE | VM_OPERATION | QUERY_INFO | CREATE_THREAD
        let h = OpenProcess(
            0x0010 | 0x0020 | 0x0008 | 0x0400 | 0x0002, // VM_READ|VM_WRITE|VM_OP|QUERY|CREATE_THREAD
            0,
            pid,
        );
        if h.is_null() {
            let err = std::io::Error::last_os_error();
            println!("❌ OpenProcess: {err}");
            return;
        }
        println!("✅ Process opened");
        h
    };

    // Write DLL path into remote process
    let dll_cstr = CString::new(dll_path.to_str().unwrap()).unwrap();
    let dll_bytes = dll_cstr.as_bytes_with_nul();

    let remote_mem = unsafe {
        let addr = VirtualAllocEx(
            process,
            ptr::null_mut(),
            dll_bytes.len() as u32,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        );
        if addr.is_null() {
            let err = std::io::Error::last_os_error();
            println!("❌ VirtualAllocEx: {err}");
            CloseHandle(process);
            return;
        }
        let mut written = 0u32;
        let ok = WriteProcessMemory(
            process,
            addr,
            dll_bytes.as_ptr(),
            dll_bytes.len() as u32,
            &mut written,
        );
        if ok == 0 {
            let err = std::io::Error::last_os_error();
            println!("❌ WriteProcessMemory: {err}");
            VirtualFreeEx(process, addr, 0, MEM_RELEASE);
            CloseHandle(process);
            return;
        }
        println!("✅ DLL path written @ 0x{:x}", addr as usize);
        addr
    };

    // Get LoadLibraryA address
    let loadlib = unsafe {
        let kernel32 =
            GetModuleHandleA(CString::new("kernel32.dll").unwrap().as_ptr() as *const i8);
        if kernel32.is_null() {
            println!("❌ No kernel32");
            cleanup(process, remote_mem);
            return;
        }
        let addr = GetProcAddress(
            kernel32,
            CString::new("LoadLibraryA").unwrap().as_ptr() as *const i8,
        );
        if addr.is_null() {
            println!("❌ No LoadLibraryA");
            cleanup(process, remote_mem);
            return;
        }
        println!("✅ LoadLibraryA @ 0x{:x}", addr as usize);
        addr as usize
    };

    // Create remote thread: LoadLibraryA(dll_path)
    unsafe {
        let func: extern "system" fn(*mut u8) -> u32 = std::mem::transmute(loadlib);
        let thread = CreateRemoteThread(
            process,
            ptr::null_mut(),
            0,
            Some(func),
            remote_mem,
            0,
            ptr::null_mut(),
        );

        if thread.is_null() {
            let err = std::io::Error::last_os_error();
            println!("❌ CreateRemoteThread: {err}");
            cleanup(process, remote_mem);
            return;
        }

        println!("✅ Remote thread created, waiting up to 10s...");

        let wait = WaitForSingleObject(thread, 10000);
        if wait == 0 {
            let mut exit = 0u32;
            GetExitCodeThread(thread, &mut exit);
            if exit != 0 {
                println!("✅ DLL loaded! (LoadLibraryA returned 0x{exit:x})");
                println!("\n   Now run: cargo run --target i686-pc-windows-msvc -p hb-app\n");
            } else {
                println!("❌ LoadLibraryA returned NULL");
            }
        } else {
            println!("⚠️  Thread still running (wait={wait}) - may need more time");
            println!("   Try connecting anyway: cargo run --target i686-pc-windows-msvc -p hb-app");
        }

        CloseHandle(thread);
    }

    cleanup(process, remote_mem);
}

fn cleanup(process: *mut u8, mem: *mut u8) {
    unsafe {
        if !mem.is_null() {
            VirtualFreeEx(process, mem, 0, MEM_RELEASE);
        }
        if !process.is_null() {
            CloseHandle(process);
        }
    }
}

fn find_hs_pid() -> Option<u32> {
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap as isize == -1 {
            return None;
        }

        let mut e = PROCESSENTRY32W::new();
        let mut pid = None;

        if Process32FirstW(snap, &mut e) != 0 {
            loop {
                let name = String::from_utf16_lossy(&e.szExeFile)
                    .trim_matches('\0')
                    .to_lowercase();
                if name == "hearthstone.exe" {
                    pid = Some(e.th32ProcessID);
                    break;
                }
                if Process32NextW(snap, &mut e) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snap);
        pid
    }
}

// Win32 types
const MEM_COMMIT: u32 = 0x1000;
const MEM_RESERVE: u32 = 0x2000;
const MEM_RELEASE: u32 = 0x8000;
const PAGE_READWRITE: u32 = 0x04;
const TH32CS_SNAPPROCESS: u32 = 0x00000002;

#[repr(C)]
struct PROCESSENTRY32W {
    dwSize: u32,
    cntUsage: u32,
    th32ProcessID: u32,
    th32DefaultHeapID: usize,
    th32ModuleID: u32,
    cntThreads: u32,
    th32ParentProcessID: u32,
    pcPriClassBase: i32,
    dwFlags: u32,
    szExeFile: [u16; 260],
}
impl PROCESSENTRY32W {
    fn new() -> Self {
        let mut s: Self = unsafe { std::mem::zeroed() };
        s.dwSize = std::mem::size_of::<Self>() as u32;
        s
    }
}

extern "system" {
    fn OpenProcess(dwDesiredAccess: u32, bInheritHandle: i32, dwProcessId: u32) -> *mut u8;
    fn CloseHandle(hObject: *mut u8) -> i32;
    fn CreateRemoteThread(
        hProcess: *mut u8,
        sec: *mut u8,
        stack: u32,
        start: Option<extern "system" fn(*mut u8) -> u32>,
        param: *mut u8,
        flags: u32,
        tid: *mut u32,
    ) -> *mut u8;
    fn VirtualAllocEx(h: *mut u8, addr: *mut u8, size: u32, alloc: u32, prot: u32) -> *mut u8;
    fn VirtualFreeEx(h: *mut u8, addr: *mut u8, size: u32, free: u32) -> i32;
    fn WriteProcessMemory(
        h: *mut u8,
        addr: *mut u8,
        buf: *const u8,
        size: u32,
        written: *mut u32,
    ) -> i32;
    fn WaitForSingleObject(h: *mut u8, ms: u32) -> u32;
    fn GetExitCodeThread(h: *mut u8, code: *mut u32) -> i32;
    fn GetModuleHandleA(name: *const i8) -> *mut u8;
    fn GetProcAddress(module: *mut u8, name: *const i8) -> *mut u8;
    fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> *mut u8;
    fn Process32FirstW(snap: *mut u8, entry: *mut PROCESSENTRY32W) -> i32;
    fn Process32NextW(snap: *mut u8, entry: *mut PROCESSENTRY32W) -> i32;
}
