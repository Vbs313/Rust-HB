//! 进程内存扫描器
//!
//! 通过 VirtualQueryEx 枚举所有内存页 → 扫描 MonoImage 签名
//! 替代 CreateRemoteThread 方案（被 ACG 阻止）

use hb_core::win32::ProcessHandle;
use hb_core::win32::query_memory_info;

// MonoImage 偏移: +0x1C = 名称字符串指针, +0x20 = 文件名指针
const MONO_IMAGE_NAME_OFF: usize = 0x1C;
const MONO_IMAGE_FILE_OFF: usize = 0x20;

/// 找到目标进程中的 MonoImage
pub struct MonoImageInfo {
    pub address: usize,
    pub name: String,
    pub filename: String,
}

/// 枚举所有 Assembly-CSharp 等关键映像
pub fn find_all_images(process: &ProcessHandle) -> Vec<MonoImageInfo> {
    let mut images = Vec::new();
    let mut addr: usize = 0x00010000;

    while addr < 0x7FFFFFFF {
        let mbi = match query_memory_info(process, addr) {
            Ok(m) => m,
            Err(_) => { addr += 0x10000; continue; }
        };

        // 只看已提交、可读的内存页
        let is_committed = mbi.State == 0x1000; // MEM_COMMIT
        let is_readable = (mbi.Protect & 0x02) != 0 || (mbi.Protect & 0x04) != 0;
        let start = mbi.BaseAddress as usize;
        let size = mbi.RegionSize;

        if is_committed && is_readable {
            let mut offset = 0usize;
            while offset + MONO_IMAGE_NAME_OFF + 4 < size {
                let scan_addr = start + offset;
                
                // 读取 +0x1C 处的指针
                if let Ok(name_ptr) = process.read_memory::<u32>(scan_addr + MONO_IMAGE_NAME_OFF) {
                    if (0x00010000..0x7FFFFFFF).contains(&name_ptr) {
                        // 读名称字符串
                        if let Ok(bytes) = process.read_bytes(name_ptr as usize, 32) {
                            let len = bytes.iter().position(|&b| b == 0).unwrap_or(32);
                            if len >= 3 {
                                let name = String::from_utf8_lossy(&bytes[..len]).to_string();
                                
                                // 检查是否是有效的 MonoImage 名称
                                if let Ok(file_ptr) = process.read_memory::<u32>(scan_addr + MONO_IMAGE_FILE_OFF) {
                                    if (0x00010000..0x7FFFFFFF).contains(&file_ptr) {
                                        if let Ok(fbytes) = process.read_bytes(file_ptr as usize, 32) {
                                            let flen = fbytes.iter().position(|&b| b == 0).unwrap_or(32);
                                            let filename = String::from_utf8_lossy(&fbytes[..flen]).to_string();
                                            
                                            // 验证：名称应包含有效字符
                                            let valid = name.len() > 2 && !name.contains('\u{FFFD}') 
                                                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_');
                                            
                                            if valid && (filename.ends_with(".dll") || name.contains("Assembly") || name.contains("mscorlib")) {
                                                images.push(MonoImageInfo {
                                                    address: scan_addr,
                                                    name,
                                                    filename,
                                                });
                                                offset += 0x10; // skip some to avoid duplicates
                                                continue;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                offset += 4;
            }
        }

        // 移到下一个内存区域
        addr = if mbi.RegionSize > 0 { start + size } else { addr + 0x10000 };
    }

    images
}

/// 从名称字符串反查 MonoImage（更精确的定位）
pub fn find_image_by_name(process: &ProcessHandle, target_name: &str) -> Option<MonoImageInfo> {
    let images = find_all_images(process);
    images.into_iter().find(|img| img.name == target_name || img.filename.starts_with(target_name))
}
