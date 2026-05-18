//! 卡牌数据库
//!
//! 对应 C# 版的 CardDB 系统。
//! 使用 HashMap 实现 O(1) 卡牌查询。

use crate::{CardId, Race};
use std::collections::HashMap;

/// 卡牌数据
#[derive(Debug, Clone)]
pub struct Card {
    pub id: CardId,
    pub card_id_str: String,
    pub cost: i32,
    pub attack: i32,
    pub health: i32,
    pub card_type: crate::CardType,
    pub race: Race,
    pub name_cn: String,
    pub name_en: String,

    // 关键字
    pub has_taunt: bool,
    pub has_divine_shield: bool,
    pub has_charge: bool,
    pub has_windfury: bool,
    pub has_stealth: bool,
    pub has_poisonous: bool,
    pub has_lifesteal: bool,
    pub has_rush: bool,
    pub has_reborn: bool,
    pub has_elusive: bool,
    pub has_immune: bool,
    pub has_mega_windfury: bool,

    // 扩展属性
    pub set_id: u32,
    pub rarity: u32,
    pub collectible: bool,
}

/// 卡牌数据库
pub struct CardDb {
    cards_by_id: HashMap<CardId, Card>,
    cards_by_str: HashMap<String, CardId>,
}

impl CardDb {
    pub fn new(cards: Vec<Card>) -> Self {
        let mut by_id = HashMap::new();
        let mut by_str = HashMap::new();

        for card in cards {
            let str_id = card.card_id_str.clone();
            let num_id = card.id;
            by_id.insert(num_id, card);
            by_str.insert(str_id, num_id);
        }

        Self {
            cards_by_id: by_id,
            cards_by_str: by_str,
        }
    }

    pub fn get_by_id(&self, id: CardId) -> Option<&Card> {
        self.cards_by_id.get(&id)
    }

    pub fn get_by_str(&self, str_id: &str) -> Option<&Card> {
        self.cards_by_str
            .get(str_id)
            .and_then(|num_id| self.cards_by_id.get(num_id))
    }

    pub fn get_cost(&self, id: CardId) -> i32 {
        self.get_by_id(id).map(|c| c.cost).unwrap_or(0)
    }

    pub fn get_attack(&self, id: CardId) -> i32 {
        self.get_by_id(id).map(|c| c.attack).unwrap_or(0)
    }

    pub fn get_health(&self, id: CardId) -> i32 {
        self.get_by_id(id).map(|c| c.health).unwrap_or(0)
    }

    /// 卡牌是否有指定关键字
    pub fn has_keyword(&self, id: CardId, keyword: &str) -> bool {
        self.get_by_id(id)
            .map(|c| match keyword {
                "taunt" => c.has_taunt,
                "divine_shield" => c.has_divine_shield,
                "charge" => c.has_charge,
                "windfury" => c.has_windfury,
                "stealth" => c.has_stealth,
                "poisonous" => c.has_poisonous,
                "lifesteal" => c.has_lifesteal,
                "rush" => c.has_rush,
                "reborn" => c.has_reborn,
                _ => false,
            })
            .unwrap_or(false)
    }

    /// 检查是否是亡语随从
    pub fn has_deathrattle(&self, _id: CardId) -> bool {
        // TODO: 从卡牌文本中提取
        false
    }

    /// 卡牌数量
    pub fn count(&self) -> usize {
        self.cards_by_id.len()
    }
}

impl Default for CardDb {
    fn default() -> Self {
        Self {
            cards_by_id: HashMap::new(),
            cards_by_str: HashMap::new(),
        }
    }
}
