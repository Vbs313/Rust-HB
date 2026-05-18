//! # Hearthbuddy Rust Edition — 主程序
//!
//! 通过 IPC 与 HsMod (BepInEx 插件) 通信，驱动 AI 引擎进行游戏。

use hb_core::config::AppConfig;
use hb_core::log;
use hb_bot_framework::routine_manager::RoutineManager;
use hb_bot_framework::plugin_manager::PluginManager;
use hb_ipc::IpcClient;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. 加载配置
    let config = AppConfig::load_chain()?;
    log::init(&config.log_level, config.log_file.as_deref());
    tracing::info!("Hearthbuddy Rust Edition v{}", env!("CARGO_PKG_VERSION"));

    // 2. 初始化框架
    let mut routine_mgr = RoutineManager::new();
    let default_routine = hb_bot_framework::default_routine::create_default();
    routine_mgr.register(default_routine);
    routine_mgr.set_active("DefaultRoutine")?;
    tracing::info!("DefaultRoutine registered");

    let plugin_mgr = PluginManager::new();
    for plugin in hb_plugins::register_all() {
        plugin_mgr.register(plugin);
    }
    plugin_mgr.initialize_all();

    // 3. 连接 HsMod IPC
    tracing::info!("Connecting to HsMod IPC ({}秒超时)...", 10);
    match IpcClient::connect(Duration::from_secs(10)) {
        Ok(mut ipc) => {
            tracing::info!("✅ IPC connected!");
            run_game_loop(&mut ipc, &routine_mgr).await;
        },
        Err(e) => {
            tracing::warn!("IPC connection failed: {e}");
            tracing::warn!("Starting in offline/demo mode...");
            run_demo_loop(&routine_mgr).await;
        }
    }

    // 4. 清理
    plugin_mgr.deinitialize_all();
    tracing::info!("Shutdown complete");
    Ok(())
}

/// 游戏主循环（IPC 在线模式）
async fn run_game_loop(ipc: &mut IpcClient, routine_mgr: &RoutineManager) {
    let mut tick: u64 = 0;
    loop {
        tokio::time::sleep(Duration::from_millis(500)).await;
        tick += 1;

        // 获取游戏状态，失败时重连
        let state = match ipc.get_game_state() {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("IPC error: {e}, reconnecting...");
                // 尝试重连
                match IpcClient::connect(Duration::from_secs(5)) {
                    Ok(new_ipc) => { *ipc = new_ipc; tracing::info!("Reconnected!"); },
                    Err(e2) => tracing::error!("Reconnect failed: {e2}"),
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
        };

        // 非对战场景跳过
        if state.scene != "Gameplay" && state.scene != "gameplay" {
            if tick.is_multiple_of(20) {
                tracing::info!("Scene: {} (waiting for gameplay)", state.scene);
            }
            continue;
        }

        // 检测是否我方回合
        if !state.is_own_turn {
            if tick.is_multiple_of(20) {
                tracing::info!("Waiting for our turn... (turn {})", state.turn);
            }
            continue;
        }

        tracing::info!("🎯 Our turn! Mana: {}/{}", state.own_mana, state.own_max_mana);

        // 调用 AI 引擎
        if let Some(routine) = routine_mgr.active() {
            match routine.our_turn_logic() {
                Ok(()) => tracing::info!("AI decision completed"),
                Err(e) => tracing::error!("AI error: {e}"),
            }
        }

        // 等待下一帧
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

/// 离线演示模式
async fn run_demo_loop(routine_mgr: &RoutineManager) {
    tracing::info!("Running in demo mode (no HsMod connection)...");
    let mut tick: u64 = 0;
    loop {
        tokio::time::sleep(Duration::from_secs(10)).await;
        tick += 1;
        tracing::info!("Demo tick {tick}: running AI routine...");
        if let Some(routine) = routine_mgr.active() {
            if let Err(e) = routine.our_turn_logic() {
                tracing::warn!("Routine error: {e}");
            }
        }
    }
}
