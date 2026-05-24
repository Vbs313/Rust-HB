//! # Hearthbuddy Rust Edition — 主程序
//!
//! 通过 IPC 与 HsMod (BepInEx 插件) 通信，驱动 AI 引擎进行游戏。
//! TUI 终端仪表盘：ratatui + Catppuccin Mocha + i18n (EN/ZH)

mod supervisor;
mod ipc_client_wrapper;
mod state_monitor;
mod action_executor;
mod tui;
mod i18n;

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
use tui::LogBuffer;

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

    // i18n: 默认语言（可用环境变量 HB_LANG=zh 切换）
    if let Ok(lang) = std::env::var("HB_LANG") {
        i18n::set_language(&lang);
    }

    // 共享日志缓冲区
    let log_buf = LogBuffer::new(200);

    // 3. 连接 HsMod IPC
    if supervisor.is_shutting_down() {
        log_buf.push_str("warn", "Shutdown before IPC connect, skipping");
        tracing::warn!("Shutdown before IPC connect, skipping");
    } else {
        log_buf.push_str("info", "Connecting to IPC...");
        tracing::info!("Connecting to HsMod IPC (10s timeout)...");
        match AsyncIpcClient::connect(Duration::from_secs(10)).await {
            Ok(ipc) => {
                log_buf.push_str("info", "IPC connected!");
                tracing::info!("IPC connected!");
                let ipc: Arc<dyn IpcClientOps> = Arc::new(ipc);
                run_game_loop(&supervisor, ipc, &routine_mgr, log_buf.clone()).await;
            }
            Err(e) => {
                log_buf.push_str("warn", format!("IPC failed: {e}"));
                tracing::warn!("IPC connection failed: {e}");
                tracing::warn!("Starting in offline/demo mode...");
                run_demo_loop(&supervisor, &routine_mgr, log_buf.clone()).await;
            }
        }
    }

    // 4. 清理
    tracing::info!("Shutting down plugins...");
    plugin_mgr.deinitialize_all();
    log_buf.push_str("info", "Shutdown complete");
    tracing::info!("Shutdown complete");
    Ok(())
}

/// 游戏主循环（IPC 在线模式）
async fn run_game_loop(
    supervisor: &Supervisor,
    ipc: Arc<dyn IpcClientOps>,
    routine_mgr: &RoutineManager,
    log_buf: LogBuffer,
) {
    let (mut state_rx, status_rx) = StateMonitor::spawn_default(
        supervisor.shutdown_flag(),
        ipc.clone(),
    );

    // 启动 TUI（独立 task）
    let tui_log = log_buf.clone();
    let tui_state_rx = state_rx.clone();
    let tui_status_rx = status_rx.clone();
    let tui_cancel = supervisor.shutdown_flag();
    tokio::spawn(async move {
        tui::run_tui(tui_cancel, tui_state_rx, tui_status_rx, tui_log).await;
    });

    let mut tick: u64 = 0;

    loop {
        tokio::select! {
            // 关闭信号（Ctrl+C）→ 退出
            _ = supervisor.wait_for_shutdown() => {
                log_buf.push_str("info", t!("shutdown.received"));
                tracing::info!("{}", t!("shutdown.received"));
                break;
            }

            // 游戏状态变更 → 状态机处理
            result = state_rx.changed() => {
                if result.is_err() {
                    log_buf.push_str("warn", "StateMonitor channel closed");
                    tracing::warn!("StateMonitor channel closed, exiting");
                    break;
                }

                tick += 1;
                let snapshot = state_rx.borrow_and_update().clone();
                let Some(state) = snapshot else {
                    if tick.is_multiple_of(10) {
                        log_buf.push_str("info", "Waiting for IPC reconnection...");
                    }
                    continue;
                };

                // ── 状态机 ──────────────────────────────────
                // 状态 A: 不在游戏对战中
                if state.scene != "Gameplay" && state.scene != "gameplay" {
                    if tick.is_multiple_of(20) {
                        let msg = format!("{}", state.scene);
                        log_buf.push_str("info", msg);
                    }
                    continue;
                }

                // 状态 B: 在游戏中，对手回合
                if !state.is_own_turn {
                    if tick.is_multiple_of(20) {
                        log_buf.push_str("info", format!("Waiting for turn ({})", state.turn));
                    }
                    continue;
                }

                // 状态 C: 我方回合 → AI 决策 + 动作执行
                log_buf.push_str("info", format!("Our turn! Mana: {}/{}", state.own_mana, state.own_max_mana));

                // C1: 执行 Routine
                if let Some(routine) = routine_mgr.active() {
                    let routine_state = state.clone();
                    if let Err(e) = tokio::task::spawn_blocking(move || {
                        routine.our_turn_logic(&routine_state)
                    }).await.expect("Routine task panicked") {
                        log_buf.push_str("error", format!("Routine error: {e}"));
                        tracing::error!("Routine error: {e}");
                    }
                }

                // C2: AI 决策 → IPC 执行
                match decide_and_execute(Some(ipc.clone()), None, &state).await {
                    Ok(()) => {
                        log_buf.push_str("info", "Action dispatched");
                    }
                    Err(e) => {
                        log_buf.push_str("warn", format!("Action skipped: {e}"));
                    }
                }

                // C3: 等待游戏处理
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    }
    log_buf.push_str("info", "Game loop exited");
    tracing::info!("Game loop exited");
}

/// 离线演示模式
async fn run_demo_loop(supervisor: &Supervisor, routine_mgr: &RoutineManager, log_buf: LogBuffer) {
    log_buf.push_str("info", "Demo mode (no IPC)");
    tracing::info!("Running in demo mode (no IPC connection)...");
    let mut tick: u64 = 0;
    while !supervisor.is_shutting_down() {
        tokio::time::sleep(Duration::from_secs(10)).await;
        if supervisor.is_shutting_down() { break; }
        tick += 1;
        log_buf.push_str("info", format!("Demo tick {tick}"));
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
                log_buf.push_str("warn", format!("Routine error: {e}"));
                tracing::warn!("Routine error: {e}");
            }
        }
    }
    log_buf.push_str("info", "Demo loop exited");
    tracing::info!("Demo loop exited");
}
