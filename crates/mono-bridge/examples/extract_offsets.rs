//! OffsetTable36 提取器 — 自动模式（简化版）
//! 配合 run_extract.bat 使用：先启动 HB，再运行此工具

use hb_core::win32::query_memory_info;
use hb_core::win32::ProcessHandle;

fn main() {
    println!("=== OffsetTable36 Extractor ===\n");

    // 先尝试已知的 PID（ysenatmhyn 进程已运行）
    println!("Trying known PID 14916...");
    let p = if let Ok(p) = ProcessHandle::find_by_pid(14916) {
        println!("✅ Attached to PID 14916");
        p
    } else {
        // 循环等待 Hearthbuddy 进程出现
        println!("PID 14916 not accessible, waiting for process...");
        let names = [
            "Hearthbuddy",
            "ynvbkpxdsd",
            "djndiyomai",
            "炉石中控",
            "vsezrrppcj",
            "ysenatmhyn",
        ];
        'outer: loop {
            for name in &names {
                if let Ok(p) = ProcessHandle::find_by_name(name) {
                    println!("✅ {} PID={}", name, p.pid);
                    break 'outer p;
                }
            }
            print!(".");
            use std::io::{stdout, Write};
            stdout().flush().ok();
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    };

    // 再等一会儿让 HB 完成初始化
    println!("\nWaiting for initialization...");
    std::thread::sleep(std::time::Duration::from_secs(3));

    // 扫描内存找 36-int offset 表
    println!("\nScanning memory...");
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
        println!("\n❌ No offset table found");
        println!("   HB may have been modified to not store offsets as contiguous array");
    }
}

fn is_offset_table(vals: &[u32; 36]) -> bool {
    let small = vals.iter().filter(|&&v| v < 0x200).count();
    let zeros = vals.iter().filter(|&&v| v == 0).count();
    let huge = vals.iter().filter(|&&v| v > 0x10000).count();
    small >= 20 && zeros <= 12 && huge <= 5
}
