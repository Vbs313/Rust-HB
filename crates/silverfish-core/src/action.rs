//! 动作系统
//!
//! 对应 C# 版的 Action/action.cs 和 actionEnum

use crate::minion::Minion;
use crate::playfield::HandCard;
use serde::{Deserialize, Serialize};

/// 动作类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionType {
    EndTurn = 0,
    PlayCard,
    AttackWithHero,
    UseHeroPower,
    AttackWithMinion,
    Trade,
    UseLocation,
    UseTitanAbility,
    Forge,
    LaunchStarship,
    Rewind,
}

/// 一个具体的游戏动作
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub action_type: ActionType,
    pub hand_card: Option<HandCard>,
    pub source: Option<Minion>,
    pub target: Option<Minion>,
    pub position: i32,
    pub penality: i32,
    pub choice: i32,
}
