//! # hb-bot-framework
//!
//! Bot/Plugin/Routine 框架
//!
//! 对应 C# 版的 Triton.Bot 命名空间。
//! 提供 IBot / IPlugin / IRoutine 等核心接口和加载管理。

#![allow(dead_code)]

pub mod bot_manager;
pub mod plugin_manager;
pub mod routine_manager;
pub mod task_manager;
pub mod events;
pub mod default_routine;

// unused

/// Bot 接口
pub trait Bot: Send + Sync {
    fn name(&self) -> &'static str;
    fn author(&self) -> &'static str;
    fn description(&self) -> &'static str;

    /// 启动 Bot
    fn start(&self) -> Result<(), BotError>;
    /// 停止 Bot
    fn stop(&self) -> Result<(), BotError>;
    /// 脉冲（每帧调用）
    fn pulse(&self) -> Result<(), BotError>;
    /// 是否正在运行
    fn is_running(&self) -> bool;
}

/// 插件接口
pub trait Plugin: Send + Sync {
    fn name(&self) -> &'static str;
    fn author(&self) -> &'static str;
    fn description(&self) -> &'static str;

    /// 初始化
    fn initialize(&self) -> Result<(), BotError>;
    /// 反初始化
    fn deinitialize(&self) -> Result<(), BotError>;
    /// 是否已启用
    fn is_enabled(&self) -> bool;
}

/// 策略接口（AI 决策入口）
pub trait Routine: Send + Sync {
    fn name(&self) -> &'static str;
    fn author(&self) -> &'static str;
    fn description(&self) -> &'static str;

    /// 我方回合逻辑（接收游戏状态）
    fn our_turn_logic(&self, state: &hb_ipc::GameStateData) -> Result<(), BotError>;
    /// 留牌逻辑
    fn mulligan_logic(&self) -> Result<Vec<i32>, BotError>;
}

/// Bot 框架错误
#[derive(Debug, thiserror::Error)]
pub enum BotError {
    #[error("Bot not found: {0}")]
    BotNotFound(String),
    #[error("Plugin not found: {0}")]
    PluginNotFound(String),
    #[error("Routine not found: {0}")]
    RoutineNotFound(String),
    #[error("Bot already running")]
    AlreadyRunning,
    #[error("Bot not running")]
    NotRunning,
    #[error("Core error: {0}")]
    Core(#[from] hb_core::error::Error),
    #[error("Game API error: {0}")]
    Game(#[from] hb_game_api::GameError),
}
