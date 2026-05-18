//! 从 Hearthbuddy.exe 内存中提取 OffsetTable36
//! 先启动 Hearthbuddy.exe（已移除认证的修改版），再运行此工具

use hb_core::win32::query_memory_info;
use hb_core::win32::ProcessHandle;

fn main() {
    println!("=== OffsetTable Extractor ===\n");
    println!("步骤:");
    println!("  1. 确认 Hearthstone.exe 已运行 (当前 PID=9364 ✅)");
    println!("  2. 启动 HB3.1.2/Hearthbuddy.exe（或改名的 .exe）");
    println!("  3. 按 Enter 扫描...");
    let _ = std::io::stdin().read_line(&mut String::new());

    // 找进程：检查多个可能的进程名
    let names = ["Hearthbuddy", "ynvbkpxdsd", "djndiyomai", "炉石中控"];
    let mut process = None;
    for name in &names {
        if let Ok(p) = ProcessHandle::find_by_name(name) {
            println!("✅ 找到进程: {name} PID={}", p.pid);
            process = Some(p);
            break;
        }
    }

    let p = match process {
        Some(p) => p,
        None => {
            println!("❌ 未找到 Hearthbuddy 相关进程");
            println!("   请先启动 HB3.1.2 目录下的 exe");
            return;
        }
    };

    // 扫描进程内存找 36 个连续 u32 构成的有效 offset 表
    println!("\n扫描进程内存 (可能需要 30 秒)...");
    let mut addr: usize = 0x00010000;
    let mut found = 0u32;

    while addr < 0x7FFFFFFF && found < 3 {
        let mbi = match query_memory_info(&p, addr) {
            Ok(m) => m,
            _ => {
                addr += 0x10000;
                continue;
            }
        };
        let start = mbi.BaseAddress as usize;
        let size = mbi.RegionSize;
        if mbi.State != 0x1000 || (mbi.Protect & 0x04) == 0 || size > 0x200000 || size < 144 {
            addr = start + size.max(0x10000);
            continue;
        }

        let mut off = 0usize;
        while off + 144 < size {
            if let Ok(data) = p.read_bytes(start + off, 144) {
                let mut vals = [0u32; 36];
                for i in 0..36 {
                    vals[i] = u32::from_le_bytes([
                        data[i * 4],
                        data[i * 4 + 1],
                        data[i * 4 + 2],
                        data[i * 4 + 3],
                    ]);
                }
                if is_offset_table(&vals) {
                    found += 1;
                    println!("\n✅ OffsetTable #{} @ 0x{:08x}:", found, start + off);
                    for row in 0..9 {
                        let i = row * 4;
                        println!(
                            "  [{:02}-{:02}] 0x{:04x} 0x{:04x} 0x{:04x} 0x{:04x}",
                            i,
                            i + 3,
                            vals[i],
                            vals[i + 1],
                            vals[i + 2],
                            vals[i + 3]
                        );
                    }
                }
            }
            off += 144;
        }
        addr = start + size.max(0x10000);
    }
    if found == 0 {
        println!("❌ 未找到 offset 表");
        println!("   可能原因: Hearthbuddy 的修改版可能已将偏移数据替换为其他机制");
    }
}

fn is_offset_table(vals: &[u32; 36]) -> bool {
    let small = vals.iter().filter(|&&v| v < 0x200).count();
    let zeros = vals.iter().filter(|&&v| v == 0).count();
    let huge = vals.iter().filter(|&&v| v > 0x10000).count();
    small >= 20 && zeros <= 12 && huge <= 5
}
