//! 诊断: 为什么扫描不到 MonoImage

use hb_core::win32::query_memory_info;
use hb_core::win32::ProcessHandle;

fn main() {
    println!("=== MonoImage 扫描诊断 ===\n");
    let p = match ProcessHandle::find_by_name("Hearthstone") {
        Ok(p) => {
            println!("✅ PID={}", p.pid);
            p
        }
        Err(e) => {
            println!("❌ {e}");
            return;
        }
    };

    // ===== 1. 验证已知 corlib 地址的签名 =====
    println!("[1] 验证 MonoImage 签名偏移...");

    // 从之前已知的 corlib 地址（每次游戏启动会变，所以得从 mono_get_corlib 获取）
    println!("  找 mono 模块...");
    let mono_base = find_mono_base(&p).unwrap_or(0);
    if mono_base == 0 {
        println!("  ❌ mono module not found");
        return;
    }
    println!("  mono @ 0x{mono_base:x}");

    if let Some(corlib_fn) = find_export_in_module(&p, mono_base, "mono_get_corlib") {
        if let Some(corlib) = call_remote(&p, corlib_fn) {
            println!("  corlib MonoImage @ 0x{corlib:x}");

            // 测试偏移 +0x1C
            if let Ok(np) = p.read_memory::<u32>(corlib + 0x1C) {
                if let Ok(b) = p.read_bytes(np as usize, 48) {
                    let name =
                        String::from_utf8_lossy(&b[..b.iter().position(|&x| x == 0).unwrap_or(48)]);
                    println!("  +0x1C name_ptr=0x{np:x} -> '{name}'");
                } else {
                    println!("  ❌ +0x1C 读取失败");
                }
            }
            if let Ok(fp) = p.read_memory::<u32>(corlib + 0x20) {
                if let Ok(b) = p.read_bytes(fp as usize, 48) {
                    let fname =
                        String::from_utf8_lossy(&b[..b.iter().position(|&x| x == 0).unwrap_or(48)]);
                    println!("  +0x20 file_ptr=0x{fp:x} -> '{fname}'");
                }
            }

            // 测试扫描逻辑：在 corlib 附近扫描能否找到它自己
            println!("\n[2] 在 corlib 附近测试扫描逻辑...");
            let scan_start = corlib.saturating_sub(0x1000);
            if let Ok(data) = p.read_bytes(scan_start, 0x2000) {
                let mut found = false;
                for i in (0..data.len().saturating_sub(0x28)).step_by(4) {
                    let np = u32::from_le_bytes([
                        data[i + 0x1C],
                        data[i + 0x1D],
                        data[i + 0x1E],
                        data[i + 0x1F],
                    ]) as usize;
                    if np == corlib + 0x1C {
                        // matches our corlib address
                        let fp = u32::from_le_bytes([
                            data[i + 0x20],
                            data[i + 0x21],
                            data[i + 0x22],
                            data[i + 0x23],
                        ]) as usize;
                        let abs_addr = scan_start + i;
                        println!(
                            "  ✅ 找到 corlib @ 0x{abs_addr:x} (i={i}, np=0x{np:x}, fp=0x{fp:x})"
                        );
                        found = true;
                        break;
                    }
                }
                if !found {
                    // 直接检查: corlib 的 +0x1C 是否指向自身地址
                    println!("  ⚠️ corlib 签名检查失败");
                    println!(
                        "  corlib+0x1C = 0x{:08x}",
                        match p.read_memory::<u32>(corlib + 0x1C) {
                            Ok(v) => v,
                            Err(_) => 0,
                        }
                    );
                    println!(
                        "  corlib+0x20 = 0x{:08x}",
                        match p.read_memory::<u32>(corlib + 0x20) {
                            Ok(v) => v,
                            Err(_) => 0,
                        }
                    );
                }
            }
        } else {
            println!("  ❌ mono_get_corlib failed");
        }
    } else {
        println!("  ❌ mono_get_corlib not found");
    }

    // ===== 3. 枚举所有内存区域 =====
    println!("\n[3] 内存区域统计:");
    let mut count = 0u32;
    let mut total = 0u64;
    let mut a: usize = 0x00010000;
    while a < 0x7FFFFFFF {
        if let Ok(mbi) = query_memory_info(&p, a) {
            let s = mbi.BaseAddress as usize;
            let sz = mbi.RegionSize;
            if mbi.State == 0x1000
                && ((mbi.Protect & 0x02) != 0 || (mbi.Protect & 0x04) != 0)
                && sz > 0
                && sz < 0x1000000
            {
                count += 1;
                total += sz as u64;
            }
            a = s + sz;
        } else {
            a += 0x10000;
        }
    }
    println!("  {count} regions, {:.1} MB", total as f64 / 1048576.0);

    // ===== 4. 快速扫描 =====
    println!("\n[4] MonoImage 全局扫描...");
    let images = hb_mono_bridge::scanner::find_all_images(&p);
    println!("  找到 {} 个", images.len());
    for img in &images {
        println!("  @ 0x{:08x} {} ({})", img.address, img.name, img.filename);
    }
}

