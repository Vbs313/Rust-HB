//! 自动停止插件
//!
//! 达到指定条件（胜场/负场/局数）后自动停止运行。

use hb_bot_framework::{BotError, Plugin};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// 停止条件
#[derive(Debug, Clone, Copy)]
pub enum StopCondition {
    /// 达到指定胜场数
    Wins(u32),
    /// 达到指定负场数
    Losses(u32),
    /// 达到指定总局数
    TotalGames(u32),
}

pub struct AutoStopPlugin {
    enabled: AtomicBool,
    wins: AtomicU32,
    losses: AtomicU32,
    total: AtomicU32,
    max_wins: u32,
    max_losses: u32,
    max_games: u32,
}

impl AutoStopPlugin {
    pub fn new() -> Self {
        Self {
            enabled: AtomicBool::new(true),
            wins: AtomicU32::new(0),
            losses: AtomicU32::new(0),
            total: AtomicU32::new(0),
            max_wins: 0, // 0 = disable
            max_losses: 0,
            max_games: 0,
        }
    }

    /// 记录一场胜利
    pub fn record_win(&self) {
        self.wins.fetch_add(1, Ordering::Relaxed);
        self.total.fetch_add(1, Ordering::Relaxed);
        self.check_stop();
    }

    /// 记录一场失败
    pub fn record_loss(&self) {
        self.losses.fetch_add(1, Ordering::Relaxed);
        self.total.fetch_add(1, Ordering::Relaxed);
        self.check_stop();
    }

    fn check_stop(&self) {
        let wins = self.wins.load(Ordering::Relaxed);
        let losses = self.losses.load(Ordering::Relaxed);
        let total = self.total.load(Ordering::Relaxed);

        if (self.max_wins > 0 && wins >= self.max_wins)
            || (self.max_losses > 0 && losses >= self.max_losses)
            || (self.max_games > 0 && total >= self.max_games)
        {
            tracing::info!("AutoStop: stopping (wins={wins}, losses={losses}, total={total})");
            self.enabled.store(false, Ordering::Relaxed);
        }
    }
}

impl Plugin for AutoStopPlugin {
    fn name(&self) -> &'static str {
        "AutoStop"
    }
    fn author(&self) -> &'static str {
        "Hearthbuddy Team"
    }
    fn description(&self) -> &'static str {
        "Automatically stop after configurable conditions"
    }
    fn initialize(&self) -> Result<(), BotError> {
        tracing::info!(
            "AutoStop: initialized (wins≤{}, losses≤{}, games≤{})",
            self.max_wins,
            self.max_losses,
            self.max_games
        );
        Ok(())
    }
    fn deinitialize(&self) -> Result<(), BotError> {
        let w = self.wins.load(Ordering::Relaxed);
        let l = self.losses.load(Ordering::Relaxed);
        tracing::info!("AutoStop: deinitialized (final: {w}W/{l}L)");
        Ok(())
    }
    fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
}
