//! Mono 运行时内部结构解析
//!
//! 从 MonoImage → MonoClass → field info → heap instance
//! 替代 GreyMagic 的核心逻辑

use crate::scanner::MonoImageInfo;
use hb_core::win32::query_memory_info;
use hb_core::win32::ProcessHandle;

/// MonoClass 结构探查结果
#[derive(Debug)]
pub struct MonoClassInfo {
    pub address: usize,
    pub name: String,
    pub namespace: String,
    pub class_size: u32,
    pub image_addr: usize,
    pub field_count: u32,
    pub fields_addr: usize,
    pub parent_addr: usize,
}

/// 字段信息
#[derive(Debug)]
pub struct FieldInfo {
    pub name: String,
    pub type_: String,
    pub offset: u32,
    pub is_static: bool,
}

/// 在进程内存中找 MonoClass 结构
/// 多偏移试探: name 可能在 +0x08 / +0x0C / +0x10
pub fn find_class_by_name(process: &ProcessHandle, class_name: &str) -> Option<MonoClassInfo> {
    let name_addr = scan_string(process, class_name)?;
    eprintln!("    String '{}' @ 0x{name_addr:x}", class_name);

    // 尝试的 name 偏移和对应的 namespace 偏移
    let tries: &[(usize, usize)] = &[
        (0x08, 0x0C),  // 标准 MonoClass layout
        (0x0C, 0x10),  // 变体1
        (0x10, 0x14),  // 变体2
        (0x04, 0x08),  // 变体3
    ];
    
    // 扫描范围：前后各 0x10000 字节
    let scan_start = name_addr.saturating_sub(0x10000);
    let scan_end = name_addr.saturating_add(0x10000).min(0x7FFFFFFF);
    
    for &(name_off, ns_off) in tries {
        for candidate in (scan_start..scan_end).step_by(4) {
            let Ok(ptr) = process.read_memory::<u32>(candidate + name_off) else { continue };
            if ptr as usize != name_addr { continue; }
            
            // Namespace
            let Ok(ns_ptr) = process.read_memory::<u32>(candidate + ns_off) else { continue };
            if !(0x00010000..0x7FFFFFFF).contains(&ns_ptr) { continue; }
            let Ok(nbytes) = process.read_bytes(ns_ptr as usize, 48) else { continue };
            let nlen = nbytes.iter().position(|&b| b == 0).unwrap_or(48);
            if nlen == 0 || nlen >= 40 { continue; }
            let ns = String::from_utf8_lossy(&nbytes[..nlen]);
            if ns.contains('\u{FFFD}') { continue; }
            
            // Image (尝试两个偏移)
            let img0: u32 = process.read_memory(candidate).unwrap_or(0);
            let img4: u32 = process.read_memory(candidate + 4).unwrap_or(0);
            let image_addr = if (0x00010000..0x7FFFFFFF).contains(&img0) { img0 as usize }
                else if (0x00010000..0x7FFFFFFF).contains(&img4) { img4 as usize }
                else { continue; };
            
            let class_size = process.read_memory::<u32>(candidate + 0x2C).unwrap_or(0);
            let field_count = process.read_memory::<u32>(candidate + 0x34).unwrap_or(0);
            let fields_addr = process.read_memory::<u32>(candidate + 0x38).unwrap_or(0) as usize;
            let parent_addr = process.read_memory::<u32>(candidate + 0x10).unwrap_or(0) as usize;
            
            eprintln!("    Candidate @ 0x{candidate:x} name_off=0x{name_off:x} ns='{ns}' img=0x{image_addr:x}");
            
            return Some(MonoClassInfo {
                address: candidate, name: class_name.to_string(), namespace: ns.to_string(),
                class_size, image_addr, field_count, fields_addr, parent_addr,
            });
        }
    }
    None
}

