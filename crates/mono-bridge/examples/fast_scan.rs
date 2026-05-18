//! 高速 MonoImage 扫描器 — 只找 Assembly-CSharp

use hb_core::win32::{ProcessHandle, query_memory_info};

fn main() {
    println!("=== Fast MonoImage Scanner ===\n");
    let p = ProcessHandle::find_by_name("Hearthstone").expect("Hearthstone not running");
    println!("✅ PID={}", p.pid);

    // Verify known offsets
    let corlib = 0x0969ea60;
    let name = read_str(&p, p.read_memory::<u32>(corlib + 0x1C).unwrap_or(0) as usize, 48);
    println!("corlib @ 0x{corlib:08x} name='{name}'");

    println!("\nScanning for Assembly-CSharp...");
    let result = find_image_all(&p, "Assembly-CSharp");
    match result {
        Some(addr) => {
            let name = read_str(&p, p.read_memory::<u32>(addr + 0x1C).unwrap_or(0) as usize, 48);
            println!("\n✅ Assembly-CSharp @ 0x{addr:08x} name='{name}'");
            
            // Now read the struct and find class_cache
            println!("\nAnalyzing MonoImage struct...");
            if let Ok(data) = p.read_bytes(addr, 0x200) {
                dump_hex(&data, addr, "Image");
                
                // Scan for class_cache
                println!("\nLooking for class_cache...");
                find_class_cache(&p, &data, addr);
            }
        }
        None => println!("❌ Not found"),
    }
}

fn find_image_all(p: &ProcessHandle, target: &str) -> Option<usize> {
    let mut addr: usize = 0x00010000;
    let chunk = 0x100000usize; // 1MB chunks
    let mut checked = 0u32;
    
    while addr < 0x7FFFFFFF {
        let mbi = match query_memory_info(p, addr) {
            Ok(m) => m,
            Err(_) => { addr += 0x10000; continue; }
        };
        let s = mbi.BaseAddress as usize;
        let sz = mbi.RegionSize;
        let committed = mbi.State == 0x1000;
        let readable = (mbi.Protect & 0x04) != 0;
        
        if committed && readable && sz > 0x100 && sz < 0x2000000 {
            let mut off = 0usize;
            while off < sz {
                let rs = chunk.min(sz - off);
                if let Ok(data) = p.read_bytes(s + off, rs) {
                    for i in (0..data.len().saturating_sub(0x28)).step_by(4) {
                        let np = u32::from_le_bytes([data[i+0x1C], data[i+0x1D], data[i+0x1E], data[i+0x1F]]) as usize;
                        if np < 0x00010000 || np > 0x7FFFFFFF { continue; }
                        
                        // Quick check: read first 2 bytes of the string remotely
                        // If they don't match the target, skip
                        if let Ok(first2) = p.read_bytes(np, target.len()) {
                            if first2.len() >= target.len() && &first2[..target.len()] == target.as_bytes() {
                                // Verify filename field
                                let fp = u32::from_le_bytes([data[i+0x20], data[i+0x21], data[i+0x22], data[i+0x23]]) as usize;
                                if fp > 0x10000 && fp < 0x7FFFFFFF {
                                    return Some(s + off + i);
                                }
                            }
                        }
                    }
                }
                off += chunk;
                checked += 1;
                if checked % 20 == 0 {
                    print!("\r   Scanning... 0x{:08x} ({})", s + off, checked);
                    use std::io::{Write, stdout};
                    let _ = stdout().flush();
                }
            }
        }
        addr = s + sz.max(0x10000);
    }
    None
}

