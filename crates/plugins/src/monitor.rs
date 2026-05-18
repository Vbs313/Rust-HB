//! 监控插件
//!
//! 实时监控运行数据：运行时长、游戏场次、状态等。

use hb_bot_framework::{BotError, Plugin};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

pub struct MonitorPlugin {
    enabled: AtomicBool,
    start_time: Instant,
    games_played: AtomicU64,
    total_duration: AtomicU64, // 毫秒
}

impl MonitorPlugin {
    pub fn new() -> Self {
        Self {
            enabled: AtomicBool::new(true),
            start_time: Instant::now(),
            games_played: AtomicU64::new(0),
            total_duration: AtomicU64::new(0),
        }
    }

    /// 记录一场游戏
    pub fn record_game(&self, duration_ms: u64) {
        self.games_played.fetch_add(1, Ordering::Relaxed);
        self.total_duration
            .fetch_add(duration_ms, Ordering::Relaxed);
    }

    /// 获取运行状态摘要
    pub fn summary(&self) -> String {
        let elapsed = self.start_time.elapsed();
        let games = self.games_played.load(Ordering::Relaxed);
        format!(
            "⏱ {}h{:02}m | 🎮 {} games | {:.1}s avg",
            elapsed.as_secs() / 3600,
            (elapsed.as_secs() % 3600) / 60,
            games,
            if games > 0 {
                self.total_duration.load(Ordering::Relaxed) as f64 / games as f64 / 1000.0
            } else {
                0.0
            }
        )
    }
}

impl Plugin for MonitorPlugin {
    fn name(&self) -> &'static str {
        "Monitor"
    }
    fn author(&self) -> &'static str {
        "Hearthbuddy Team"
    }
    fn description(&self) -> &'static str {
        "实时监控运行统计"
    }
    fn initialize(&self) -> Result<(), BotError> {
        tracing::info!("Monitor: started at {:?}", self.start_time);
        Ok(())
    }
    fn deinitialize(&self) -> Result<(), BotError> {
        tracing::info!("Monitor: {}", self.summary());
        Ok(())
    }
    fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
}
