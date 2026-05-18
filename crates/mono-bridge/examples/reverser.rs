//! Mono 运行时暴力逆向工具
//!
//! 策略：逐步扫描 Hearthstone 进程内存，通过已知锚点
//! 反推出 MonoImage → class_cache → MonoClass → name 的各级偏移。
//!
//! 步骤:
//!   1. 枚举所有 MonoImage (重用 scanner)
//!   2. 从 Assembly-CSharp 映像读取原始字节
//!   3. 扫描所有可能的 pointer 字段 → 寻找 class_cache
//!   4. 从 class_cache 获取所有 MonoClass
//!   5. 暴力试探 MonoClass.name 偏移
//!   6. 验证: 读到的类名中包含 "SceneMgr" / "GameState" / "Entity"

use hb_core::win32::query_memory_info;
use hb_core::win32::ProcessHandle;
use std::collections::HashSet;

fn main() {
    println!("=== Mono Runtime Reverser ===\n");

    // 1. 找 Hearthstone 进程
    let hs = find_hearthstone();
    let p = match hs {
        Some(p) => p,
        None => {
            println!("❌ Hearthstone not running. Start it first.");
            return;
        }
    };
    println!("✅ Hearthstone PID={}\n", p.pid);

    // 2. 使用已知的 corlib MonoImage 地址（从 scan 示例获取）
    println!("[Step 1] Finding Assembly-CSharp via known corlib anchor...");

    let corlib_addr: usize = 0x969ea60;
    let corlib_name = read_string_opt(
        &p,
        p.read_memory::<u32>(corlib_addr + 0x1C).unwrap_or(0) as usize,
        32,
    )
    .unwrap_or_default();
    println!("   corlib @ 0x{corlib_addr:08x} name='{corlib_name}'");

    let search_start = corlib_addr.saturating_sub(0x200000);
    let search_end = corlib_addr.saturating_add(0x200000).min(0x7FFFFFFF);

    let img_addr = find_image_in_range(&p, "Assembly-CSharp", search_start, search_end)
        .or_else(|| find_image_in_range(&p, "Assembly", search_start, search_end));

    let img_addr = match img_addr {
        Some(addr) => {
            let name = read_string_opt(
                &p,
                p.read_memory::<u32>(addr + 0x1C).unwrap_or(0) as usize,
                48,
            )
            .unwrap_or_default();
            println!("   ✅ Assembly-CSharp @ 0x{addr:08x} (name='{name}')");
            addr
        }
        None => {
            println!("❌ Assembly-CSharp not found near corlib!");
            return;
        }
    };

    let img = hb_mono_bridge::scanner::MonoImageInfo {
        address: img_addr,
        name: String::new(),
        filename: String::new(),
    };

    // 3. 读取 MonoImage 结构体 + 0x100 字节
    println!("\n[Step 2] Dumping MonoImage raw bytes...");
    let image_data = match p.read_bytes(img.address, 0x100) {
        Ok(d) => d,
        Err(e) => {
            println!("❌ Failed to read MonoImage: {e}");
            return;
        }
    };

    dump_hex(&image_data, img.address, "MonoImage");

    // 4. 找出所有可能的指针字段
    println!("\n[Step 3] Finding potential pointer fields (10+ candidates)...");
    let pointers = find_pointer_fields(&p, &image_data, img.address);
    println!("   {} potential pointer fields found", pointers.len());
    for (offset, target, label) in &pointers[..10.min(pointers.len())] {
        println!("   +0x{offset:02X} → 0x{target:08x} {label}");
    }
    if pointers.len() > 10 {
        println!("   ... and {} more", pointers.len() - 10);
    }

    // 5. 从每个指针字段，尝试找到 class_cache（GHashTable 或 GPtrArray）
    println!("\n[Step 4] Looking for class_cache / type_table...");
    let mut best_caches: Vec<(usize, String, Vec<(String, String)>)> = Vec::new();

    for &(offset, target, ref label) in &pointers {
        if label != "<data>" {
            continue;
        }
        if let Some(classes) = try_read_class_cache(&p, target, &image_data, img.address) {
            if classes.len() > 10 {
                println!(
                    "   ✅ +0x{offset:02X} (cache ptr 0x{target:08x}) → {} classes!",
                    classes.len()
                );
                // 打印前 5 个
                for (ns, name) in classes.iter().take(5) {
                    println!("      {ns}.{name}");
                }
                best_caches.push((offset, format!("type_table @ 0x{target:x}"), classes));
                if best_caches.len() >= 3 {
                    break;
                }
            }
        }
    }

    let cache = best_caches.first();
    match cache {
        Some((off, desc, classes)) => {
            println!("\n   🎯 Best class_cache at +0x{off:02X} -> {desc}");
            println!("   Found {} classes", classes.len());

            // 6. 验证关键类
            let known_classes = ["SceneMgr", "GameState", "Entity", "GameStateManager"];
            for kc in &known_classes {
                if classes.iter().any(|(_, n)| n == kc) {
                    println!("   ✅ Found: {kc}");
                } else {
                    println!("   ❌ Missing: {kc}");
                }
            }

            // 7. 输出所有类（方便手动查找）
            println!("\n[Step 5] Class dump (first 50):");
            for (i, (ns, name)) in classes.iter().enumerate().take(50) {
                println!("   [{i:3}] {ns}.{name}");
            }
            if classes.len() > 50 {
                println!("   ... and {} more", classes.len() - 50);
            }

            // 8. 写入 class list 文件
            let out_path = "target/mono_classes.txt";
            let mut out = String::new();
            out.push_str(&format!("class_cache_offset=0x{off:02X}\n\n"));
            for (ns, name) in classes {
                out.push_str(&format!("{ns}.{name}\n"));
            }
            std::fs::write(out_path, &out).ok();
            println!("\n   💾 Class list saved to: {out_path}");
        }
        None => {
            println!("❌ Could not find class_cache or type_table");
            println!("\nTrying alternative: brute-force MonoClass name field...");
            brute_force_mono_class_name(&p, &image_data, img.address);
        }
    }
}

