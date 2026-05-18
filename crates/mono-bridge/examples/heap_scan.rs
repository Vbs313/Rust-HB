//! 在 Mono GC 堆上直接搜索 MonoClass 结构
//! 策略：扫描 0x01000000-0x40000000 中每个 4 字节对齐位置
//! 检查是否有两个相邻的指针分别指向有效的类名字符串和命名空间字符串

use hb_core::win32::{ProcessHandle, query_memory_info};

fn main() {
    println!("=== MonoClass Heap Scanner ===\n");
    let p = ProcessHandle::find_by_name("Hearthstone").expect("Hearthstone not running");
    println!("✅ PID={}\n", p.pid);

    let corlib_img: usize = 0x0969ea60;
    let corlib_name = read_str_opt(&p, 
        p.read_memory::<u32>(corlib_img + 0x1C).unwrap_or(0) as usize, 48)
        .unwrap_or_default();
    println!("corlib @ 0x{corlib_img:08x} name='{corlib_name}'");

    // Find Assembly-CSharp MonoImage
    let asm_img = find_image_fast(&p, "Assembly-CSharp", corlib_img);
    if let Some(img) = asm_img {
        println!("Assembly-CSharp @ 0x{img:08x}");
        
        // Scan MonoImage for class_cache
        if let Ok(data) = p.read_bytes(img, 0x200) {
            println!("\nScanning MonoImage for class_cache pointer...");
            for off in (0..data.len()-12).step_by(4) {
                let ptr = read_u32(&data, off) as usize;
                if ptr < 0x10000 || ptr > 0x7FFFFFFF { continue; }
                
                if let Some(classes) = try_gptrarray(&p, ptr, img) {
                    let len = classes.len();
                    println!("\n   🎯 MonoImage+0x{off:02X} = GPtrArray @ 0x{ptr:08x}: {len} classes");
                    let mut found_game = false;
                    for (ns, name) in classes.iter().take(20) {
                        println!("      {ns}.{name}");
                        for t in &["SceneMgr", "GameState", "Entity", "GameStateManager"] {
                            if name == t { found_game = true; }
                        }
                    }
                    if classes.len() > 20 { println!("      ... {} more", classes.len() - 20); }
                    if found_game { println!("\n   ✅ Found game classes!"); }
                    return;
                }
                
                if let Some(classes) = try_ghashtable(&p, ptr) {
                    let len = classes.len();
                    if len > 10 {
                        println!("\n   🎯 MonoImage+0x{off:02X} = GHashTable @ 0x{ptr:08x}: {len} entries");
                        for (ns, name) in classes.iter().take(10) {
                            println!("      {ns}.{name}");
                        }
                        return;
                    }
                }
            }
        }
    } else {
        println!("❌ Assembly-CSharp not found");
    }
}

fn find_image_fast(p: &ProcessHandle, target: &str, center: usize) -> Option<usize> {
    for range in [0x200000usize, 0x400000, 0x800000, 0x1000000] {
        let start = center.saturating_sub(range);
        let end = center.saturating_add(range).min(0x7FFFFFFF);
        let mut addr = start;
        while addr < end {
            let mbi = match query_memory_info(p, addr) {
                Ok(m) => m,
                Err(_) => { addr += 0x10000; continue; }
            };
            let s = mbi.BaseAddress as usize;
            let sz = mbi.RegionSize;
            if mbi.State == 0x1000 && (mbi.Protect & 0x04) != 0 && sz < 0x200000 {
                if let Ok(data) = p.read_bytes(s, sz.min(0x40000)) {
                    for i in (0..data.len().saturating_sub(0x28)).step_by(4) {
                        let np = read_u32(&data, i + 0x1C) as usize;
                        let fp = read_u32(&data, i + 0x20) as usize;
                        if np <= 0x10000 || fp <= 0x10000 || np > 0x7FFFFFFF || fp > 0x7FFFFFFF { continue; }
                        if let Some(name) = read_str_opt(p, np, 48) {
                            if name == target { return Some(s + i); }
                        }
                    }
                }
            }
            addr = s + sz.max(0x10000);
            if addr > end { break; }
        }
    }
    None
}

