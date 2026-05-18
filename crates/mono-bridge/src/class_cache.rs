//! 从 MonoImage 的 class_cache 枚举所有类
//! 替代按字符串搜索（太慢且不可靠）

use hb_core::win32::ProcessHandle;
use crate::scanner::MonoImageInfo;

#[derive(Debug)]
pub struct MonoClassRaw {
    pub address: usize,
    pub name: String,
    pub namespace: String,
    pub image_addr: usize,
    pub class_size: u32,
    pub field_count: u32,
    pub fields_addr: usize,
}

/// 从 MonoImage 读 class_cache, 尝试多个偏移
pub fn enum_classes_from_image(process: &ProcessHandle, image: &MonoImageInfo) -> Vec<MonoClassRaw> {
    let mut classes = Vec::new();
    
    // 尝试 class_cache 偏移: 0x34, 0x38, 0x3C, 0x40
    for cache_off in &[0x34usize, 0x38, 0x3C, 0x40, 0x44, 0x48, 0x4C, 0x50] {
        let cache_ptr: u32 = match process.read_memory(image.address + cache_off) {
            Ok(p) if p > 0x00010000 && p < 0x7FFFFFFF => p,
            _ => continue,
        };
        
        // 尝试作为 GSList 遍历
        let mut node = cache_ptr as usize;
        let mut count = 0u32;
        loop {
            count += 1;
            if count > 2000 { break; }
            
            // GSList.data = MonoClass*
            let class_ptr: u32 = match process.read_memory(node) {
                Ok(p) if p > 0x00010000 && p < 0x7FFFFFFF => p,
                _ => break,
            };
            
            // 读类名
            if let Some(info) = read_mono_class_info(process, class_ptr as usize) {
                if !classes.iter().any(|c: &MonoClassRaw| c.address == class_ptr as usize) {
                    classes.push(info);
                }
            }
            
            // GSList.next
            let next: u32 = match process.read_memory(node + 4) {
                Ok(n) if n > 0x00010000 && n < 0x7FFFFFFF && n != node as u32 => n,
                _ => break,
            };
            node = next as usize;
        }
        
        if !classes.is_empty() {
            break;
        }
    }
    classes
}

/// 从 MonoClass 地址读取基本信息
pub fn read_mono_class_info(process: &ProcessHandle, addr: usize) -> Option<MonoClassRaw> {
    // 尝试两个常见的 name 偏移
    for &name_off in &[0x08usize, 0x0C, 0x10] {
        let name_ptr: u32 = match process.read_memory(addr + name_off) {
            Ok(p) if p > 0x00010000 && p < 0x7FFFFFFF => p,
            _ => continue,
        };
        if let Ok(bytes) = process.read_bytes(name_ptr as usize, 48) {
            let nlen = bytes.iter().position(|&b| b == 0).unwrap_or(48);
            if !(2..=40).contains(&nlen) { continue; }
            let name = String::from_utf8_lossy(&bytes[..nlen]).to_string();
            if name.contains('\u{FFFD}') || name.contains('\0') { continue; }
            
            // 读 namespace (通常在 name 后面 4 字节)
            let ns_off = name_off + 4;
            let ns_ptr: u32 = match process.read_memory(addr + ns_off) {
                Ok(p) if p > 0x00010000 && p < 0x7FFFFFFF => p,
                _ => 0,
            };
            let namespace = if ns_ptr > 0 {
                if let Ok(b) = process.read_bytes(ns_ptr as usize, 48) {
                    let l = b.iter().position(|&x| x == 0).unwrap_or(48);
                    String::from_utf8_lossy(&b[..l]).to_string()
                } else { String::new() }
            } else { String::new() };
            
            // 验证: 必须是合法的类名（只含 ASCII）
            if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '+' || c == '.') {
                continue;
            }
            
            let image_addr = process.read_memory::<u32>(addr).unwrap_or(0) as usize;
            let image_addr2 = process.read_memory::<u32>(addr + 4).unwrap_or(0) as usize;
            let class_size = process.read_memory::<u32>(addr + 0x2C).unwrap_or(0);
            let field_count = process.read_memory::<u32>(addr + 0x34).unwrap_or(0);
            let fields_addr = process.read_memory::<u32>(addr + 0x38).unwrap_or(0) as usize;
            
            return Some(MonoClassRaw {
                address: addr,
                name,
                namespace,
                image_addr: if image_addr > 0 { image_addr } else { image_addr2 },
                class_size,
                field_count,
                fields_addr,
            });
        }
    }
    None
}