/// 在指定范围搜索 MonoImage
fn find_image_in_range(p: &ProcessHandle, target: &str, start: usize, end: usize) -> Option<usize> {
    let chunk = 0x10000usize; // 64KB

    let mut addr = start;
    while addr < end {
        let mbi = match query_memory_info(p, addr) {
            Ok(m) => m,
            Err(_) => {
                addr += 0x10000;
                continue;
            }
        };
        let region_start = mbi.BaseAddress as usize;
        let region_size = mbi.RegionSize;
        let committed = mbi.State == 0x1000;
        let readable = (mbi.Protect & 0x02) != 0 || (mbi.Protect & 0x04) != 0;

        if committed && readable && region_size > 0x1000 && region_size < 0x800000 {
            let mut off = 0usize;
            while off < region_size {
                let read_size = chunk.min(region_size - off);
                if let Ok(data) = p.read_bytes(region_start + off, read_size) {
                    for i in (0..data.len().saturating_sub(0x28)).step_by(4) {
                        let name_ptr = u32::from_le_bytes([
                            data[i + 0x1C],
                            data[i + 0x1C + 1],
                            data[i + 0x1C + 2],
                            data[i + 0x1C + 3],
                        ]) as usize;
                        if !(0x00010000..0x7FFFFFFF).contains(&name_ptr) {
                            continue;
                        }
                        if let Some(name) = read_string_opt(p, name_ptr, 48) {
                            if name.contains(target) {
                                let abs_addr = region_start + off + i;
                                let file_ptr = u32::from_le_bytes([
                                    data[i + 0x20],
                                    data[i + 0x20 + 1],
                                    data[i + 0x20 + 2],
                                    data[i + 0x20 + 3],
                                ]) as usize;
                                if (0x00010000..0x7FFFFFFF).contains(&file_ptr) {
                                    return Some(abs_addr);
                                }
                            }
                        }
                    }
                }
                off += chunk;
            }
        }
        addr = region_start + region_size.max(0x10000);
        if addr > end {
            break;
        }
    }
    None
}

/// 找 Hearthstone 进程
fn find_hearthstone() -> Option<ProcessHandle> {
    for name in &["Hearthstone", "Hearthstone.exe"] {
        if let Ok(p) = ProcessHandle::find_by_name(name) {
            return Some(p);
        }
    }
    None
}

