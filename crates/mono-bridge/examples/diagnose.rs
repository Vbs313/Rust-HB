//! MonoClass 堆扫描 — 多偏移试探
use hb_core::win32::ProcessHandle;
use hb_core::win32::query_memory_info;

fn main() {
    println!("=== MonoClass 堆扫描 (多偏移) ===\n");
    let p = match ProcessHandle::find_by_name("Hearthstone") {
        Ok(p) => { println!("✅ PID={}", p.pid); p }
        Err(e) => { println!("❌ {e}"); return; }
    };

    // 3 种 name 偏移
    for &name_off in &[0x08usize, 0x0C, 0x10] {
        let ns_off = name_off + 4;
        println!("\n--- name @ +0x{name_off:02x}, ns @ +0x{ns_off:02x} ---");
        let mut found = 0u32;
        
        let mut addr: usize = 0x01000000;
        while addr < 0x40000000 && found < 50 {
            let mbi = match query_memory_info(&p, addr) { Ok(m) => m, _ => { addr += 0x10000; continue; } };
            let start = mbi.BaseAddress as usize;
            let size = mbi.RegionSize;
            if mbi.State != 0x1000 || (mbi.Protect & 0x04) == 0 || size > 0x100000 || size < 0x40 {
                addr = start + size.max(0x10000); continue;
            }
            let mut off = 0usize;
            while off < size && found < 50 {
                let rs = 0x10000usize.min(size - off);
                let data = match p.read_bytes(start + off, rs) { Ok(d) => d, _ => { off += 0x10000; continue; } };
                for i in (0..data.len().saturating_sub(0x40)).step_by(4) {
                    let np = u32::from_le_bytes([data[i+name_off],data[i+name_off+1],data[i+name_off+2],data[i+name_off+3]]);
                    if !(0x01000000..0x7FFFFFFF).contains(&np) { continue; }
                    
                    if let Ok(nb) = p.read_bytes(np as usize, 32) {
                        let nl = nb.iter().position(|&b|b==0).unwrap_or(32);
                        if nl < 2 || nl > 40 { continue; }
                        let name = String::from_utf8_lossy(&nb[..nl]);
                        if name.contains('\u{FFFD}') || name.is_empty() { continue; }
                        let all_ascii = name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '+' || c == '.' || c == '`');
                        if !all_ascii { continue; }
                        
                        // namespace
                        let nsp = u32::from_le_bytes([data[i+ns_off],data[i+ns_off+1],data[i+ns_off+2],data[i+ns_off+3]]);
                        let ns = if nsp > 0x01000000 && nsp < 0x7FFFFFFF {
                            if let Ok(b) = p.read_bytes(nsp as usize, 48) {
                                let l = b.iter().position(|&x|x==0).unwrap_or(48);
                                String::from_utf8_lossy(&b[..l]).to_string()
                            } else { String::new() }
                        } else { String::new() };
                        
                        // image
                        let img0 = u32::from_le_bytes([data[i],data[i+1],data[i+2],data[i+3]]);
                        let img4 = u32::from_le_bytes([data[i+4],data[i+5],data[i+6],data[i+7]]);
                        let img = if img0 > 0x01000000 && img0 < 0x7FFFFFFF { img0 }
                            else if img4 > 0x01000000 && img4 < 0x7FFFFFFF { img4 }
                            else { continue; };
                        
                        let addr_full = start + off + i;
                        found += 1;
                        
                        let is_key = name == "SceneMgr" || name == "Entity" || name == "Player" 
                            || name == "GameState" || name.starts_with("GameStat");
                        
                        if is_key || found <= 20 {
                            println!("  [{found:3}] @ 0x{addr_full:08x} img=0x{img:08x} '{name}' ns='{ns}'");
                        }
                    }
                }
                off += 0x10000;
            }
            addr = start + size.max(0x10000);
        }
        if found > 20 { println!("  ... 共 {found} 个 (截断)"); }
    }
}
