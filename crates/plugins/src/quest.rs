//! 任务插件
//!
//! 跟踪每日/每周任务完成进度。

use hb_bot_framework::{BotError, Plugin};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// 任务条目
#[derive(Debug, Clone)]
pub struct QuestInfo {
    pub id: u32,
    pub name: String,
    pub progress: u32,
    pub max_progress: u32,
    pub xp_reward: u32,
}

pub struct QuestPlugin {
    enabled: AtomicBool,
    quests: Mutex<HashMap<u32, QuestInfo>>,
    total_xp: AtomicU32,
}

impl QuestPlugin {
    pub fn new() -> Self {
        Self {
            enabled: AtomicBool::new(true),
            quests: Mutex::new(HashMap::new()),
            total_xp: AtomicU32::new(0),
        }
    }

    /// 更新任务进度
    pub fn update_quest(&self, id: u32, name: &str, progress: u32, max: u32, xp: u32) {
        let mut q = self.quests.lock();
        q.insert(
            id,
            QuestInfo {
                id,
                name: name.to_string(),
                progress,
                max_progress: max,
                xp_reward: xp,
            },
        );
        if progress >= max {
            self.total_xp.fetch_add(xp, Ordering::Relaxed);
            tracing::info!("Quest completed: {name} (+{xp} XP)");
        }
    }

    pub fn summary(&self) -> String {
        let q = self.quests.lock();
        let active: Vec<&QuestInfo> = q.values().filter(|q| q.progress < q.max_progress).collect();
        format!(
            "📋 {} active quests, {} total XP",
            active.len(),
            self.total_xp.load(Ordering::Relaxed)
        )
    }
}

impl Plugin for QuestPlugin {
    fn name(&self) -> &'static str {
        "Quest"
    }
    fn author(&self) -> &'static str {
        "Hearthbuddy Team"
    }
    fn description(&self) -> &'static str {
        "任务进度跟踪"
    }
    fn initialize(&self) -> Result<(), BotError> {
        tracing::info!("Quest plugin ready");
        Ok(())
    }
    fn deinitialize(&self) -> Result<(), BotError> {
        tracing::info!("Quest: {}", self.summary());
        Ok(())
    }
    fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
}
