//! # Hearthbuddy Rust Edition — 主程序
//!
//! 通过 IPC 与 HsMod (BepInEx 插件) 通信，驱动 AI 引擎进行游戏。
//!
//! ## 架构
//!
//! - Supervisor: 生命周期管理（Ctrl+C → 优雅关闭）
//! - AsyncIpcClient: 异步 IPC 包装（Arc<Mutex> + spawn_blocking）
//! - StateMonitor: 后台读取游戏状态，通过 watch channel 广播
//! - ActionExecutor: AI 决策 → IPC 动作执行
//! - 主循环: tokio::select! 驱动状态机

mod supervisor;
mod ipc_client_wrapper;
mod state_monitor;
mod action_executor;

use hb_core::config::AppConfig;
use hb_core::log;
use hb_bot_framework::routine_manager::RoutineManager;
use hb_bot_framework::plugin_manager::PluginManager;
use action_executor::decide_and_execute;
use ipc_client_wrapper::{AsyncIpcClient, IpcClientOps};
use state_monitor::StateMonitor;
use std::sync::Arc;
use std::time::Duration;
use supervisor::Supervisor;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 0. Supervisor 生命周期管理
    let supervisor = Supervisor::new();

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

    // 3. 连接 HsMod IPC（异步连接，不阻塞 runtime）
    if supervisor.is_shutting_down() {
        tracing::warn!("Shutdown before IPC connect, skipping");
    } else {
        tracing::info!("Connecting to HsMod IPC (10s timeout)...");
        match AsyncIpcClient::connect(Duration::from_secs(10)).await {
            Ok(ipc) => {
                tracing::info!("IPC connected!");
                let ipc: Arc<dyn IpcClientOps> = Arc::new(ipc);
                run_game_loop(&supervisor, ipc, &routine_mgr).await;
            }
            Err(e) => {
                tracing::warn!("IPC connection failed: {e}");
                tracing::warn!("Starting in offline/demo mode...");
                run_demo_loop(&supervisor, &routine_mgr).await;
            }
        }
    }

    // 4. 清理
    tracing::info!("Shutting down plugins...");
    plugin_mgr.deinitialize_all();
    tracing::info!("Shutdown complete");
    Ok(())
}

/// 游戏主循环（IPC 在线模式）
async fn run_game_loop(
    supervisor: &Supervisor,
    ipc: Arc<dyn IpcClientOps>,
    routine_mgr: &RoutineManager,
) {
    let (mut state_rx, _status_rx) = StateMonitor::spawn_default(
        supervisor.shutdown_flag(),
        ipc.clone(),
    );
    let mut tick: u64 = 0;

    loop {
        tokio::select! {
            // 关闭信号（Ctrl+C）→ 退出
            _ = supervisor.wait_for_shutdown() => {
                tracing::info!("Shutdown signal received, exiting game loop");
                break;
            }

            // 游戏状态变更 → 状态机处理
            result = state_rx.changed() => {
                if result.is_err() {
                    tracing::warn!("StateMonitor channel closed, exiting");
                    break;
                }

                tick += 1;
                let snapshot = state_rx.borrow_and_update().clone();
                let Some(state) = snapshot else {
                    if tick.is_multiple_of(10) {
                        tracing::info!("Waiting for IPC reconnection...");
                    }
                    continue;
                };

                // ── 状态机 ──────────────────────────────────
                // 状态 A: 不在游戏对战中
                if state.scene != "Gameplay" && state.scene != "gameplay" {
                    if tick.is_multiple_of(20) {
                        tracing::info!("Scene: {} (waiting for gameplay)", state.scene);
                    }
                    continue;
                }

                // 状态 B: 在游戏中，对手回合
                if !state.is_own_turn {
                    if tick.is_multiple_of(20) {
                        tracing::info!("Waiting for our turn... (turn {})", state.turn);
                    }
                    continue;
                }

                // 状态 C: 我方回合 → AI 决策 + 动作执行
                tracing::info!("Our turn! Mana: {}/{}", state.own_mana, state.own_max_mana);

                // C1: 执行 Routine 的 our_turn_logic（插件钩子、日志等）
                if let Some(routine) = routine_mgr.active() {
                    let routine_state = state.clone();
                    if let Err(e) = tokio::task::spawn_blocking(move || {
                        routine.our_turn_logic(&routine_state)
                    }).await.expect("Routine task panicked") {
                        tracing::error!("Routine error: {e}");
                    }
                }

                // C2: 将 AI 决策转换为动作并通过 IPC 执行
                match decide_and_execute(Some(ipc.clone()), None, &state).await {
                    Ok(()) => tracing::info!("Action dispatch completed"),
                    Err(e) => tracing::warn!("Action dispatch skipped: {e}"),
                }

                // C3: 等待游戏处理动作后再检查状态
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    }
    tracing::info!("Game loop exited");
}

/// 离线演示模式
async fn run_demo_loop(supervisor: &Supervisor, routine_mgr: &RoutineManager) {
    tracing::info!("Running in demo mode (no IPC connection)...");
    let mut tick: u64 = 0;
    while !supervisor.is_shutting_down() {
        tokio::time::sleep(Duration::from_secs(10)).await;
        if supervisor.is_shutting_down() { break; }
        tick += 1;
        tracing::info!("Demo tick {tick}: running AI routine...");
        if let Some(routine) = routine_mgr.active() {
            let demo_state = hb_ipc::GameStateData {
                scene: "Hub".into(), is_own_turn: false, turn: 0,
                own_mana: 10, own_max_mana: 10,
                own_hero: hb_ipc::EntityData {
                    entity_id: 1, card_id: "HERO_01".into(),
                    health: 30, attack: 0, armor: 0,
                    has_taunt: false, has_divine_shield: false, has_stealth: false,
                    has_poisonous: false, has_lifesteal: false,
                    is_exhausted: false, num_attacks: 0,
                },
                enemy_hero: hb_ipc::EntityData {
                    entity_id: 2, card_id: "HERO_02".into(),
                    health: 30, attack: 0, armor: 0,
                    has_taunt: false, has_divine_shield: false, has_stealth: false,
                    has_poisonous: false, has_lifesteal: false,
                    is_exhausted: false, num_attacks: 0,
                },
                own_hand: vec![], own_minions: vec![], enemy_minions: vec![],
                own_hand_count: 0, enemy_hand_count: 0,
                own_deck_count: 30, enemy_deck_count: 30,
            };
            if let Err(e) = routine.our_turn_logic(&demo_state) {
                tracing::warn!("Routine error: {e}");
            }
        }
    }
    tracing::info!("Demo loop exited");
}