/// 从 MonoClass 读取所有字段信息
pub fn read_class_fields(process: &ProcessHandle, class: &MonoClassInfo) -> Vec<FieldInfo> {
    let mut fields = Vec::new();
    if class.fields_addr == 0 || class.field_count == 0 {
        return fields;
    }

    // MonoClassField 结构 (x86):
    // +0x00: name (char*)
    // +0x04: type (MonoType*)
    // +0x08: parent (MonoClassField*)
    // +0x0C: offset (int)
    // +0x10: flags (int)

    for i in 0..class.field_count as usize {
        let field_addr = class.fields_addr + i * 0x14; // sizeof(MonoClassField) ≈ 20
        if let Ok(name_ptr) = process.read_memory::<u32>(field_addr) {
            if !(0x00010000..0x7FFFFFFF).contains(&name_ptr) {
                continue;
            }
            if let Ok(bytes) = process.read_bytes(name_ptr as usize, 48) {
                let nlen = bytes.iter().position(|&b| b == 0).unwrap_or(48);
                let name = String::from_utf8_lossy(&bytes[..nlen]).to_string();
                if name.contains('\u{FFFD}') {
                    continue;
                }

                let offset = process.read_memory::<u32>(field_addr + 0x0C).unwrap_or(0);
                let flags = process.read_memory::<u32>(field_addr + 0x10).unwrap_or(0);

                // 读取类型
                let type_addr = process.read_memory::<u32>(field_addr + 0x04).unwrap_or(0) as usize;
                let type_str = if type_addr > 0 {
                    read_type_name(process, type_addr)
                } else {
                    "unknown".into()
                };

                fields.push(FieldInfo {
                    name,
                    type_: type_str,
                    offset,
                    is_static: (flags & 0x10) != 0,
                });
            }
        }
        if fields.len() >= 100 {
            break;
        } // 安全限制
    }
    fields
}

/// 读取 MonoType 的名称
fn read_type_name(process: &ProcessHandle, type_addr: usize) -> String {
    // MonoType +0x00: attrs (u32)
    // MonoType +0x04: type (u32) - 基本类型
    // MonoType +0x08: data (MonoClass*) - 对于 CLASS/VALUETYPE
    let type_val = process.read_memory::<u32>(type_addr + 0x04).unwrap_or(0xFF);
    match type_val {
        0 => "Object".into(),
        1 => "Object".into(),
        2 => "bool".into(),
        3 => "char".into(),
        4 => "i8".into(),
        5 => "u8".into(),
        6 => "i16".into(),
        7 => "u16".into(),
        8 => "i32".into(),
        9 => "u32".into(),
        10 => "i64".into(),
        11 => "u64".into(),
        12 => "f32".into(),
        13 => "f64".into(),
        14 => "String".into(),
        17 => "IntPtr".into(),
        24 => "i32".into(), // enum
        _ => {
            // 尝试读 class name
            if let Ok(class_ptr) = process.read_memory::<u32>(type_addr + 0x08) {
                if (0x00010000..0x7FFFFFFF).contains(&class_ptr) {
                    if let Ok(name_ptr) = process.read_memory::<u32>(class_ptr as usize + 0x08) {
                        if let Ok(b) = process.read_bytes(name_ptr as usize, 32) {
                            let l = b.iter().position(|&x| x == 0).unwrap_or(32);
                            return String::from_utf8_lossy(&b[..l]).to_string();
                        }
                    }
                }
            }
            format!("type_{}", type_val)
        }
    }
}

/// 在进程内存中扫描字符串
pub fn scan_string(process: &ProcessHandle, target: &str) -> Option<usize> {
    let bytes = target.as_bytes();
    let mut addr: usize = 0x00010000;
    while addr < 0x7FFFFFFF {
        if let Ok(data) = process.read_bytes(addr, 0x10000) {
            if let Some(pos) = data.windows(bytes.len()).position(|w| w == bytes) {
                return Some(addr + pos);
            }
        }
        // 用 VirtualQuery 跳过不可读区域
        if let Ok(mbi) = query_memory_info(process, addr) {
            let s = mbi.BaseAddress as usize;
            let sz = mbi.RegionSize;
            addr = s + sz.max(0x10000);
        } else {
            addr += 0x10000;
        }
    }
    None
}

