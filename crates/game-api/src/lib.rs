//! # hb-game-api
//!
//! 炉石传说游戏交互 API
//!
//! 替代 C# 版的 TritonHs.cs，提供：
//! - 游戏状态读取（实体、TAG、手牌、场面的完整状态）
//! - 游戏操作（出牌、攻击、使用英雄技能等）
//! - 弹窗检测与处理
//! - 场景状态机

#![allow(dead_code)]

pub mod dialog_handlers;
pub mod game_actions;
pub mod game_state;
pub mod scene_detector;

use hb_core::win32::ProcessHandle;
use hb_input_sim::InputSimulator;
use hb_mono_bridge::MonoBridge;

/// 游戏 API 主入口
pub struct GameApi {
    pub process: ProcessHandle,
    pub mono: MonoBridge,
    pub input: InputSimulator,
}

impl GameApi {
    /// 连接到炉石进程
    pub fn attach(process: ProcessHandle) -> Result<Self, GameError> {
        let mono = MonoBridge::attach(process.duplicate()?)?;
        let input = InputSimulator::new(hb_core::config::MouseSpeedMode::HumanLike);
        Ok(Self {
            process,
            mono,
            input,
        })
    }

    /// 获取当前游戏场景
    pub fn get_current_scene(&self) -> Result<game_state::GameScene, GameError> {
        scene_detector::detect_scene(&self.mono)
    }

    /// 读取完整游戏状态
    pub fn read_game_state(&self) -> Result<game_state::GameState, GameError> {
        game_state::GameState::read_from_process(&self.mono)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GameError {
    #[error("Mono bridge error: {0}")]
    Mono(#[from] hb_mono_bridge::BridgeError),
    #[error("Core error: {0}")]
    Core(#[from] hb_core::error::Error),
    #[error("Input simulation error: {0}")]
    Input(#[from] hb_input_sim::InputError),
    #[error("Failed to read game state: {0}")]
    StateRead(String),
}