/// 找出 MonoImage 中所有可能是指针的 u32 字段
fn find_pointer_fields(
    p: &ProcessHandle,
    data: &[u8],
    _base: usize,
) -> Vec<(usize, usize, String)> {
    let mut pointers = Vec::new();

    for off in (0..data.len() - 4).step_by(4) {
        let val =
            u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]) as usize;

        if !(0x00010000..0x7FFFFFFF).contains(&val) {
            continue;
        }

        let label = compute_label(p, val);
        pointers.push((off, val, label));
    }

    pointers.sort_by_key(|(off, _, _)| *off);
    pointers
}

/// 计算指针标签（返回 String 避免生命周期问题）
fn compute_label(p: &ProcessHandle, val: usize) -> String {
    let b = match p.read_bytes(val, 8) {
        Ok(b) => b,
        Err(_) => return "<inaccessible>".into(),
    };

    if b[0] == 0 {
        return "<nullstr>".into();
    }

    // 是否为空终止 ASCII 字符串？
    if b.iter()
        .all(|&c| c.is_ascii_graphic() || c == b' ' || c == b'\0')
    {
        let len = b.iter().position(|&c| c == 0).unwrap_or(8);
        if len >= 3 && len <= 30 {
            let s = String::from_utf8_lossy(&b[..len]).to_string();
            if s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' || c == ' ')
            {
                if val > 0x01000000 && is_pointer_array(p, val) {
                    return "<ptr_array>".into();
                }
                return s;
            }
        }
        return "<data>".into();
    }

    // 如果 b[0..4] 是有效指针 → 多层指针
    let ptr2 = u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize;
    if (0x00010000..0x7FFFFFFF).contains(&ptr2) {
        if let Ok(b2) = p.read_bytes(ptr2, 4) {
            let ptr3 = u32::from_le_bytes([b2[0], b2[1], b2[2], b2[3]]) as usize;
            if (0x00010000..0x7FFFFFFF).contains(&ptr3) {
                return "<ptr_table>".into();
            }
        }
        return "<ptr>".into();
    }

    "<data>".into()
}

/// 检查某个地址是否为指针数组
fn is_pointer_array(p: &ProcessHandle, addr: usize) -> bool {
    if let Ok(data) = p.read_bytes(addr, 0x40) {
        let ptr_count = data.len() / 4;
        let mut valid_ptrs = 0;
        for i in 0..ptr_count.min(16) {
            let v = u32::from_le_bytes([
                data[i * 4],
                data[i * 4 + 1],
                data[i * 4 + 2],
                data[i * 4 + 3],
            ]) as usize;
            if (0x00010000..0x7FFFFFFF).contains(&v) {
                valid_ptrs += 1;
            }
        }
        valid_ptrs >= 4
    } else {
        false
    }
}

/// 尝试将某个地址解析为 class_cache (GHashTable 或 GPtrArray)
/// 返回找到的 (namespace, class_name) 列表
fn try_read_class_cache(
    p: &ProcessHandle,
    addr: usize,
    img_data: &[u8],
    img_addr: usize,
) -> Option<Vec<(String, String)>> {
    // 策略 1: 尝试当 GHashTable 解析
    if let Some(classes) = try_as_ghashtable(p, addr, img_addr) {
        return Some(classes);
    }

    // 策略 2: 尝试当 GPtrArray 解析
    if let Some(classes) = try_as_gptrarray(p, addr, img_addr) {
        return Some(classes);
    }

    None
}

