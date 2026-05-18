//! 简洁版 Mono 逆向 — 找已知类名 → 反推结构偏移
//!
//! 核心思路：
//! 1. 在 Hearthstone 内存中找到 "SceneMgr"/"GameState" 等字符串
//! 2. 搜索所有指向这些字符串的指针
//! 3. 每个指针都可能是一个 MonoClass 的 name 字段
//! 4. 在指针附近找 namespace 和 image 指针来验证

use hb_core::win32::ProcessHandle;

fn main() {
    println!("=== MonoClass Struct Reverser ===\n");
    let p = ProcessHandle::find_by_name("Hearthstone").expect("Hearthstone not running");
    println!("✅ PID={}\n", p.pid);

    // 找几个关键类名的字符串地址
    // 先验证算法: 用 mscorlib 中一定存在的类
    let verify_targets = ["Object", "String", "Int32", "ValueType"];
    for class_name in &verify_targets {
        find_mono_class_from_string(&p, class_name);
    }

    // 然后找游戏类
    let game_targets = [
        "SceneMgr",
        "GameState",
        "Entity",
        "GameStateManager",
        "GameEntity",
    ];
    for class_name in &game_targets {
        find_mono_class_from_string(&p, class_name);
    }

    // 也扫一下 Assembly-CSharp 映像附近找 class_cache
    println!("\n--- MonoImage class_cache scan ---");
    if let Some(img) = find_image_around(&p, "Assembly-CSharp", 0x0969ea60, 0x1000000) {
        println!("Assembly-CSharp @ 0x{img:08x}");
        scan_image_class_cache(&p, img);
    }
}

fn find_mono_class_from_string(p: &ProcessHandle, class_name: &str) {
    let bytes = class_name.as_bytes();
    let mut str_addr: Option<usize> = None;

    // Scan full address space for the string (in 64MB chunks)
    let ranges: &[(usize, usize)] = &[
        (0x0001_0000, 0x0100_0000), // low memory
        (0x0100_0000, 0x0500_0000),
        (0x0500_0000, 0x0900_0000),
        (0x0900_0000, 0x0D00_0000),
        (0x0D00_0000, 0x1100_0000),
        (0x1100_0000, 0x1500_0000),
        (0x1500_0000, 0x1900_0000),
        (0x1900_0000, 0x2000_0000),
        (0x2000_0000, 0x3000_0000),
        (0x3000_0000, 0x4000_0000),
        (0x4000_0000, 0x5000_0000),
        (0x5000_0000, 0x6000_0000),
        (0x6000_0000, 0x7000_0000),
        (0x7000_0000, 0x7FFF_0000),
    ];
    for &(rstart, rend) in ranges {
        if let Some(addr) = scan_for_string(p, rstart, rend, bytes) {
            str_addr = Some(addr);
            break;
        }
    }

    let str_addr = match str_addr {
        Some(a) => a,
        None => {
            println!("   {class_name}: string not found");
            return;
        }
    };
    println!("\n   '{class_name}' @ 0x{str_addr:08x}");

    // Search for references to this string across full GC heap
    // MonoClass instances are allocated on the GC heap (0x01000000-0x40000000)
    let mut references = Vec::new();
    for range_start in [
        0x01000000usize,
        0x05000000,
        0x09000000,
        0x0D000000,
        0x11000000,
        0x15000000,
        0x19000000,
        0x20000000,
        0x30000000,
        0x40000000,
    ] {
        scan_for_references_full(
            p,
            str_addr,
            range_start,
            range_start + 0x04000000,
            &mut references,
        );
        if references.len() > 100 {
            break;
        }
    }

    if references.is_empty() {
        println!("      No references found nearby");
        return;
    }
    println!("      Found {} references", references.len());

    // Analyze each reference to find MonoClass header
    for (i, &ref_addr) in references.iter().enumerate().take(5) {
        // The reference is somewhere inside a MonoClass struct at the name field offset
        // Try different name offset values to find the MonoClass start
        for name_off in [0x00u32, 0x04, 0x08, 0x0C, 0x10, 0x14, 0x18, 0x1C, 0x20] {
            if ref_addr < name_off as usize {
                continue;
            }
            let class_start = ref_addr - name_off as usize;

            let hdr = match p.read_bytes(class_start, 0x30) {
                Ok(d) => d,
                Err(_) => continue,
            };

            // Check if the name field value matches (sanity)
            let actual_name_ptr = read_le32_at(&hdr, name_off as usize);
            if actual_name_ptr != str_addr {
                continue;
            }

            // Find namespace field - should point to a valid string
            for ns_off in [0x00u32, 0x04, 0x08, 0x0C, 0x10, 0x14, 0x18, 0x1C, 0x20] {
                if ns_off == name_off || ns_off as usize + 4 > hdr.len() {
                    continue;
                }
                let ns_ptr = read_le32_at(&hdr, ns_off as usize);
                if ns_ptr == 0 || ns_ptr == str_addr || ns_ptr > 0x7FFFFFFF {
                    continue;
                }

                if let Some(ns) = read_string_opt(p, ns_ptr, 32) {
                    if ns.len() < 2 || ns.len() > 30 || ns.contains('\u{FFFD}') {
                        continue;
                    }
                    if !ns
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
                    {
                        continue;
                    }

                    println!("      [{i}] MonoClass @ 0x{class_start:08x} name@+0x{name_off:02X} ns@+0x{ns_off:02X}");
                    println!("           namespace = '{ns}'");

                    // Find image pointer
                    for img_off in [0x00u32, 0x04, 0x08, 0x0C, 0x10, 0x14, 0x18, 0x1C, 0x20] {
                        if img_off == name_off
                            || img_off == ns_off
                            || img_off as usize + 4 > hdr.len()
                        {
                            continue;
                        }
                        let img_ptr = read_le32_at(&hdr, img_off as usize);
                        if img_ptr <= 0x10000 || img_ptr > 0x7FFFFFFF {
                            continue;
                        }

                        let img_name_ptr =
                            p.read_memory::<u32>(img_ptr + 0x1C).unwrap_or(0) as usize;
                        if img_name_ptr > 0x10000 && img_name_ptr < 0x7FFFFFFF {
                            if let Some(img_name) = read_string_opt(p, img_name_ptr, 48) {
                                if img_name.len() > 3 {
                                    println!("           image@+0x{img_off:02X} → 0x{img_ptr:08x} '{img_name}'");
                                }
                            }
                        }
                    }

                    // Find field_count and class_size (important offsets)
                    for fc_off in [0x28u32, 0x2C, 0x30, 0x34, 0x38, 0x3C] {
                        if fc_off as usize + 4 > hdr.len() {
                            continue;
                        }
                        let val = read_le32_at(&hdr, fc_off as usize);
                        if val > 0 && val < 200 {
                            // field_count is usually a small int
                            println!("           +0x{fc_off:02X} = {val} (possible field_count?)");
                        }
                    }

                    return;
                }
            }
        }
    }
}

