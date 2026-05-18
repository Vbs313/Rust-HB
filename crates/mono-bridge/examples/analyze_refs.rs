//! Analyze found MonoClass references — FIXED version
use hb_core::win32::ProcessHandle;

fn main() {
    println!("=== MonoClass Layout Analysis ===\n");
    let p = ProcessHandle::find_by_name("Hearthstone").expect("Hearthstone not running");
    println!("✅ PID={}\n", p.pid);

    // Format: (ref_address, string_address, name)
    let refs = [
        (0x0512046c, 0x02b8ac18, "GameState"),
        (0x05161c4c, 0x02b43918, "Entity"),
        (0x0514d85c, 0x02bacdb8, "GameEntity"),
    ];

    for &(ref_addr, str_addr, name) in &refs {
        println!("=== '{name}' ref @ 0x{ref_addr:08x} (points to '{name}' string @ 0x{str_addr:08x}) ===");
        
        for name_off in [0x00usize, 0x04, 0x08, 0x0C, 0x10, 0x14, 0x18, 0x1C, 0x20, 0x24, 0x28] {
            if name_off > ref_addr { continue; }
            let class_start = ref_addr - name_off;
            
            let hdr = match p.read_bytes(class_start, 0x50) {
                Ok(d) => d,
                Err(_) => continue,
            };
            
            // Verify: at offset name_off within hdr, we should see str_addr
            if read_u32(&hdr, name_off) as usize != str_addr { continue; }
            
            println!("\n   ✅ name@+0x{name_off:02X} => MonoClass @ 0x{class_start:08x}");
            
            // Dump first 64 bytes
            for row in 0..4 {
                let off = row * 16;
                let mut hex = String::new();
                let mut asc = String::new();
                for col in 0..16 {
                    let b = hdr[off + col];
                    hex.push_str(&format!("{b:02x} "));
                    asc.push(if b.is_ascii_graphic() { b as char } else { '.' });
                }
                println!("   +0x{off:02X} | {hex:48} | {asc}");
            }
            
            // Analyze ALL pointer fields (potential ns, image, parent, etc.)
            println!("\n   Pointer fields:");
            for off in (0..64usize).step_by(4) {
                let val = read_u32(&hdr, off) as usize;
                if val == str_addr { 
                    println!("   +0x{off:02X}: 0x{val:08x} ← NAME ⭐");
                    continue;
                }
                if val == 0 || val == 0xFFFFFFFF || val < 0x10000 || val > 0x7FFFFFFF { continue; }
                
                // Is it a string? (namespace candidate)
                if let Some(s) = read_str_opt(&p, val, 48) {
                    if s.len() >= 2 && !s.contains('\u{FFFD}') && s.len() <= 40 {
                        println!("   +0x{off:02X}: 0x{val:08x} -> '{s}'");
                        continue;
                    }
                }
                
                // Is it an image? (has name at +0x1C)
                let img_name = read_str_opt(&p, p.read_memory::<u32>(val+0x1C).unwrap_or(0) as usize, 48).unwrap_or_default();
                if img_name.len() > 3 {
                    println!("   +0x{off:02X}: 0x{val:08x} -> IMAGE '{img_name}' ⭐");
                    continue;
                }
                
                // Is it a vtable or class pointer?
                let val2 = p.read_memory::<u32>(val).unwrap_or(0) as usize;
                if val2 > 0x10000 && val2 < 0x7FFFFFFF {
                    // Check val+0x1C for potential class name
                    let vname = read_str_opt(&p, p.read_memory::<u32>(val2+0x1C).unwrap_or(0) as usize, 32).unwrap_or_default();
                    if vname.len() > 3 {
                        println!("   +0x{off:02X}: 0x{val:08x} -> type '{vname}'");
                    } else {
                        println!("   +0x{off:02X}: 0x{val:08x} -> ptr->ptr");
                    }
                } else {
                    println!("   +0x{off:02X}: 0x{val:08x}");
                }
            }
            
            // Small integers (field_count, class_size)
            println!("\n   Small ints:");
            for off in (0..64usize).step_by(4) {
                let val = read_u32(&hdr, off) as usize;
                if val > 0 && val < 500 && val as u32 as usize == val {
                    println!("   +0x{off:02X}: {val}");
                }
            }
            break;
        }
        println!();
    }
}

fn read_u32(data: &[u8], off: usize) -> u32 {
    if off + 4 > data.len() { 0 } else {
        u32::from_le_bytes([data[off], data[off+1], data[off+2], data[off+3]])
    }
}

fn read_str_opt(p: &ProcessHandle, addr: usize, max: usize) -> Option<String> {
    if addr < 0x10000 || addr > 0x7FFFFFFF { return None; }
    let b = p.read_bytes(addr, max).ok()?;
    let l = b.iter().position(|&x| x == 0).unwrap_or(max);
    if l < 2 { return None; }
    let s = String::from_utf8_lossy(&b[..l]).to_string();
    if s.contains('\u{FFFD}') { None } else { Some(s) }
}
