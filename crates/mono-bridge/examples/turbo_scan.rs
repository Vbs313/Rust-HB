//! 超高速 MonoImage 扫描器
//!
//! 用 CreateRemoteThread 调用 mono_get_domain / mono_assembly_foreach
//! 来获取所有已加载的 MonoImage，无需内存扫描

use hb_core::win32::ProcessHandle;

fn main() {
    println!("=== MonoImage Turbo Scanner (via CreateRemoteThread) ===\n");

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

    // 1. Find mono.dll
    println!("\n[Step 1] Finding mono.dll...");
    let mono_base = find_mono_base(&p);
    let mono = match mono_base {
        Some(b) => {
            println!("   mono @ 0x{b:08x}");
            b
        }
        None => {
            println!("❌ mono.dll not found");
            return;
        }
    };

    // 2. Find key exports
    println!("\n[Step 2] Finding Mono API exports...");
    let get_root_domain = find_export(&p, mono, "mono_get_root_domain");
    let get_corlib = find_export(&p, mono, "mono_get_corlib");
    let class_from_name = find_export(&p, mono, "mono_class_from_name");
    let class_get_name = find_export(&p, mono, "mono_class_get_name");
    let class_get_namespace = find_export(&p, mono, "mono_class_get_namespace");
    let class_get_fields = find_export(&p, mono, "mono_class_get_fields");
    let class_get_field_count = find_export(&p, mono, "mono_class_num_fields");
    let class_get_instance_size = find_export(&p, mono, "mono_class_instance_size");
    let image_get_name = find_export(&p, mono, "mono_image_get_name");
    let assembly_foreach = find_export(&p, mono, "mono_assembly_foreach");
    let domain_assemblies = find_export(&p, mono, "mono_domain_assemblies");

    println!(
        "   mono_get_root_domain     @ 0x{:08x}",
        get_root_domain.unwrap_or(0)
    );
    println!(
        "   mono_get_corlib          @ 0x{:08x}",
        get_corlib.unwrap_or(0)
    );
    println!(
        "   mono_class_from_name     @ 0x{:08x}",
        class_from_name.unwrap_or(0)
    );

    // 3. Call mono_get_root_domain
    println!("\n[Step 3] Calling mono_get_root_domain()...");
    let root_domain = get_root_domain.and_then(|addr| call_remote(&p, addr));
    match root_domain {
        Some(addr) => println!("   root_domain @ 0x{addr:08x}"),
        None => {
            println!("❌ mono_get_root_domain failed (CreateRemoteThread blocked by ACG?)");
        }
    }

    // 4. Call mono_get_corlib
    println!("\n[Step 4] Calling mono_get_corlib()...");
    let corlib = get_corlib.and_then(|addr| call_remote(&p, addr));
    match corlib {
        Some(addr) => {
            let name = read_string(&p, read_u32(&p, addr + 0x1C) as usize, 32);
            println!("   corlib MonoImage @ 0x{addr:08x} name='{name}'");
        }
        None => {
            println!("❌ mono_get_corlib failed");
        }
    }

    // 5. Read root domain structure to find assemblies
    let domain_data: Option<(Vec<u8>, usize)> = root_domain.and_then(|domain| {
        println!("\n[Step 5] Dumping root domain struct (first 512 bytes)...");
        p.read_bytes(domain, 0x200).ok().map(|data| (data, domain))
    });

    if let Some((ref data, domain)) = domain_data {
        dump_hex(data, domain, "MonoDomain");

        // Search for corlib image pointer within domain struct
        println!("\n[Step 6] Looking for assembly/image pointers in domain...");
        let corlib_val = corlib.unwrap_or(0);
        for off in (0..data.len().saturating_sub(4)).step_by(4) {
            let val = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
                as usize;
            if val == corlib_val && val > 0 {
                println!("   +0x{off:04X} → corlib image pointer!");
            }
        }
    }

    // 6. Scan domain for assembly list
    let corlib_val = corlib.unwrap_or(0);
    if let Some((ref data, _)) = domain_data {
        println!("\n[Step 7] Scanning domain for assembly list...");
        for off in (0..data.len().saturating_sub(12)).step_by(4) {
            let ptr = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
                as usize;
            if !(0x00010000..0x7FFFFFFF).contains(&ptr) {
                continue;
            }

            if let Ok(arr_data) = p.read_bytes(ptr, 12) {
                let len = u32::from_le_bytes([arr_data[4], arr_data[5], arr_data[6], arr_data[7]])
                    as usize;
                let data_ptr =
                    u32::from_le_bytes([arr_data[8], arr_data[9], arr_data[10], arr_data[11]])
                        as usize;
                if len > 0 && len < 200 && (0x00010000..0x7FFFFFFF).contains(&data_ptr) {
                    if let Ok(first) = p.read_memory::<u32>(data_ptr) {
                        let assembly = first as usize;
                        if (0x00010000..0x7FFFFFFF).contains(&assembly) {
                            let img = p.read_memory::<u32>(assembly + 0x08).unwrap_or(0) as usize;
                            let name = if (0x00010000..0x7FFFFFFF).contains(&img) {
                                read_string(&p, read_u32(&p, img + 0x1C) as usize, 48)
                            } else {
                                String::new()
                            };
                            println!("   +0x{off:02X} → ptr 0x{ptr:08x}: GPtrArray(len={len}) first='{name}'");

                            // Enumerate all assemblies
                            for i in 0..len.min(100) {
                                if let Ok(asm) = p.read_memory::<u32>(data_ptr + i * 4) {
                                    let asm_addr = asm as usize;
                                    if (0x00010000..0x7FFFFFFF).contains(&asm_addr)
                                        && asm_addr != assembly
                                    {
                                        if let Ok(img) = p.read_memory::<u32>(asm_addr + 0x08) {
                                            let img_addr = img as usize;
                                            if (0x00010000..0x7FFFFFFF).contains(&img_addr)
                                                && img_addr != corlib_val
                                            {
                                                let name = read_string(
                                                    &p,
                                                    read_u32(&p, img_addr + 0x1C) as usize,
                                                    48,
                                                );
                                                if !name.is_empty() && name.len() > 3 {
                                                    println!("     [{i}] asm 0x{asm_addr:08x} → image 0x{img_addr:08x} '{name}'");
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn find_mono_base(p: &ProcessHandle) -> Option<usize> {
    let mut a: usize = 0x01000000;
    while a < 0x80000000 {
        if let Ok(mz) = p.read_memory::<u16>(a) {
            if mz == 0x5A4D {
                if find_export(p, a, "mono_get_root_domain").is_some() {
                    return Some(a);
                }
            }
        }
        a += 0x10000;
    }
    None
}

fn find_export(p: &ProcessHandle, base: usize, name: &str) -> Option<usize> {
    let lfanew: i32 = p.read_memory(base + 0x3C).ok()?;
    let nt = base + lfanew as usize;
    let opt_magic: u16 = p.read_memory(nt + 24).ok()?;
    if opt_magic != 0x10B {
        return None;
    }
    let dd = nt + 24 + 0x60;
    let eva: u32 = p.read_memory(dd).ok()?;
    if eva == 0 {
        return None;
    }
    let ea = base + eva as usize;
    let nn: u32 = p.read_memory(ea + 0x18).ok()?;
    let an: u32 = p.read_memory(ea + 0x20).ok()?;
    let ao: u32 = p.read_memory(ea + 0x24).ok()?;
    let af: u32 = p.read_memory(ea + 0x1C).ok()?;
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

fn read_u32(p: &ProcessHandle, addr: usize) -> u32 {
    p.read_memory::<u32>(addr).unwrap_or(0)
}

fn read_string(p: &ProcessHandle, addr: usize, max: usize) -> String {
    if addr < 0x10000 {
        return String::new();
    }
    if let Ok(b) = p.read_bytes(addr, max) {
        let l = b.iter().position(|&x| x == 0).unwrap_or(max);
        String::from_utf8_lossy(&b[..l]).to_string()
    } else {
        String::new()
    }
}

fn dump_hex(data: &[u8], base: usize, label: &str) {
    println!("   {label} @ 0x{base:08x}:");
    for row in 0..data.len().min(512) / 16 {
        let off = row * 16;
        let mut hex = String::new();
        let mut asc = String::new();
        for col in 0..16 {
            if off + col < data.len() {
                let b = data[off + col];
                hex.push_str(&format!("{b:02x} "));
                asc.push(if b.is_ascii_graphic() { b as char } else { '.' });
            }
        }
        println!("   +0x{off:02X} | {hex:48} | {asc}");
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
