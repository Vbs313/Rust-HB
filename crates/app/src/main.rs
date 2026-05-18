//! # Hearthbuddy Rust Edition
//!
//! 主程序入口 — 集成 AI 引擎

use hb_core::config::AppConfig;
use hb_core::log;
use hb_process_mgr::ProcessManager;
use hb_bot_framework::bot_manager::BotManager;
use hb_bot_framework::plugin_manager::PluginManager;
use hb_bot_framework::routine_manager::RoutineManager;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. 加载配置
    let config = AppConfig::load_chain()?;
    log::init(&config.log_level, config.log_file.as_deref());
    tracing::info!("Hearthbuddy Rust Edition v{}", env!("CARGO_PKG_VERSION"));

    // 2. 发现进程
    let mut proc_mgr = ProcessManager::new();
    match unsafe { proc_mgr.discover_windows() } {
        Ok(windows) => {
            tracing::info!("Found {} Hearthstone window(s)", windows.len());
            for win in &windows {
                tracing::info!("  PID={}, Title={}", win.pid, win.title);
            }
        }
        Err(e) => {
            tracing::warn!("Failed to find windows: {e}");
        }
    }

    // 3. 初始化框架
    let mut bot_mgr = BotManager::new();
    let plugin_mgr = PluginManager::new();
    let mut routine_mgr = RoutineManager::new();

    // 注册默认策略
    let default_routine = hb_bot_framework::default_routine::create_default();
    routine_mgr.register(default_routine);
    routine_mgr.set_active("DefaultRoutine")?;
    tracing::info!("DefaultRoutine registered and active");

    // 4. 注册插件
    for plugin in hb_plugins::register_all() {
        plugin_mgr.register(plugin);
    }
    plugin_mgr.initialize_all();
    tracing::info!("Plugins initialized");

    // 5. 模拟一次 AI 决策（演示）
    tracing::info!("Running AI test...");
    if let Some(routine) = routine_mgr.active() {
        match routine.our_turn_logic() {
            Ok(()) => tracing::info!("AI test completed"),
            Err(e) => tracing::warn!("AI test: {e}"),
        }
    }

    // 6. 启动主循环
    tracing::info!("Starting main loop...");
    match bot_mgr.start("DefaultBot") {
        Ok(()) => main_loop(&bot_mgr, &routine_mgr).await,
        Err(e) => tracing::warn!("Bot start skipped: {e}"),
    }

    // 7. 清理
    plugin_mgr.deinitialize_all();
    tracing::info!("Shutdown complete");

    Ok(())
}

/// 主循环：每帧调用 Bot.pulse()
async fn main_loop(bot_mgr: &BotManager, routine_mgr: &RoutineManager) {
    let mut tick: u64 = 0;
    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
        tick += 1;

        // 每帧脉冲
        if let Err(e) = bot_mgr.pulse() {
            tracing::warn!("Bot pulse error: {e}");
        }

        // 每 30 秒模拟一次 AI 决策
        if tick % 30 == 0 {
            tracing::info!("Tick {tick}: running AI routine...");
            if let Some(routine) = routine_mgr.active() {
                if let Err(e) = routine.our_turn_logic() {
                    tracing::warn!("Routine error: {e}");
                }
            }
        }
    }
}