fn find_mono_base(p: &ProcessHandle) -> Option<usize> {
    let mut a: usize = 0x01000000;
    // 通过特征 "mono-2.0" 找
    while a < 0x80000000 {
        if let Ok(mz) = p.read_memory::<u16>(a) {
            if mz == 0x5A4D {
                // 读导出表看有没有 mono 相关函数
                if let Some(_) = find_export_in_module(p, a, "mono_get_root_domain") {
                    return Some(a);
                }
            }
        }
        a += 0x10000;
    }
    None
}

fn find_export_in_module(p: &ProcessHandle, base: usize, name: &str) -> Option<usize> {
    let lfanew: i32 = p.read_memory(base + 0x3C).ok()?;
    let nt = base + lfanew as usize;
    let opt_magic: u16 = p.read_memory(nt + 24).ok()?;
    if opt_magic != 0x10B {
        return None;
    }
    let dd = nt + 24 + 0x60;
    let eva: u32 = p.read_memory(dd as usize).ok()?;
    if eva == 0 {
        return None;
    }
    let ea = base + eva as usize;
    let nn: u32 = p.read_memory(ea + 0x18).ok()?;
    let af: u32 = p.read_memory(ea + 0x1C).ok()?;
    let an: u32 = p.read_memory(ea + 0x20).ok()?;
    let ao: u32 = p.read_memory(ea + 0x24).ok()?;
    if nn == 0 {
        return None;
    }
    for i in 0..nn as usize {
        let rva: u32 = p.read_memory(base + an as usize + i * 4).ok()?;
        if let Ok(b) = p.read_bytes(base + rva as usize, 64) {
            let l = b.iter().position(|&x| x == 0).unwrap_or(64);
            if l == name.len() && b[..l].eq_ignore_ascii_case(name.as_bytes()) {
                let ord: u16 = p.read_memory(base + ao as usize + i * 2).ok()?;
                let frva: u32 = p.read_memory(base + af as usize + ord as usize * 4).ok()?;
                return Some(base + frva as usize);
            }
        }
    }
    None
}

fn call_remote(p: &ProcessHandle, addr: usize) -> Option<usize> {
    unsafe {
        let t = CreateRemoteThread(
            p.handle as *mut u8,
            std::ptr::null_mut(),
            0,
            std::mem::transmute::<usize, extern "system" fn() -> usize>(addr),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
        );
        if t.is_null() {
            return None;
        }
        if WaitForSingleObject(t, 5000) != 0 {
            CloseHandle(t);
            return None;
        }
        let mut c: u32 = 0;
        if GetExitCodeThread(t, &mut c) == 0 {
            CloseHandle(t);
            return None;
        }
        CloseHandle(t);
        Some(c as usize)
    }
}

extern "system" {
    fn CreateRemoteThread(
        h: *mut u8,
        _a: *mut u8,
        _s: usize,
        f: extern "system" fn() -> usize,
        _p: *mut u8,
        _f: u32,
        _tid: *mut u32,
    ) -> *mut u8;
    fn WaitForSingleObject(h: *mut u8, ms: u32) -> u32;
    fn GetExitCodeThread(h: *mut u8, e: *mut u32) -> i32;
    fn CloseHandle(h: *mut u8) -> i32;
}
