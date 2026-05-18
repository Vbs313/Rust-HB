//! 进程内存扫描器 — 块读取优化版
//!
//! 每次读取 64KB → 本地扫描 MonoImage 签名

use hb_core::win32::query_memory_info;
use hb_core::win32::ProcessHandle;

const NAME_OFF: usize = 0x1C;
const FILE_OFF: usize = 0x20;

#[derive(Debug)]
pub struct MonoImageInfo {
    pub address: usize,
    pub name: String,
    pub filename: String,
}

/// 高性能扫描所有 MonoImage
pub fn find_all_images(process: &ProcessHandle) -> Vec<MonoImageInfo> {
    let mut images = Vec::new();
    let mut addr: usize = 0x00010000;

    while addr < 0x7FFFFFFF {
        let mbi = match query_memory_info(process, addr) {
            Ok(m) => m,
            Err(_) => {
                addr += 0x10000;
                continue;
            }
        };

        let start = mbi.BaseAddress as usize;
        let size = mbi.RegionSize;
        let committed = mbi.State == 0x1000;
        let readable = (mbi.Protect & 0x02) != 0 || (mbi.Protect & 0x04) != 0;

        if committed && readable && size > 0 && size < 0x1000000 {
            // 分块读取（每块 64KB）
            let chunk_size: usize = 0x10000;
            let mut chunk_off = 0usize;

            while chunk_off < size {
                let read_size = chunk_size.min(size - chunk_off);
                let chunk_data = match process.read_bytes(start + chunk_off, read_size) {
                    Ok(d) => d,
                    Err(_) => {
                        chunk_off += chunk_size;
                        continue;
                    }
                };

                // 在块内扫描 4 字节对齐的地址
                for i in (0..chunk_data.len().saturating_sub(NAME_OFF + 8)).step_by(4) {
                    // 读取 +0x1C 处的指针 (小端序 u32)
                    let name_ptr = u32::from_le_bytes([
                        chunk_data[i + NAME_OFF],
                        chunk_data[i + NAME_OFF + 1],
                        chunk_data[i + NAME_OFF + 2],
                        chunk_data[i + NAME_OFF + 3],
                    ]) as usize;

                    if !(0x00010000..0x7FFFFFFF).contains(&name_ptr) {
                        continue;
                    }

                    // 读取 +0x20 处的文件名字段
                    let file_ptr = u32::from_le_bytes([
                        chunk_data[i + FILE_OFF],
                        chunk_data[i + FILE_OFF + 1],
                        chunk_data[i + FILE_OFF + 2],
                        chunk_data[i + FILE_OFF + 3],
                    ]) as usize;

                    if !(0x00010000..0x7FFFFFFF).contains(&file_ptr) {
                        continue;
                    }

                    // 读名称（直接从远程读，因为指针指向的数据不在当前块内）
                    if let Ok(nbytes) = process.read_bytes(name_ptr, 48) {
                        let nlen = nbytes.iter().position(|&b| b == 0).unwrap_or(48);
                        if nlen < 3 {
                            continue;
                        }
                        let name = String::from_utf8_lossy(&nbytes[..nlen]).to_string();
                        if name.contains('\u{FFFD}') {
                            continue;
                        }

                        // 读文件名
                        if let Ok(fbytes) = process.read_bytes(file_ptr, 48) {
                            let flen = fbytes.iter().position(|&b| b == 0).unwrap_or(48);
                            let filename = String::from_utf8_lossy(&fbytes[..flen]).to_string();

                            // 验证：必须是 MonoImage（有合理的名称和文件名）
                            let is_valid = name.chars().all(|c| {
                                c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_'
                            }) && (filename.ends_with(".dll")
                                || name.contains("Assembly")
                                || name.contains("mscorlib"));

                            if is_valid {
                                let abs_addr = start + chunk_off + i;
                                // 去重
                                if !images.iter().any(|x: &MonoImageInfo| x.address == abs_addr) {
                                    images.push(MonoImageInfo {
                                        address: abs_addr,
                                        name,
                                        filename,
                                    });
                                }
                            }
                        }
                    }
                }
                chunk_off += chunk_size;
            }
        }

        addr = start + size;
        if size == 0 {
            addr += 0x10000;
        }
    }

    images
}

/// 按名称找
pub fn find_image_by_name(process: &ProcessHandle, target: &str) -> Option<MonoImageInfo> {
    let images = find_all_images(process);
    images
        .into_iter()
        .find(|i| i.name == target || i.filename.starts_with(target))
}