/// GHashTable:
///   +0x00: size (int)
///   +0x04: mod (int)
///   +0x08: num_hash (int)
///   +0x0C: num_dirty (int)
///   +0x10: num_entries (int)
///   +0x14: nodes (GHashNode*)  ← 关键字段
///
/// GHashNode:
///   +0x00: key (void*)   ← 通常为 const char* (类名字符串)
///   +0x04: value (void*) ← MonoClass*
///   +0x08: hash (int)
fn try_as_ghashtable(
    p: &ProcessHandle,
    addr: usize,
    _img_addr: usize,
) -> Option<Vec<(String, String)>> {
    let data = p.read_bytes(addr, 0x20).ok()?;

    let size = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let _mod = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let _num_hash = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
    let num_entries = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
    let nodes = u32::from_le_bytes([data[20], data[21], data[22], data[23]]) as usize;

    // 合理性检查
    if !(1..=50000).contains(&num_entries) {
        return None;
    }
    if !(0x00010000..0x7FFFFFFF).contains(&nodes) {
        return None;
    }
    if size < num_entries || size > 100000 {
        return None;
    }

    // 读节点数组
    let node_size = 12; // sizeof(GHashNode) = 12
    let max_read = (num_entries as usize) * node_size;
    if max_read > 0x200000 {
        return None;
    }

    let node_data = p.read_bytes(nodes, max_read.min(0x10000)).ok()?;
    let mut classes = Vec::new();

    for i in 0..num_entries.min(1000) as usize {
        let off = i * node_size;
        if off + 8 >= node_data.len() {
            break;
        }
        let key = u32::from_le_bytes([
            node_data[off],
            node_data[off + 1],
            node_data[off + 2],
            node_data[off + 3],
        ]) as usize;
        let _value = u32::from_le_bytes([
            node_data[off + 4],
            node_data[off + 5],
            node_data[off + 6],
            node_data[off + 7],
        ]) as usize;

        if !(0x00010000..0x7FFFFFFF).contains(&key) {
            continue;
        }

        // key 可能是 "Class.Name" 格式的完整类名
        if let Some(class_name) = read_string_opt(p, key, 64) {
            if class_name.contains('.') && class_name.len() > 3 && class_name.len() < 60 {
                let parts: Vec<&str> = class_name.splitn(2, '.').collect();
                if parts.len() == 2 {
                    let ns = parts[0].to_string();
                    let name = parts[1].to_string();
                    if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                        && ns
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
                    {
                        classes.push((ns, name));
                    }
                }
            }
        }
    }

    if classes.len() > 5 {
        Some(classes)
    } else {
        None
    }
}

/// GPtrArray:
///   +0x00: GHashTable* (optional base)
///   +0x04: len (int)
///   +0x08: data (void**)  ← 指针数组
fn try_as_gptrarray(
    p: &ProcessHandle,
    addr: usize,
    _img_addr: usize,
) -> Option<Vec<(String, String)>> {
    let data = p.read_bytes(addr, 0x10).ok()?;
    if data.len() < 12 {
        return None;
    }

    let len = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
    let arr = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;

    if !(1..=50000).contains(&len) {
        return None;
    }
    if !(0x00010000..0x7FFFFFFF).contains(&arr) {
        return None;
    }

    // 读指针数组
    let arr_size = len * 4;
    if arr_size > 0x200000 {
        return None;
    }
    let arr_data = p.read_bytes(arr, arr_size.min(0x2000)).ok()?;

    let mut classes = Vec::new();
    for i in 0..len.min(500) as usize {
        if i * 4 + 4 > arr_data.len() {
            break;
        }
        let ptr = u32::from_le_bytes([
            arr_data[i * 4],
            arr_data[i * 4 + 1],
            arr_data[i * 4 + 2],
            arr_data[i * 4 + 3],
        ]) as usize;

        if !(0x00010000..0x7FFFFFFF).contains(&ptr) {
            continue;
        }

        // 尝试作为 MonoClass 读取
        if let Some(class_info) = try_read_mono_class(p, ptr) {
            classes.push(class_info);
        }
    }

    if classes.len() > 5 {
        Some(classes)
    } else {
        None
    }
}

