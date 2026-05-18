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
