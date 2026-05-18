//! 武器数据结构
//!
//! 对应 C# 版的 Weapon.cs

use crate::CardId;
use serde::{Deserialize, Serialize};

/// 武器
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Weapon {
    pub entity_id: i32,
    pub card_id: CardId,
    pub angr: i32,
    pub durability: i32,
    pub base_angr: i32,
    pub base_durability: i32,
    pub windfury: bool,
    pub poisonous: bool,
    pub lifesteal: bool,
    pub immune: bool,
    pub mega_windfury: bool,
}

impl Default for Weapon {
    fn default() -> Self {
        Self::new()
    }
}

impl Weapon {
    pub fn new() -> Self {
        Self {
            entity_id: 0,
            card_id: 0,
            angr: 0,
            durability: 0,
            base_angr: 0,
            base_durability: 0,
            windfury: false,
            poisonous: false,
            lifesteal: false,
            immune: false,
            mega_windfury: false,
        }
    }
    pub fn is_equipped(&self) -> bool {
        self.durability > 0 && self.angr > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weapon_default() {
        let w = Weapon::new();
        assert_eq!(w.angr, 0);
        assert_eq!(w.durability, 0);
        assert!(!w.is_equipped());
    }

    #[test]
    fn test_weapon_is_equipped() {
        let w = Weapon {
            angr: 3,
            durability: 2,
            ..Weapon::new()
        };
        assert!(w.is_equipped());
    }

    #[test]
    fn test_weapon_broken() {
        let w = Weapon {
            angr: 3,
            durability: 0,
            ..Weapon::new()
        };
        assert!(!w.is_equipped(), "Durability 0 = broken");
    }

    #[test]
    fn test_weapon_no_attack() {
        let w = Weapon {
            angr: 0,
            durability: 2,
            ..Weapon::new()
        };
        assert!(!w.is_equipped(), "0 attack = not a weapon");
    }

    #[test]
    fn test_weapon_keywords() {
        let w = Weapon {
            windfury: true,
            poisonous: true,
            lifesteal: true,
            ..Weapon::new()
        };
        assert!(w.windfury);
        assert!(w.poisonous);
        assert!(w.lifesteal);
    }

    #[test]
    fn test_weapon_serde_roundtrip() {
        let w = Weapon {
            entity_id: 1,
            card_id: 2001,
            angr: 5,
            durability: 3,
            base_angr: 5,
            base_durability: 3,
            windfury: true,
            poisonous: false,
            lifesteal: false,
            immune: false,
            mega_windfury: false,
        };
        let json = serde_json::to_string(&w).unwrap();
        let w2: Weapon = serde_json::from_str(&json).unwrap();
        assert_eq!(w.entity_id, w2.entity_id);
        assert_eq!(w.angr, w2.angr);
        assert_eq!(w.windfury, w2.windfury);
    }
}
