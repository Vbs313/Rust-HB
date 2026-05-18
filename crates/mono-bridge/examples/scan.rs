//! 测试: 全内存扫描找 Assembly-CSharp MonoImage
//! 运行前需启动 Hearthstone

use hb_core::win32::ProcessHandle;

fn main() {
    println!("=== 全内存 MonoImage 扫描 ===\n");

    let process = match ProcessHandle::find_by_name("Hearthstone") {
        Ok(p) => { println!("✅ PID={}", p.pid); p }
        Err(e) => { println!("❌ {e}"); return; }
    };

    println!("扫描进程内存找 MonoImage... (可能需要 10-30 秒)\n");
    let images = hb_mono_bridge::scanner::find_all_images(&process);
    
    println!("找到 {} 个 MonoImage:\n", images.len());
    for img in &images {
        let kind = if img.name.contains("mscorlib") { "📦 corlib" }
            else if img.name.contains("Assembly-CSharp") { "🎯 GAME" }
            else if img.name.contains("Unity") { "🎮 unity" }
            else if img.name.contains("Assembly") { "📋 asm" }
            else { "📄" };
        println!("  {} @ 0x{:08x}  {} ({})", kind, img.address, img.name, img.filename);
    }

    // 找 Assembly-CSharp
    if let Some(acs) = images.iter().find(|i| i.name.contains("Assembly-CSharp")) {
        println!("\n✅ Assembly-CSharp @ 0x{:08x}", acs.address);
        println!("   名称: {}", acs.name);
        println!("   文件: {}", acs.filename);
    } else {
        println!("\n❌ 未找到 Assembly-CSharp");
        println!("   (可尝试用 hb-mono-bridge::scanner::find_all_images 扩大扫描范围)");
    }
}
