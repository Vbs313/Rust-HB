fn main() {
    let pid = 14916u32;

    // Try HB-core's find_by_name
    for name in &["ysenatmhyn", "Hearthbuddy", "vsezrrppcj", "Hearthstone"] {
        match hb_core::win32::ProcessHandle::find_by_name(name) {
            Ok(p) => println!("✅ {}: PID={}", name, p.pid),
            Err(e) => println!("❌ {}: {}", name, e),
        }
    }

    // Try find_by_pid
    println!("\nTrying find_by_pid({})...", pid);
    match hb_core::win32::ProcessHandle::find_by_pid(pid) {
        Ok(p) => println!("✅ PID {} opened", p.pid),
        Err(e) => println!("❌ PID {}: {}", pid, e),
    }
}
