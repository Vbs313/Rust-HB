/// Test different OpenProcess access masks against ysenatmhyn
fn main() {
    let pid: u32 = 14916;
    let masks: &[(u32, &str)] = &[
        (0x0010, "PROCESS_VM_READ"),
        (0x0400, "PROCESS_QUERY_INFORMATION"),
        (0x0010 | 0x0400, "READ|QUERY"),
        (0x0008, "PROCESS_VM_OPERATION"),
        (0x0010 | 0x0008 | 0x0400, "READ|OP|QUERY"),
        (0x0010 | 0x0020 | 0x0008 | 0x0400, "ALL (standard)"),
        (0x1FFFFF, "PROCESS_ALL_ACCESS"),
    ];

    for &(mask, name) in masks {
        unsafe {
            let handle = OpenProcess(mask, 0, pid);
            if !handle.is_null() {
                println!("✅ {name}: handle={:p}", handle);
                CloseHandle(handle);
            } else {
                let err = std::io::Error::last_os_error();
                println!("❌ {name}: {err}",);
            }
        }
    }

    // Also test Hearthstone for comparison
    println!("\nComparing with Hearthstone (PID 3328):");
    let hs_pid: u32 = 3328;
    unsafe {
        let handle = OpenProcess(0x0010 | 0x0400, 0, hs_pid);
        if !handle.is_null() {
            println!("✅ Hearthstone READ|QUERY: handle={:p}", handle);
            CloseHandle(handle);
        } else {
            let err = std::io::Error::last_os_error();
            println!("❌ Hearthstone: {err}");
        }
    }

    extern "system" {
        fn OpenProcess(dwDesiredAccess: u32, bInheritHandle: i32, dwProcessId: u32) -> *mut u8;
        fn CloseHandle(hObject: *mut u8) -> i32;
    }
}