/// 从已知的类名和命名空间创建完整类信息
pub fn resolve_class(
    process: &ProcessHandle,
    image: &MonoImageInfo,
    namespace: &str,
    class_name: &str,
) -> Option<MonoClassInfo> {
    // 方法1: 从 MonoImage 的 class_cache 遍历
    // 方法2: 从类名字符串扫描 (当前实现)
    if let Some(info) = find_class_by_name(process, class_name) {
        // 验证 image 地址匹配
        if info.image_addr == image.address {
            return Some(info);
        }
        // 有时 MonoClass.image 指向的是不同的偏移
        // 只要命名空间匹配就接受
        if info.namespace == namespace {
            return Some(info);
        }
    }
    None
}

// ===== 实例读取 =====

/// 从 Mono 堆上找到类的所有实例
/// 通过扫描 GC 堆，检查每个对象的 vtable→class 是否匹配
pub fn find_instances(process: &ProcessHandle, class: &MonoClassInfo) -> Vec<usize> {
    let mut instances = Vec::new();

    // 扫描 GC heap (0x01000000-0x40000000 一般是堆范围)
    let mut addr: usize = 0x01000000;
    while addr < 0x40000000 {
        if let Ok(mbi) = query_memory_info(process, addr) {
            let start = mbi.BaseAddress as usize;
            let size = mbi.RegionSize;
            let committed = mbi.State == 0x1000;
            let readable = (mbi.Protect & 0x04) != 0; // PAGE_READWRITE

            if committed && readable && size < 0x1000000 && size > 8 {
                // 逐 4 字节扫描堆
                let chunk_size = 0x10000usize;
                let mut off = 0usize;
                while off < size {
                    let rs = chunk_size.min(size - off);
                    if let Ok(data) = process.read_bytes(start + off, rs) {
                        for i in (0..data.len().saturating_sub(8)).step_by(4) {
                            // 每个对象的第一个字段是 vtable 指针
                            let vtable = u32::from_le_bytes([
                                data[i],
                                data[i + 1],
                                data[i + 2],
                                data[i + 3],
                            ]) as usize;

                            if !(0x01000000..0x80000000).contains(&vtable) {
                                continue;
                            }

                            // vtable → mono_class (vtable+0x00 通常是 MonoClass*)
                            if let Ok(class_addr) = process.read_memory::<u32>(vtable) {
                                if class_addr as usize == class.address {
                                    let obj_addr = start + off + i;
                                    instances.push(obj_addr);
                                }
                            }
                        }
                    }
                    off += chunk_size;
                }
            }
            addr = start + size;
            if size == 0 {
                addr += 0x10000;
            }
        } else {
            addr += 0x10000;
        }
    }
    instances
}

/// 从对象读取字段值
pub fn read_field_i32(process: &ProcessHandle, obj_addr: usize, field: &FieldInfo) -> Option<i32> {
    if field.is_static {
        return None;
    } // TODO: 静态字段
    process
        .read_memory::<i32>(obj_addr + field.offset as usize)
        .ok()
}

pub fn read_field_bool(
    process: &ProcessHandle,
    obj_addr: usize,
    field: &FieldInfo,
) -> Option<bool> {
    if field.is_static {
        return None;
    }
    process
        .read_memory::<u8>(obj_addr + field.offset as usize)
        .ok()
        .map(|v| v != 0)
}

pub fn read_field_ptr(
    process: &ProcessHandle,
    obj_addr: usize,
    field: &FieldInfo,
) -> Option<usize> {
    if field.is_static {
        return None;
    }
    process
        .read_memory::<u32>(obj_addr + field.offset as usize)
        .ok()
        .map(|v| v as usize)
}
