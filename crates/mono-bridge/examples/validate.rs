//! 从 MonoImage class_cache 快速枚举类（使用已知地址）
use hb_core::win32::ProcessHandle;
use hb_mono_bridge::class_cache::enum_classes_from_image;
use hb_mono_bridge::scanner::MonoImageInfo;

fn main() {
    println!("=== Mono 类枚举 (via class_cache) ===\n");

    let p = match ProcessHandle::find_by_name("Hearthstone") {
        Ok(p) => {
            println!("✅ PID={}", p.pid);
            p
        }
        Err(e) => {
            println!("❌ {e}");
            return;
        }
    };

    // Assembly-CSharp MonoImage 地址（从之前扫描获得，每次游戏启动可能变化）
    // 可通过扫描确认，但扫描较慢（30s+）
    let acs_addr = find_acs_addr(&p);
    if acs_addr == 0 {
        println!("❌ 未找到 Assembly-CSharp, 请先运行 scan 示例");
        return;
    }
    let acs = MonoImageInfo {
        address: acs_addr,
        name: "Assembly-CSharp".into(),
        filename: "Assembly-CSharp.dll".into(),
    };
    println!("✅ Assembly-CSharp @ 0x{:08x}\n", acs.address);

    // 枚举所有类
    println!("枚举类 (class_cache)...");
    let classes = enum_classes_from_image(&p, &acs);
    println!("共 {} 个类\n", classes.len());

    // 按命名空间分组
    use std::collections::HashMap;
    let mut ns_map: HashMap<String, u32> = HashMap::new();
    for c in &classes {
        *ns_map
            .entry(if c.namespace.is_empty() {
                "(global)".into()
            } else {
                c.namespace.clone()
            })
            .or_insert(0) += 1;
    }
    let mut v: Vec<_> = ns_map.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    println!("命名空间分布 (Top 15):");
    for (n, c) in v.iter().take(15) {
        println!("  {n}: {c}");
    }

    // 找关键类
    println!("\n关键类搜索:");
    let targets = [
        "SceneMgr",
        "GameState",
        "Entity",
        "Player",
        "GameStateManager",
        "GameEntity",
    ];
    for class in &classes {
        if targets.contains(&class.name.as_str()) {
            println!(
                "  ✅ {} ({}) @ 0x{:08x}",
                class.name, class.namespace, class.address
            );
            println!("     大小={}B 字段={}", class.class_size, class.field_count);
        }
    }
}

/// 快速找 Assembly-CSharp: 从已加载模块找+读MonoImage
fn find_acs_addr(p: &ProcessHandle) -> usize {
    // 方法1: 从内存扫描(太慢，跳过)
    // 方法2: 从已知地址范围查找(当前游戏的acs地址会变化但通常在 0x0e000000-0x0f000000)
    // 我们直接全量扫描但有计时
    println!("扫描 Assembly-CSharp...(可能需要 30 秒)");
    let images = hb_mono_bridge::scanner::find_all_images(p);
    for img in &images {
        if img.name == "Assembly-CSharp" && img.filename.contains("Assembly-CSharp.dll") {
            return img.address;
        }
    }
    0
}