/// 尝试读取一个 MonoClass 结构（暴力试探 name 偏移）
fn try_read_mono_class(p: &ProcessHandle, addr: usize) -> Option<(String, String)> {
    // 先看指向的内存是否可读
    let head = p.read_bytes(addr, 0x40).ok()?;

    // 试探 name 偏移: 尝试 0x08, 0x0C, 0x10, 0x14, 0x18, 0x1C
    let candidate_name_offsets = [0x08, 0x0C, 0x10, 0x14, 0x18, 0x1C, 0x04, 0x20, 0x24];

    for &name_off in &candidate_name_offsets {
        if name_off + 4 > head.len() {
            continue;
        }
        let name_ptr = u32::from_le_bytes([
            head[name_off],
            head[name_off + 1],
            head[name_off + 2],
            head[name_off + 3],
        ]) as usize;

        if !(0x00010000..0x7FFFFFFF).contains(&name_ptr) {
            continue;
        }

        if let Some(class_name) = read_string_opt(p, name_ptr, 48) {
            // 试探 ns 偏移
            for &ns_off in &candidate_name_offsets {
                if ns_off == name_off || ns_off + 4 > head.len() {
                    continue;
                }
                let ns_ptr = u32::from_le_bytes([
                    head[ns_off],
                    head[ns_off + 1],
                    head[ns_off + 2],
                    head[ns_off + 3],
                ]) as usize;

                if !(0x00010000..0x7FFFFFFF).contains(&ns_ptr) {
                    continue;
                }

                if let Some(ns) = read_string_opt(p, ns_ptr, 48) {
                    // 验证: 类名应短、合理
                    if class_name.len() >= 2
                        && class_name.len() < 40
                        && ns.len() >= 2
                        && ns.len() < 40
                        && class_name
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '`')
                        && ns
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
                        && !class_name.starts_with("_")
                        && !class_name.starts_with("<")
                    {
                        return Some((ns.to_string(), class_name.to_string()));
                    }
                }
            }
        }
    }
    None
}

/// 暴力搜索 MonoClass name 字段偏移
/// 策略：从 MonoImage 地址附近搜索所有可能指向字符串的指针
fn brute_force_mono_class_name(p: &ProcessHandle, _img_data: &[u8], img_addr: usize) {
    println!("\n   Scanning memory near MonoImage for pointers to strings...");

    let mut name_candidates: HashSet<usize> = HashSet::new();

    // 扫描 MonoImage 前后各 0x10000
    let scan_start = img_addr.saturating_sub(0x10000);
    let scan_end = img_addr.saturating_add(0x10000).min(0x7FFFFFFF);

    let chunk_size = 0x10000usize;
    let mut addr = scan_start;
    while addr < scan_end {
        if let Ok(data) = p.read_bytes(addr, chunk_size) {
            for off in (0..data.len() - 12).step_by(4) {
                let ptr =
                    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
                        as usize;
                if !(0x00010000..0x7FFFFFFF).contains(&ptr) {
                    continue;
                }
                // 检查 ptr 指向的内容是否像类名
                if let Some(s) = read_string_opt(p, ptr, 32) {
                    if s.len() >= 2
                        && s.len() < 30
                        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                    {
                        let source_addr = addr + off;
                        // 只关注在 MonoImage 范围内的
                        if source_addr >= img_addr && source_addr < img_addr + 0x80 {
                            name_candidates.insert(source_addr - img_addr);
                            println!(
                                "     MonoImage+0x{:03X} -> '{s}' @ 0x{ptr:x}",
                                source_addr - img_addr
                            );
                        }
                    }
                }
            }
        }
        addr += chunk_size;
    }

    if !name_candidates.is_empty() {
        println!(
            "\n   🎯 Found {} potential class name pointers in MonoImage!",
            name_candidates.len()
        );
        println!("   These offsets could be MonoClass.name fields");
    }
}

/// 读取远程字符串（安全版本）
fn read_string_opt(p: &ProcessHandle, addr: usize, max_len: usize) -> Option<String> {
    let bytes = p.read_bytes(addr, max_len).ok()?;
    let len = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    if len < 2 || len > max_len - 1 {
        return None;
    }
    let s = String::from_utf8_lossy(&bytes[..len]).to_string();
    if s.contains('\u{FFFD}') {
        return None;
    }
    Some(s)
}

/// 读取远程字符串（直接返回 String）
fn read_string(p: &ProcessHandle, addr: usize, max_len: usize) -> String {
    read_string_opt(p, addr, max_len).unwrap_or_default()
}

/// 16 进制转储
fn dump_hex(data: &[u8], base: usize, label: &str) {
    println!("   {label} @ 0x{base:08x}:");
    for row in 0..data.len() / 16 {
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