fn scan_for_string(p: &ProcessHandle, start: usize, end: usize, bytes: &[u8]) -> Option<usize> {
    use hb_core::win32::query_memory_info;
    let mut addr = start;
    while addr < end {
        let mbi = match query_memory_info(p, addr) {
            Ok(m) => m,
            Err(_) => {
                addr += 0x10000;
                continue;
            }
        };
        let s = mbi.BaseAddress as usize;
        let sz = mbi.RegionSize;
        if mbi.State == 0x1000 && (mbi.Protect & 0x04) != 0 && sz < 0x400000 {
            // Read full region in 256KB chunks
            let mut off = 0usize;
            while off < sz {
                let rs = (0x40000).min(sz - off);
                if let Ok(data) = p.read_bytes(s + off, rs) {
                    if let Some(pos) = data.windows(bytes.len()).position(|w| w == bytes) {
                        return Some(s + off + pos);
                    }
                }
                off += 0x40000;
            }
        }
        addr = s + sz.max(0x10000);
        if addr > end {
            break;
        }
    }
    None
}

fn scan_for_references_full(
    p: &ProcessHandle,
    target: usize,
    start: usize,
    end: usize,
    refs: &mut Vec<usize>,
) {
    use hb_core::win32::query_memory_info;
    let mut addr = start;
    while addr < end {
        let mbi = match query_memory_info(p, addr) {
            Ok(m) => m,
            Err(_) => {
                addr += 0x10000;
                continue;
            }
        };
        let s = mbi.BaseAddress as usize;
        let sz = mbi.RegionSize;
        if mbi.State == 0x1000 && (mbi.Protect & 0x04) != 0 {
            let mut off = 0usize;
            while off < sz {
                let rs = (0x40000).min(sz - off);
                if let Ok(buf) = p.read_bytes(s + off, rs) {
                    for j in (0..buf.len() - 4).step_by(4) {
                        let val = u32::from_le_bytes([buf[j], buf[j + 1], buf[j + 2], buf[j + 3]])
                            as usize;
                        if val == target {
                            refs.push(s + off + j);
                        }
                    }
                }
                off += 0x40000;
            }
        }
        addr = s + sz.max(0x10000);
        if addr > end {
            break;
        }
    }
}

fn find_image_around(
    p: &ProcessHandle,
    target: &str,
    center: usize,
    range: usize,
) -> Option<usize> {
    use hb_core::win32::query_memory_info;
    let start = center.saturating_sub(range);
    let end = center.saturating_add(range).min(0x7FFFFFFF);
    let mut addr = start;
    while addr < end {
        let mbi = match query_memory_info(p, addr) {
            Ok(m) => m,
            Err(_) => {
                addr += 0x10000;
                continue;
            }
        };
        let s = mbi.BaseAddress as usize;
        let sz = mbi.RegionSize;
        if mbi.State == 0x1000 && ((mbi.Protect & 0x04) != 0) && sz < 0x200000 {
            if let Ok(data) = p.read_bytes(s, sz.min(0x20000)) {
                for i in (0..data.len().saturating_sub(0x28)).step_by(4) {
                    let np = u32::from_le_bytes([
                        data[i + 0x1C],
                        data[i + 0x1D],
                        data[i + 0x1E],
                        data[i + 0x1F],
                    ]) as usize;
                    let fp = u32::from_le_bytes([
                        data[i + 0x20],
                        data[i + 0x21],
                        data[i + 0x22],
                        data[i + 0x23],
                    ]) as usize;
                    if np <= 0x10000 || np > 0x7FFFFFFF || fp <= 0x10000 || fp > 0x7FFFFFFF {
                        continue;
                    }
                    if let Some(name) = read_string_opt(p, np, 48) {
                        if name == target {
                            return Some(s + i);
                        }
                    }
                }
            }
        }
        addr = s + sz.max(0x10000);
        if addr > end {
            break;
        }
    }
    None
}

