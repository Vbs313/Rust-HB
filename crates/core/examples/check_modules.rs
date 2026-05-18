/// Check if version.dll or BepInEx is loaded in Hearthstone
use hb_core::win32::ProcessHandle;

fn main() {
    let pid: u32 = 19624;
    match ProcessHandle::find_by_pid(pid) {
        Ok(p) => {
            println!("✅ Attached to PID {}", p.pid);

            // Try to snapshot modules
            unsafe {
                let snapshot = CreateToolhelp32Snapshot(0x00000008, pid); // TH32CS_SNAPMODULE
                if snapshot as isize == -1 as isize {
                    println!("❌ CreateToolhelp32Snapshot failed");
                    return;
                }

                let mut me: MODULEENTRY32W = std::mem::zeroed();
                me.dwSize = std::mem::size_of::<MODULEENTRY32W>() as u32;

                if Module32FirstW(snapshot, &mut me) != 0 {
                    let mut found = 0u32;
                    loop {
                        let name = String::from_utf16_lossy(&me.szModule)
                            .trim_matches('\0')
                            .to_lowercase();

                        if name.contains("version")
                            || name.contains("bepinex")
                            || name.contains("rustipc")
                        {
                            println!("  ✅ {} loaded!", name);
                            found += 1;
                        }

                        if Module32NextW(snapshot, &mut me) == 0 {
                            break;
                        }
                        found += 0;
                    }
                    if found == 0 {
                        println!("  No BepInEx/Doorstop modules found");
                    }
                }
                CloseHandle(snapshot);
            }
        }
        Err(e) => println!("❌ find_by_pid: {e}"),
    }

    #[repr(C)]
    struct MODULEENTRY32W {
        dwSize: u32,
        th32ModuleID: u32,
        th32ProcessID: u32,
        GlblcntUsage: u32,
        ProccntUsage: u32,
        modBaseAddr: *mut u8,
        modBaseSize: u32,
        hModule: *mut u8,
        szModule: [u16; 256],
        szExePath: [u16; 260],
    }

    extern "system" {
        fn CreateToolhelp32Snapshot(dwFlags: u32, th32ProcessID: u32) -> *mut u8;
        fn Module32FirstW(hSnapshot: *mut u8, lpme: *mut MODULEENTRY32W) -> i32;
        fn Module32NextW(hSnapshot: *mut u8, lpme: *mut MODULEENTRY32W) -> i32;
        fn CloseHandle(hObject: *mut u8) -> i32;
    }
}
