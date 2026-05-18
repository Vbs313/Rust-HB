//! Final approach: scan heap ONCE for ALL class name references
//!
//! Known class name string addresses from earlier scan:
//! Object @ 0x0087cc6f, String @ 0x0087c943, Int32 @ 0x0087c9a8
//! ValueType @ 0x02c188c4, SceneMgr @ 0x02b044cc, GameState @ 0x02b8ac18
//! Entity @ 0x02b43918, GameEntity @ 0x02bacdb8
//!
//! Scan the GC heap (0x01000000-0x40000000) in ONE pass, checking every u32
//! against all known target addresses. Found pointers = MonoClass.name fields.

use hb_core::win32::{ProcessHandle, query_memory_info};

fn main() {
    println!("=== One-pass Heap Reference Scanner ===\n");
    let p = ProcessHandle::find_by_name("Hearthstone").expect("Hearthstone not running");
    println!("✅ PID={}", p.pid);

    // Target strings and their addresses
    let targets = [
        (0x02b044ccusize, "SceneMgr"),
        (0x02b8ac18usize, "GameState"),
        (0x02b43918usize, "Entity"),
        (0x02bacdb8usize, "GameEntity"),
        (0x02c188c4usize, "ValueType"),
    ];

    println!("\nScanning GC heap (0x01000000-0x40000000) for references to target strings...");
    println!("(This may take 30-90 seconds)\n");

    let start = 0x01000000usize;
    let end = 0x40000000usize;
    let chunk = 0x80000usize; // 512KB chunks

    // For each target, collect references
    let mut results: Vec<(String, Vec<usize>)> = targets.iter()
        .map(|(_, name)| (name.to_string(), Vec::new()))
        .collect();

    let mut addr = start;
    let mut chunks_processed = 0u32;

    while addr < end {
        let mbi = match query_memory_info(&p, addr) {
            Ok(m) => m,
            Err(_) => { addr += 0x10000; continue; }
        };
        let s = mbi.BaseAddress as usize;
        let sz = mbi.RegionSize;
        let committed = mbi.State == 0x1000;
        let readable = (mbi.Protect & 0x04) != 0;

        if committed && readable && sz > 0x100 && sz < 0x2000000 {
            let mut off = 0usize;
            while off < sz {
                let rs = chunk.min(sz - off);
                if let Ok(data) = p.read_bytes(s + off, rs) {
                    for j in (0..data.len() - 4).step_by(4) {
                        let val = read_u32(&data, j) as usize;

                        // Check against all targets
                        for (ti, &(taddr, _)) in targets.iter().enumerate() {
                            if val == taddr {
                                results[ti].1.push(s + off + j);
                            }
                        }
                    }
                }
                off += chunk;
                chunks_processed += 1;
                if chunks_processed % 50 == 0 {
                    print!("\r   Progress: 0x{:08x} ({} chunks)", s + off, chunks_processed);
                    use std::io::{Write, stdout};
                    let _ = stdout().flush();
                }
            }
        }
        addr = s + sz.max(0x10000);
        if addr > end { break; }
    }

    println!("\n\n=== RESULTS ===");
    for (name, refs) in &results {
        println!("   {name}: {} references", refs.len());
        if !refs.is_empty() {
            // Show first few references
            for (i, &r) in refs.iter().take(5).enumerate() {
                println!("      [{i}] @ 0x{r:08x}");
                
                // Try to find MonoClass header
                for name_off in [0x00usize, 0x04, 0x08, 0x0C, 0x10, 0x14, 0x18] {
                    if name_off > r { continue; }
                    let class_start = r - name_off;
                    
                    if let Ok(hdr) = p.read_bytes(class_start, 0x40) {
                        // Verify name pointer matches
                        if read_u32(&hdr, name_off) as usize != r { continue; }
                        
                        // Look for namespace pointer
                        for ns_off in [0x00usize, 0x04, 0x08, 0x0C, 0x10, 0x14, 0x18] {
                            if ns_off == name_off { continue; }
                            let ns_ptr = read_u32(&hdr, ns_off) as usize;
                            if ns_ptr > 0x10000 && ns_ptr < 0x7FFFFFFF && ns_ptr != r {
                                if let Some(ns) = read_str_opt(&p, ns_ptr, 32) {
                                    if ns.len() >= 2 && ns.len() <= 30 {
                                        // Found namespace! Verify: look for image pointer
                                        for img_off in [0x00usize, 0x04, 0x08, 0x0C, 0x10, 0x14] {
                                            if img_off == name_off || img_off == ns_off { continue; }
                                            let img_ptr = read_u32(&hdr, img_off) as usize;
                                            if img_ptr > 0x10000 && img_ptr < 0x7FFFFFFF {
                                                let img_name_ptr = p.read_memory::<u32>(img_ptr + 0x1C).unwrap_or(0) as usize;
                                                if img_name_ptr > 0x10000 && img_name_ptr < 0x7FFFFFFF {
                                                    if let Some(img_name) = read_str_opt(&p, img_name_ptr, 48) {
                                                        if img_name.len() > 3 {
                                                            println!("         => MonoClass @ 0x{class_start:08x}");
                                                            println!("            name@+0x{name_off:02X} ns@+0x{ns_off:02X} image@+0x{img_off:02X}");
                                                            println!("            namespace = '{ns}', image = '{img_name}'");
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
            }
        }
    }
}

fn read_u32(data: &[u8], off: usize) -> u32 {
    if off + 4 > data.len() { 0 } else {
        u32::from_le_bytes([data[off], data[off+1], data[off+2], data[off+3]])
    }
}

fn read_str_opt(p: &ProcessHandle, addr: usize, max: usize) -> Option<String> {
    if addr < 0x10000 || addr > 0x7FFFFFFF { return None; }
    let b = p.read_bytes(addr, max).ok()?;
    let l = b.iter().position(|&x| x == 0).unwrap_or(max);
    if l < 2 { return None; }
    let s = String::from_utf8_lossy(&b[..l]).to_string();
    if s.contains('\u{FFFD}') { None } else { Some(s) }
}