fn scan_image_class_cache(p: &ProcessHandle, img_addr: usize) {
    let data = match p.read_bytes(img_addr, 0x100) {
        Ok(d) => d,
        Err(_) => {
            println!("   Can't read image");
            return;
        }
    };

    for off in (0..data.len().saturating_sub(12)).step_by(4) {
        let ptr =
            u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]) as usize;
        if ptr <= 0x10000 || ptr > 0x7FFFFFFF {
            continue;
        }

        // Try as GPtrArray: [data?][len][ptr_array]
        if let Ok(ad) = p.read_bytes(ptr, 16) {
            let len = u32::from_le_bytes([ad[4], ad[5], ad[6], ad[7]]) as usize;
            let arr = u32::from_le_bytes([ad[8], ad[9], ad[10], ad[11]]) as usize;
            if len > 0 && len < 5000 && arr > 0x10000 && arr < 0x7FFFFFFF {
                // Check first entry
                if let Ok(first) = p.read_memory::<u32>(arr) {
                    let f = first as usize;
                    if f > 0x10000 && f < 0x7FFFFFFF {
                        if let Some((ns, name)) = try_mono_class_fast(p, f, img_addr) {
                            println!("   MonoImage+0x{off:02X}: GPtrArray(len={len}) first='{ns}.{name}'");
                            // Count
                            let mut count = 1;
                            for i in 1..len.min(500) {
                                if let Ok(e) = p.read_memory::<u32>(arr + i * 4) {
                                    let e_addr = e as usize;
                                    if e_addr > 0x10000 && e_addr < 0x7FFFFFFF && e_addr != f {
                                        if let Some(_) = try_mono_class_fast(p, e_addr, img_addr) {
                                            count += 1;
                                        }
                                    }
                                }
                            }
                            println!("      Total valid classes: ~{count}");
                            return;
                        }
                    }
                }
            }
        }
    }
}

fn try_mono_class_fast(
    p: &ProcessHandle,
    addr: usize,
    img_addr: usize,
) -> Option<(String, String)> {
    let data = p.read_bytes(addr, 0x30).ok()?;

    for name_off in [0x04usize, 0x08, 0x0C, 0x10, 0x14, 0x18] {
        if name_off + 8 > data.len() {
            continue;
        }
        let name_ptr = u32::from_le_bytes([
            data[name_off],
            data[name_off + 1],
            data[name_off + 2],
            data[name_off + 3],
        ]) as usize;
        if name_ptr <= 0x10000 || name_ptr > 0x7FFFFFFF {
            continue;
        }

        let name = match read_string_opt(p, name_ptr, 32) {
            Some(n) => n,
            None => continue,
        };
        if name.len() < 2 || name.len() > 30 {
            continue;
        }
        if !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '`')
        {
            continue;
        }

        for ns_off in [0x00usize, 0x04, 0x08, 0x0C, 0x10, 0x14, 0x18] {
            if ns_off == name_off || ns_off + 4 > data.len() {
                continue;
            }
            let ns_ptr = u32::from_le_bytes([
                data[ns_off],
                data[ns_off + 1],
                data[ns_off + 2],
                data[ns_off + 3],
            ]) as usize;
            if ns_ptr <= 0x10000 || ns_ptr > 0x7FFFFFFF || ns_ptr == name_ptr {
                continue;
            }

            let ns = match read_string_opt(p, ns_ptr, 32) {
                Some(n) => n,
                None => continue,
            };
            if ns.len() < 2 || ns.len() > 30 || name.contains('\u{FFFD}') || ns.contains('\u{FFFD}')
            {
                continue;
            }
            if !ns
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
            {
                continue;
            }

            // Verify image pointer
            for img_off in [0x00usize, 0x04, 0x08, 0x0C, 0x10, 0x14] {
                if img_off == name_off || img_off == ns_off || img_off + 4 > data.len() {
                    continue;
                }
                let img_ptr = u32::from_le_bytes([
                    data[img_off],
                    data[img_off + 1],
                    data[img_off + 2],
                    data[img_off + 3],
                ]) as usize;
                if img_ptr == img_addr {
                    return Some((ns, name));
                }
            }
        }
    }
    None
}

fn read_le32_at(data: &[u8], off: usize) -> usize {
    if off + 4 > data.len() {
        return 0;
    }
    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]) as usize
}

fn read_string_opt(p: &ProcessHandle, addr: usize, max: usize) -> Option<String> {
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
