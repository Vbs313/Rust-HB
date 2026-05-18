//! 游戏事件系统
//!
//! 对应 C# 版的 GameEventManager.cs

use std::sync::Arc;
use parking_lot::RwLock;

/// 游戏事件
#[derive(Debug, Clone)]
pub enum GameEvent {
    TurnStart(bool),           // 回合开始 (own)
    TurnEnd(bool),
    CardPlayed(i32),           // 卡牌打出 (entity_id)
    MinionSummoned(i32),
    MinionDied(i32),
    MinionAttacked(i32, i32), // (attacker, target)
    HeroAttacked(i32, i32),
    SecretRevealed(i32),
    DamageDealt(i32, i32, i32), // (source, target, amount)
    HealDealt(i32, i32, i32),
    GameStart,
    GameEnd,
    SceneChanged(String),
}

/// 事件监听器
pub type EventListener = Arc<dyn Fn(GameEvent) + Send + Sync>;

/// 事件管理器
pub struct EventManager {
    listeners: RwLock<Vec<(String, EventListener)>>,
}

impl EventManager {
    pub fn new() -> Self {
        Self { listeners: RwLock::new(Vec::new()) }
    }

    pub fn subscribe(&self, name: &str, listener: EventListener) {
        self.listeners.write().push((name.to_string(), listener));
    }

    pub fn emit(&self, event: GameEvent) {
        let listeners = self.listeners.read();
        for (_, listener) in listeners.iter() {
            listener(event.clone());
        }
    }
}
