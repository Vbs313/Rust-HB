#![allow(dead_code)]
//! # hb-silverfish-core
//!
//! Silverfish AI 引擎核心
//!
//! 替代 C# 版的 Silverfish AI 引擎，实现炉石传说的局面模拟、搜索和决策。
//!
//! ## 架构
//!
//! ```text
//! Ai (AI 主控制器)
//! ├── Playfield (局面状态) ← 核心数据结构
//! │   ├── Minion (随从)
//! │   ├── Weapon (武器)
//! │   ├── HandCard (手牌)
//! │   └── Enchantment (附魔/Buff)
//! ├── MoveGenerator (动作生成)
//! │   ├── PlayCard / Attack / HeroPower / Location / Titan
//! │   └── PenalityManager (惩罚评估)
//! ├── MiniSimulator (DFS 搜索)
//! ├── EnemyTurnSimulator (敌方模拟)
//! └── Behavior (策略评估)
//!     └── BehaviorRush / BehaviorControl / ...
//! ```

pub mod action;
pub mod ai;
pub mod behavior;
pub mod card_db;
pub mod enemy_turn_simulator;
pub mod mini_simulator;
pub mod minion;
pub mod move_generator;
pub mod penality_manager;
pub mod playfield;
pub mod sim_template;
pub mod weapon;

use serde::{Deserialize, Serialize};

/// 卡牌 ID 类型（对应 C# 的 CardDB.cardIDEnum）
pub type CardId = u32;

/// 玩家标识
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Player {
    Own,
    Enemy,
}

impl Player {
    pub fn opposite(&self) -> Self {
        match self {
            Player::Own => Player::Enemy,
            Player::Enemy => Player::Own,
        }
    }
}

/// 种族枚举（对应炉石卡牌种族）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, enum_map::Enum)]
pub enum Race {
    None,
    Beast,
    Demon,
    Dragon,
    Elemental,
    Mech,
    Murloc,
    Naga,
    Pirate,
    Quilboar,
    Totem,
    Undead,
    All,
    // Pet (猎人宠物)
    // ... 更多
}

/// 卡牌类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardType {
    Invalid,
    Minion,
    Spell,
    Weapon,
    Hero,
    HeroPower,
    Location,
    Token,
}

/// 关键字枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Keyword {
    Taunt,
    DivineShield,
    Windfury,
    MegaWindfury,
    Stealth,
    Poisonous,
    Lifesteal,
    Rush,
    Charge,
    Reborn,
    Immune,
    Elusive,
    Frenzy,
    HonorableKill,
    Overkill,
    Spellburst,
    Infuse,
    Corrupt,
    Tradeable,
    Forge,
    Titan,
    Magnetic,
    Dormant,
}
