//! 通过 root_domain 结构定位 Assembly-CSharp（验证版）
//! 已知：domain @ 0x09d22e70, corlib_img @ 0x0969ea60

use hb_core::win32::ProcessHandle;

fn main() {
    println!("=== Domain-to-Image Navigator ===\n");
    let p = ProcessHandle::find_by_name("Hearthstone").expect("Hearthstone not running");
    println!("✅ PID={}", p.pid);

    let domain: usize = 0x09d22e70;
    let corlib_img: usize = 0x0969ea60;
    
    // 1. Verify corlib image
    let corlib_name = read_str(&p, p.read_memory::<u32>(corlib_img + 0x1C).unwrap_or(0) as usize, 32);
    println!("corlib image @ 0x{corlib_img:08x} name='{corlib_name}'");
    
    // 2. Read domain + 0xB8 candidate (assembly pointer)
    println!("\n--- domain+0xB8 = 0x09699be0 ---");
    if let Ok(data) = p.read_bytes(0x09699be0, 0x20) {
        let v0 = read_u32(&data, 0) as usize; // name?
        let v4 = read_u32(&data, 4) as usize; // ?
        let v8 = read_u32(&data, 8) as usize; // image?
        let vc = read_u32(&data, 12) as usize;
        
        println!("  +0x00: 0x{v0:08x} (name?)");
        println!("  +0x04: 0x{v4:08x}");
        println!("  +0x08: 0x{v8:08x} (image?)");
        println!("  +0x0C: 0x{vc:08x}");
        
        // Check v0 as string (assembly name)
        if v0 > 0x10000 && v0 < 0x7FFFFFFF {
            let name = read_str(&p, v0, 32);
            println!("  name -> '{name}'");
        }
        
        // Check v8 as image (has name at +0x1C)
        if v8 > 0x10000 && v8 < 0x7FFFFFFF {
            let img_name = read_str(&p, p.read_memory::<u32>(v8 + 0x1C).unwrap_or(0) as usize, 32);
            println!("  image +0x08 -> 0x{v8:08x} name='{img_name}'");
        }
        
        if vc > 0x10000 && vc < 0x7FFFFFFF {
            let img_name = read_str(&p, p.read_memory::<u32>(vc + 0x1C).unwrap_or(0) as usize, 32);
            println!("  image +0x0C -> 0x{vc:08x} name='{img_name}'");
        }
    }

    // 3. Check domain+0x34 candidate
    println!("\n--- domain+0x34 = 0x09d25150 ---");
    if let Ok(data) = p.read_bytes(0x09d25150, 0x20) {
        for off in (0..20).step_by(4) {
            let v = read_u32(&data, off) as usize;
            if v > 0x10000 && v < 0x7FFFFFFF {
                // Check if it's an image
                let img_name = read_str(&p, p.read_memory::<u32>(v + 0x1C).unwrap_or(0) as usize, 32);
                if img_name.len() > 3 && !img_name.contains('\u{FFFD}') {
                    println!("  +0x{off:02X}: 0x{v:08x} -> image '{img_name}'");
                }
            }
        }
    }
    
    // 4. Check domain+0x24 range (0x09d3bfc0 to 0x09d27f70)
    for &candidate in &[0x09d27f70usize, 0x09d29fa0, 0x09d3bfc0, 0x09d32f78, 
                        0x09d32f30, 0x09d32ee8, 0x09d25150, 0x09d34ff0, 0x0a68cbe0] {
        // Check if it's an Assembly (with image at +0x08)
        if let Ok(data) = p.read_bytes(candidate, 0x20) {
            // Check +0x00 for name
            let n0 = read_u32(&data, 0) as usize;
            let img = read_u32(&data, 8) as usize;
            
            if img > 0x10000 && img < 0x7FFFFFFF {
                let img_name = read_str(&p, p.read_memory::<u32>(img + 0x1C).unwrap_or(0) as usize, 32);
                if img_name.len() > 3 {
                    println!("\n  0x{candidate:08x}: ASSEMBLY? +0x08 -> image 0x{img:08x} '{img_name}'");
                    
                    if n0 > 0x10000 && n0 < 0x7FFFFFFF {
                        let name = read_str(&p, n0, 32);
                        println!("    asm name = '{name}'");
                    }
                }
            }
        }
    }
    
    // 5. Check corlib area for other images (scan ±8MB from corlib)
    println!("\n--- Corlib area image scan (fast, high-accuracy) ---");
    // Search for images but verify: name must contain valid chars AND
    // be followed by a valid filename at +0x20
    let start = corlib_img.saturating_sub(0x800000);
    let end = corlib_img.saturating_add(0x800000).min(0x7FFFFFFF);
    let mut addr = start;
    while addr < end {
        let mbi = match hb_core::win32::query_memory_info(&p, addr) {
            Ok(m) => m,
            Err(_) => { addr += 0x10000; continue; }
        };
        let s = mbi.BaseAddress as usize;
        let sz = mbi.RegionSize;
        if mbi.State == 0x1000 && (mbi.Protect & 0x04) != 0 && sz < 0x800000 {
            if let Ok(data) = p.read_bytes(s, sz.min(0x80000)) {
                for i in (0..data.len().saturating_sub(0x28)).step_by(4) {
                    let np = read_u32(&data, i+0x1C) as usize;
                    let fp = read_u32(&data, i+0x20) as usize;
                    if np < 0x10000 || np > 0x7FFFFFFF || fp < 0x10000 || fp > 0x7FFFFFFF { continue; }
                    
                    // Faster: read directly from the buffer for name match
                    if let Ok(nb) = p.read_bytes(np, 48) {
                        let nl = nb.iter().position(|&x| x == 0).unwrap_or(48);
                        if nl < 3 || nl > 30 { continue; }
                        let name = String::from_utf8_lossy(&nb[..nl]).to_string();
                        if name.contains('\u{FFFD}') { continue; }
                        
                        // Only print DLL-like image names
                        if name.contains("Assembly") || name.contains("System") || name.contains("Unity") || name.contains("Newtonsoft") || name.contains("Ionic") || name == "mscorlib" || name.contains("HB") || name.contains("Triton") || name.contains("Hearthbuddy") || name.contains("Bots") || name.contains("Routine") {
                            // Verify: read filename too
                            if let Ok(fb) = p.read_bytes(fp, 48) {
                                let fl = fb.iter().position(|&x| x == 0).unwrap_or(48);
                                let fname = String::from_utf8_lossy(&fb[..fl]).to_string();
                                if (fname.ends_with(".dll") || fname.ends_with(".exe")) && !fname.contains('\u{FFFD}') {
                                    let abs = s + i;
                                    if abs != corlib_img { // skip corlib
                                        println!("  @ 0x{abs:08x}: {name} ({fname})");
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        addr = s + sz.max(0x10000);
        if addr > end { break; }
    }
}

fn read_u32(data: &[u8], off: usize) -> u32 {
    if off + 4 > data.len() { 0 } else {
        u32::from_le_bytes([data[off], data[off+1], data[off+2], data[off+3]])
    }
}

fn read_str(p: &ProcessHandle, addr: usize, max: usize) -> String {
    if addr < 0x10000 || addr > 0x7FFFFFFF { return String::new(); }
    if let Ok(b) = p.read_bytes(addr, max) {
        let l = b.iter().position(|&x| x == 0).unwrap_or(max);
        if l >= 2 { return String::from_utf8_lossy(&b[..l]).to_string(); }
    }
    String::new()
}
