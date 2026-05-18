//! 验证 Assembly-CSharp 并找 class_cache
//! Assembly-CSharp candidate @ 0x0685f5d4

use hb_core::win32::ProcessHandle;

fn main() {
    println!("=== Assembly-CSharp Validation ===\n");
    let p = ProcessHandle::find_by_name("Hearthstone").expect("Hearthstone not running");
    println!("✅ PID={}", p.pid);

    let asm_img: usize = 0x0685f5d4;

    // 1. Verify name
    let name_ptr = p.read_memory::<u32>(asm_img + 0x1C).unwrap_or(0) as usize;
    println!("name_ptr @ +0x1C = 0x{name_ptr:08x}");
    let name = read_str(&p, name_ptr, 48);
    println!("name = '{name}'");

    // 2. Verify filename
    let file_ptr = p.read_memory::<u32>(asm_img + 0x20).unwrap_or(0) as usize;
    println!("file_ptr @ +0x20 = 0x{file_ptr:08x}");
    let fname = read_str(&p, file_ptr, 48);
    println!("filename = '{fname}'");

    if name != "Assembly-CSharp" {
        println!("❌ Not Assembly-CSharp! Got '{name}'");
        return;
    }
    println!("✅ CONFIRMED: Assembly-CSharp @ 0x{asm_img:08x}");

    // 3. Read FULL struct (first 1024 bytes) and find class_cache
    println!("\n--- Scanning for class_cache (0-1024 bytes) ---");
    if let Ok(data) = p.read_bytes(asm_img, 0x400) {
        for off in (0..data.len() - 12).step_by(4) {
            let ptr = read_u32(&data, off) as usize;
            if ptr < 0x10000 || ptr > 0x7FFFFFFF {
                continue;
            }

            // Try GPtrArray
            if let Ok(ad) = p.read_bytes(ptr, 12) {
                let len = read_u32(&ad, 4) as usize;
                let arr = read_u32(&ad, 8) as usize;
                if len > 0 && len < 5000 && arr > 0x10000 && arr < 0x7FFFFFFF {
                    if let Ok(first) = p.read_memory::<u32>(arr) {
                        let f = first as usize;
                        if f > 0x10000 && f < 0x7FFFFFFF {
                            if let Ok(fd) = p.read_bytes(f, 0x30) {
                                for no in [0x04, 0x08, 0x0C, 0x10] {
                                    let np = read_u32(&fd, no) as usize;
                                    if np > 0x10000 && np < 0x7FFFFFFF {
                                        if let Some(n) = read_str_opt(&p, np, 32) {
                                            if n.len() >= 2
                                                && n.len() <= 30
                                                && n.chars()
                                                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
                                            {
                                                // Found! Check if image matches
                                                for io in [0x00, 0x04, 0x08, 0x0C] {
                                                    if io == no {
                                                        continue;
                                                    }
                                                    let ip = read_u32(&fd, io) as usize;
                                                    if ip == asm_img {
                                                        println!("\n   🎯 MonoImage+0x{off:02X} = GPtrArray @ 0x{ptr:08x}");
                                                        println!(
                                                            "   First class: {n} (image verified)"
                                                        );

                                                        // Dump ALL classes
                                                        for i in 0..len.min(100) {
                                                            if let Ok(e) =
                                                                p.read_memory::<u32>(arr + i * 4)
                                                            {
                                                                let ea = e as usize;
                                                                if ea > 0x10000
                                                                    && ea < 0x7FFFFFFF
                                                                    && ea != f
                                                                {
                                                                    if let Ok(ed) =
                                                                        p.read_bytes(ea, 0x30)
                                                                    {
                                                                        'class_scan: for no2 in
                                                                            [0x04, 0x08, 0x0C, 0x10]
                                                                        {
                                                                            let np2 =
                                                                                read_u32(&ed, no2)
                                                                                    as usize;
                                                                            if np2 > 0x10000
                                                                                && np2 < 0x7FFFFFFF
                                                                            {
                                                                                if let Some(n2) =
                                                                                    read_str_opt(
                                                                                        &p, np2, 32,
                                                                                    )
                                                                                {
                                                                                    if n2.len() >= 2
                                                                                    {
                                                                                        // Check image
                                                                                        for io2 in [
                                                                                            0x00,
                                                                                            0x04,
                                                                                            0x08,
                                                                                            0x0C,
                                                                                        ] {
                                                                                            if io2 == no2 { continue; }
                                                                                            if read_u32(&ed, io2) as usize == asm_img {
                                                                                                if n2 == "SceneMgr" || n2 == "GameState" || n2 == "Entity" || n2 == "GameStateManager" || n2 == "GameEntity" {
                                                                                                    println!("      [{i}] {n2} ⭐");
                                                                                                }
                                                                                                break 'class_scan;
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
                }
            }

            // Try GHashTable
            if let Ok(ad) = p.read_bytes(ptr, 24) {
                let ne = read_u32(&ad, 16) as usize;
                let nodes = read_u32(&ad, 20) as usize;
                if ne > 0 && ne < 50000 && nodes > 0x10000 && nodes < 0x7FFFFFFF {
                    if let Ok(nd) = p.read_bytes(nodes, 12) {
                        let key = read_u32(&nd, 0) as usize;
                        if key > 0x10000 && key < 0x7FFFFFFF {
                            if let Some(s) = read_str_opt(&p, key, 64) {
                                if s.contains('.') && s.len() > 5 && s.len() < 60 {
                                    println!(
                                        "\n   🎯 MonoImage+0x{off:02X} = GHashTable @ 0x{ptr:08x}"
                                    );
                                    println!("   First key: {s}");

                                    // Dump entries
                                    let max_read = (ne * 12).min(0x40000);
                                    if let Ok(ndata) = p.read_bytes(nodes, max_read) {
                                        let mut count = 0;
                                        for i in 0..ne.min(5000) {
                                            let o = i * 12;
                                            if o + 12 > ndata.len() {
                                                break;
                                            }
                                            let k = read_u32(&ndata, o) as usize;
                                            let v = read_u32(&ndata, o + 4) as usize;
                                            if k > 0x10000 && k < 0x7FFFFFFF {
                                                if let Some(s2) = read_str_opt(&p, k, 64) {
                                                    if s2.contains('.') {
                                                        // Verify value is a valid MonoClass in our image
                                                        if v > 0x10000 && v < 0x7FFFFFFF {
                                                            if let Ok(vd) = p.read_bytes(v, 0x30) {
                                                                for no in [0x04, 0x08, 0x0C, 0x10] {
                                                                    let vnp =
                                                                        read_u32(&vd, no) as usize;
                                                                    if vnp == k {
                                                                        // name pointer matches hash key
                                                                        if count < 30 {
                                                                            println!("      {s2}");
                                                                        }
                                                                        count += 1;
                                                                        break;
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        println!("   Total entries: {count}");
                                    }

                                    // Check for target classes
                                    let ndata = p
                                        .read_bytes(nodes, (ne * 12).min(0x40000))
                                        .unwrap_or_default();
                                    for i in 0..ne.min(5000) {
                                        let o = i * 12;
                                        if o + 12 > ndata.len() {
                                            break;
                                        }
                                        let k = read_u32(&ndata, o) as usize;
                                        if k > 0x10000 && k < 0x7FFFFFFF {
                                            if let Some(s2) = read_str_opt(&p, k, 64) {
                                                for t in &[
                                                    "SceneMgr",
                                                    "GameState",
                                                    "Entity",
                                                    "GameStateManager",
                                                ] {
                                                    if s2.contains(t) {
                                                        println!("   ⭐ Found: {s2}");
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
    println!("   No class_cache found in first 1024 bytes");

    // 4. Extend search to full struct
    println!("\n--- Scanning 1024-4096 bytes ---");
    if let Ok(data) = p.read_bytes(asm_img + 0x400, 0xC00) {
        for off in (0..data.len() - 12).step_by(4) {
            let ptr = read_u32(&data, off) as usize;
            if ptr < 0x10000 || ptr > 0x7FFFFFFF {
                continue;
            }

            if let Ok(ad) = p.read_bytes(ptr, 12) {
                let len = read_u32(&ad, 4) as usize;
                let arr = read_u32(&ad, 8) as usize;
                if len > 0 && len < 5000 && arr > 0x10000 && arr < 0x7FFFFFFF {
                    if let Ok(first) = p.read_memory::<u32>(arr) {
                        let f = first as usize;
                        if f > 0x10000 && f < 0x7FFFFFFF {
                            if let Some(n) = read_str_opt(&p, f, 32) {
                                if n.len() >= 2
                                    && n.chars()
                                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
                                {
                                    println!(
                                        "\n   🎯 +0x{:04X}: GPtrArray(len={}) first='{n}'",
                                        off + 0x400,
                                        len
                                    );
                                    return;
                                }
                            }
                        }
                    }
                }
            }

            if let Ok(ad) = p.read_bytes(ptr, 24) {
                let ne = read_u32(&ad, 16) as usize;
                let nodes = read_u32(&ad, 20) as usize;
                if ne > 0 && ne < 50000 && nodes > 0x10000 && nodes < 0x7FFFFFFF {
                    if let Ok(nd) = p.read_bytes(nodes, 12) {
                        let key = read_u32(&nd, 0) as usize;
                        if key > 0x10000 && key < 0x7FFFFFFF {
                            if let Some(s) = read_str_opt(&p, key, 64) {
                                if s.contains('.') && s.len() > 5 {
                                    println!(
                                        "\n   🎯 +0x{:04X}: GHashTable(entries={}) first='{s}'",
                                        off + 0x400,
                                        ne
                                    );
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

fn read_u32(data: &[u8], off: usize) -> u32 {
    if off + 4 > data.len() {
        0
    } else {
        u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
    }
}

fn read_str(p: &ProcessHandle, addr: usize, max: usize) -> String {
    read_str_opt(p, addr, max).unwrap_or_default()
}

fn read_str_opt(p: &ProcessHandle, addr: usize, max: usize) -> Option<String> {
    if addr < 0x10000 || addr > 0x7FFFFFFF {
        return None;
    }
    let b = p.read_bytes(addr, max).ok()?;
    let l = b.iter().position(|&x| x == 0).unwrap_or(max);
    if l < 2 {
        return None;
    }
    let s = String::from_utf8_lossy(&b[..l]).to_string();
    if s.contains('\u{FFFD}') {
        None
    } else {
        Some(s)
    }
}