fn try_gptrarray(p: &ProcessHandle, addr: usize, img: usize) -> Option<Vec<(String, String)>> {
    let data = p.read_bytes(addr, 12).ok()?;
    let len = read_u32(&data, 4) as usize;
    let arr = read_u32(&data, 8) as usize;
    if len == 0 || len > 5000 || arr < 0x10000 || arr > 0x7FFFFFFF { return None; }
    
    let first = p.read_memory::<u32>(arr).ok()? as usize;
    if first < 0x10000 || first > 0x7FFFFFFF { return None; }
    
    if let Some((ns, name)) = try_monoclass(&p, first, img) {
        let mut classes = vec![(ns, name)];
        for i in 1..len.min(500) {
            if let Ok(e) = p.read_memory::<u32>(arr + i * 4) {
                let eaddr = e as usize;
                if eaddr > 0x10000 && eaddr < 0x7FFFFFFF {
                    if let Some(mc) = try_monoclass(&p, eaddr, img) {
                        classes.push(mc);
                    }
                }
            }
        }
        Some(classes)
    } else {
        None
    }
}

fn try_ghashtable(p: &ProcessHandle, addr: usize) -> Option<Vec<(String, String)>> {
    let data = p.read_bytes(addr, 24).ok()?;
    let num_entries = read_u32(&data, 16) as usize;
    let nodes = read_u32(&data, 20) as usize;
    if num_entries == 0 || num_entries > 50000 || nodes < 0x10000 || nodes > 0x7FFFFFFF { return None; }
    
    let mut classes = Vec::new();
    let max_read = (num_entries * 12).min(0x40000);
    let ndata = match p.read_bytes(nodes, max_read) {
        Ok(d) => d,
        Err(_) => return None,
    };
    
    for i in 0..num_entries.min(2000) {
        let off = i * 12;
        if off + 8 > ndata.len() { break; }
        let key = read_u32(&ndata, off) as usize;
        if key < 0x10000 || key > 0x7FFFFFFF { continue; }
        if let Some(s) = read_str_opt(p, key, 64) {
            if s.contains('.') && s.len() > 3 {
                let parts: Vec<&str> = s.splitn(2, '.').collect();
                if parts.len() == 2 && parts[1].len() > 1 {
                    classes.push((parts[0].into(), parts[1].into()));
                }
            }
        }
    }
    if classes.len() > 5 { Some(classes) } else { None }
}

fn try_monoclass(p: &ProcessHandle, addr: usize, img: usize) -> Option<(String, String)> {
    let data = p.read_bytes(addr, 0x30).ok()?;
    if data[0] == 0 || data[0] == 0xFF { return None; }
    
    for name_off in [0x04usize, 0x08, 0x0C, 0x10, 0x14, 0x18] {
        let name_ptr = read_u32(&data, name_off) as usize;
        if name_ptr <= 0x10000 || name_ptr > 0x7FFFFFFF { continue; }
        let name = match read_str_opt(p, name_ptr, 32) {
            Some(n) => n,
            None => continue,
        };
        if name.len() < 2 || name.len() > 30 { continue; }
        if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '`') { continue; }
        
        for ns_off in [0x00usize, 0x04, 0x08, 0x0C, 0x10, 0x14, 0x18] {
            if ns_off == name_off { continue; }
            let ns_ptr = read_u32(&data, ns_off) as usize;
            if ns_ptr <= 0x10000 || ns_ptr > 0x7FFFFFFF || ns_ptr == name_ptr { continue; }
            let ns = match read_str_opt(p, ns_ptr, 32) {
                Some(n) => n,
                None => continue,
            };
            if ns.len() < 2 || ns.len() > 30 { continue; }
            if !ns.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.') { continue; }
            
            for img_off in [0x00usize, 0x04, 0x08, 0x0C, 0x10, 0x14] {
                if img_off == name_off || img_off == ns_off { continue; }
                let img_ptr = read_u32(&data, img_off) as usize;
                if img_ptr == img { return Some((ns, name)); }
            }
        }
    }
    None
}

fn read_u32(data: &[u8], off: usize) -> u32 {
    if off + 4 > data.len() { return 0; }
    u32::from_le_bytes([data[off], data[off+1], data[off+2], data[off+3]])
}

fn read_str_opt(p: &ProcessHandle, addr: usize, max: usize) -> Option<String> {
    if addr < 0x10000 || addr > 0x7FFFFFFF { return None; }
    let b = p.read_bytes(addr, max).ok()?;
    let l = b.iter().position(|&x| x == 0).unwrap_or(max);
    if l < 2 { return None; }
    let s = String::from_utf8_lossy(&b[..l]).to_string();
    if s.contains('\u{FFFD}') { None } else { Some(s) }
}
