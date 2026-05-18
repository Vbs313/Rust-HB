//! 从 Hearthbuddy.exe 内存中提取 OffsetTable36
//! 1. 启动 Hearthbuddy.exe（需用户手动启动）
//! 2. 在其内存中搜索 36 个连续 int 的 offset pattern
//! 3. 输出到 offsets.json

use hb_core::win32::ProcessHandle;
use hb_core::win32::query_memory_info;

fn main() {
    println!("=== OffsetTable Extractor ===\n");
    println!("请先启动 HB3.1.2/Hearthbuddy.exe（它会自动连接 Hearthstone）");
    println!("按 Enter 开始扫描...");
    let _ = std::io::stdin().read_line(&mut String::new());

    // 找 Hearthbuddy 进程
    let p = match ProcessHandle::find_by_name("Hearthbuddy") {
        Ok(p) => { println!("✅ Hearthbuddy PID={}", p.pid); p }
        Err(_) => {
            // 也可能是改名的 exe
            match ProcessHandle::find_by_name("ynvbkpxdsd") {
                Ok(p) => { println!("✅ ynvbkpxdsd PID={}", p.pid); p }
                Err(e) => { println!("❌ 未找到 Hearthbuddy 进程: {e}"); return; }
            }
        }
    };

    // 扫描进程内存找 36 个连续 u32 构成的有效 offset 表
    println!("\n扫描内存...");
    let mut addr: usize = 0x00010000;
    let mut count = 0u32;

    while addr < 0x7FFFFFFF && count < 3 {
        let mbi = match query_memory_info(&p, addr) {
            Ok(m) => m, _ => { addr += 0x10000; continue; }
        };
        let start = mbi.BaseAddress as usize;
        let size = mbi.RegionSize;
        if mbi.State != 0x1000 || (mbi.Protect & 0x04) == 0 || size > 0x200000 || size < 144 {
            addr = start + size.max(0x10000); continue;
        }

        let mut off = 0usize;
        while off + 144 < size {
            if let Ok(data) = p.read_bytes(start + off, 144) {
                // 解码为 36 个 u32
                let mut vals = [0u32; 36];
                for i in 0..36 {
                    vals[i] = u32::from_le_bytes([
                        data[i*4], data[i*4+1], data[i*4+2], data[i*4+3]
                    ]);
                }
                // 检查是否像 offset 表
                if is_offset_table(&vals) {
                    count += 1;
                    println!("\n✅ 找到 OffsetTable #{} @ 0x{:08x}:", count, start + off);
                    for row in 0..9 {
                        let i = row * 4;
                        println!("  int_{:02}-{:02}: 0x{:04x} 0x{:04x} 0x{:04x} 0x{:04x}",
                            i, i+3, vals[i], vals[i+1], vals[i+2], vals[i+3]);
                    }
                    // 保存到文件
                    
                    
                    println!("\n已保存到 offsets.json");
                }
            }
            off += 144;
        }
        addr = start + size.max(0x10000);
    }
    if count == 0 {
        println!("❌ 未找到 offset 表（Hearthbuddy 可能还没加载完偏移）");
    }
}

fn is_offset_table(vals: &[u32; 36]) -> bool {
    // 特征: 
    // 1. 大部分值 < 0x200 (结构体偏移)
    // 2. 部分值可能为 0
    // 3. 可以有少量大值（指针偏移）
    // 4. 不能有太多随机大值
    let small = vals.iter().filter(|&&v| v < 0x200).count();
    let zeros = vals.iter().filter(|&&v| v == 0).count();
    let huge = vals.iter().filter(|&&v| v > 0x10000).count();
    
    small >= 20 && zeros <= 12 && huge <= 4
}
