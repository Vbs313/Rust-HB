//! 统计插件
//!
//! 统计各职业胜率、环境分布等数据。

use hb_bot_framework::{BotError, Plugin};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// 职业统计
#[derive(Debug, Clone, Default)]
pub struct ClassStats {
    pub wins: u32,
    pub losses: u32,
    pub games: u32,
}

impl ClassStats {
    pub fn winrate(&self) -> f64 {
        if self.games == 0 {
            0.0
        } else {
            self.wins as f64 / self.games as f64 * 100.0
        }
    }
}

pub struct StatsPlugin {
    enabled: AtomicBool,
    by_class: Mutex<HashMap<String, ClassStats>>,
    total_games: AtomicU32,
}

impl StatsPlugin {
    pub fn new() -> Self {
        Self {
            enabled: AtomicBool::new(true),
            by_class: Mutex::new(HashMap::new()),
            total_games: AtomicU32::new(0),
        }
    }

    /// 记录一场比赛结果
    pub fn record_game(&self, class: &str, won: bool) {
        let mut stats = self.by_class.lock();
        let entry = stats.entry(class.to_string()).or_default();
        entry.games += 1;
        if won {
            entry.wins += 1;
        } else {
            entry.losses += 1;
        }
        self.total_games.fetch_add(1, Ordering::Relaxed);
    }

    pub fn summary(&self) -> String {
        let stats = self.by_class.lock();
        let mut lines: Vec<String> = stats
            .iter()
            .map(|(cls, s)| format!("{}: {}W/{}L ({:.1}%)", cls, s.wins, s.losses, s.winrate()))
            .collect();
        lines.sort();
        format!(
            "📊 {} classes, {} total games\n{}",
            stats.len(),
            self.total_games.load(Ordering::Relaxed),
            lines.join("\n")
        )
    }
}

impl Plugin for StatsPlugin {
    fn name(&self) -> &'static str {
        "Stats"
    }
    fn author(&self) -> &'static str {
        "Hearthbuddy Team"
    }
    fn description(&self) -> &'static str {
        "职业胜率统计"
    }
    fn initialize(&self) -> Result<(), BotError> {
        tracing::info!("Stats plugin ready");
        Ok(())
    }
    fn deinitialize(&self) -> Result<(), BotError> {
        tracing::info!("Stats:\n{}", self.summary());
        Ok(())
    }
    fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
}
