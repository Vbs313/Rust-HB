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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_type_variants() {
        let types = [
            ActionType::EndTurn,
            ActionType::PlayCard,
            ActionType::AttackWithHero,
            ActionType::UseHeroPower,
            ActionType::AttackWithMinion,
            ActionType::Trade,
            ActionType::UseLocation,
            ActionType::UseTitanAbility,
            ActionType::Forge,
            ActionType::LaunchStarship,
            ActionType::Rewind,
        ];
        assert_eq!(types.len(), 11);
        // 枚举值应互不相等
        for i in 0..types.len() {
            for j in i + 1..types.len() {
                assert_ne!(types[i] as i32, types[j] as i32);
            }
        }
    }

    #[test]
    fn test_action_construction() {
        let action = Action {
            action_type: ActionType::EndTurn,
            hand_card: None,
            source: None,
            target: None,
            position: 0,
            penality: 0,
            choice: 0,
        };
        assert_eq!(action.action_type, ActionType::EndTurn);
        assert_eq!(action.penality, 0);
        assert!(action.hand_card.is_none());
        assert!(action.source.is_none());
    }

    #[test]
    fn test_action_penality_ordering() {
        let a1 = Action {
            action_type: ActionType::PlayCard,
            hand_card: None,
            source: None,
            target: None,
            position: 0,
            penality: 10,
            choice: 0,
        };
        let a2 = Action {
            action_type: ActionType::AttackWithMinion,
            hand_card: None,
            source: None,
            target: None,
            position: 0,
            penality: 50,
            choice: 0,
        };
        assert!(a1.penality < a2.penality);
    }
}