fn find_class_cache(p: &ProcessHandle, img: &[u8], img_addr: usize) {
    for off in (0..img.len() - 12).step_by(4) {
        let ptr = read_u32(img, off) as usize;
        if ptr < 0x10000 || ptr > 0x7FFFFFFF { continue; }
        
        // GPtrArray
        if let Ok(d) = p.read_bytes(ptr, 12) {
            let len = read_u32(&d, 4) as usize;
            let arr = read_u32(&d, 8) as usize;
            if len > 0 && len < 5000 && arr > 0x10000 && arr < 0x7FFFFFFF {
                if let Ok(first) = p.read_memory::<u32>(arr) {
                    let f = first as usize;
                    if f > 0x10000 && f < 0x7FFFFFFF {
                        if let Ok(fd) = p.read_bytes(f, 0x30) {
                            // Check first few bytes of potential MonoClass
                            for no in [0x04usize, 0x08, 0x0C, 0x10] {
                                let np = read_u32(&fd, no) as usize;
                                if np > 0x10000 && np < 0x7FFFFFFF {
                                    if let Some(n) = read_str_opt(p, np, 32) {
                                        if n.len() >= 3 && n.len() <= 25 {
                                            println!("   MonoImage+0x{off:03X}: GPtrArray(len={})", len);
                                            println!("   First entry @ 0x{f:08x}: name='{n}'");
                                            println!("\n   🎯 class_cache @ MonoImage+0x{off:03X}!");
                                            
                                            // Dump first 10 classes
                                            for i in 0..len.min(20) {
                                                if let Ok(e) = p.read_memory::<u32>(arr + i*4) {
                                                    let ea = e as usize;
                                                    if ea > 0x10000 && ea < 0x7FFFFFFF && ea != f {
                                                        if let Ok(ed) = p.read_bytes(ea, 0x30) {
                                                            for no2 in [0x04usize, 0x08, 0x0C, 0x10] {
                                                                let np2 = read_u32(&ed, no2) as usize;
                                                                if np2 > 0x10000 && np2 < 0x7FFFFFFF {
                                                                    if let Some(n2) = read_str_opt(p, np2, 32) {
                                                                        if n2.len() >= 2 && n2.len() <= 25 {
                                                                            // Check image pointer
                                                                            for io in [0x00, 0x04, 0x08, 0x0C] {
                                                                                if io == no2 { continue; }
                                                                                let ip = read_u32(&ed, io) as usize;
                                                                                if ip == img_addr {
                                                                                    println!("      [{i}] {n2} (verified)");
                                                                                    break;
                                                                                }
                                                                            }
                                                                            break;
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // GHashTable
        if let Ok(d) = p.read_bytes(ptr, 24) {
            let ne = read_u32(&d, 16) as usize;
            let nodes = read_u32(&d, 20) as usize;
            if ne > 0 && ne < 50000 && nodes > 0x10000 && nodes < 0x7FFFFFFF {
                // Check first entry's key
                if let Ok(nd) = p.read_bytes(nodes, 12) {
                    let key = read_u32(&nd, 0) as usize;
                    if key > 0x10000 && key < 0x7FFFFFFF {
                        if let Some(s) = read_str_opt(p, key, 64) {
                            if s.contains('.') && s.len() > 5 && s.len() < 50 {
                                println!("   MonoImage+0x{off:03X}: GHashTable(entries={})", ne);
                                println!("   First key: '{s}'");
                                println!("\n   🎯 class_cache @ MonoImage+0x{off:03X}!");
                                
                                // Dump some entries
                                let max_read = (ne * 12).min(0x20000);
                                if let Ok(ndata) = p.read_bytes(nodes, max_read) {
                                    let mut count = 0;
                                    for i in 0..ne.min(5000) {
                                        let o = i * 12;
                                        if o + 8 > ndata.len() { break; }
                                        let k = read_u32(&ndata, o) as usize;
                                        if k > 0x10000 && k < 0x7FFFFFFF {
                                            if let Some(s2) = read_str_opt(p, k, 64) {
                                                if s2.contains('.') && count < 15 {
                                                    println!("      {s2}");
                                                    count += 1;
                                                }
                                            }
                                        }
                                    }
                                }
                                return;
                            }
                        }
                    }
                }
            }
        }
    }
    println!("   No class_cache found in MonoImage struct");
}

fn read_u32(data: &[u8], off: usize) -> u32 {
    if off + 4 > data.len() { return 0; }
    u32::from_le_bytes([data[off], data[off+1], data[off+2], data[off+3]])
}

fn read_str(p: &ProcessHandle, addr: usize, max: usize) -> String {
    read_str_opt(p, addr, max).unwrap_or_default()
}

fn read_str_opt(p: &ProcessHandle, addr: usize, max: usize) -> Option<String> {
    if addr < 0x10000 || addr > 0x7FFFFFFF { return None; }
    let b = p.read_bytes(addr, max).ok()?;
    let l = b.iter().position(|&x| x == 0).unwrap_or(max);
    if l < 2 { return None; }
    let s = String::from_utf8_lossy(&b[..l]).to_string();
    if s.contains('\u{FFFD}') { None } else { Some(s) }
}

fn dump_hex(data: &[u8], base: usize, label: &str) {
    println!("   {label} @ 0x{base:08x}:");
    for row in 0..data.len().min(256) / 16 {
        let off = row * 16;
        let mut hex = String::new();
        let mut asc = String::new();
        for col in 0..16 {
            let b = data[off + col];
            hex.push_str(&format!("{b:02x} "));
            asc.push(if b.is_ascii_graphic() { b as char } else { '.' });
        }
        println!("   +0x{off:02X} | {hex:48} | {asc}");
    }
}
