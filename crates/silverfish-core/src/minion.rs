//! 随从/英雄数据结构
//!
//! 对应 C# 版的 Minion.cs

use crate::playfield::HandCard;
use crate::{CardId, Race};
use serde::{Deserialize, Serialize};

/// 随从/英雄
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Minion {
    pub entity_id: i32,
    pub card_id: CardId,
    pub name_index: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub angr: i32,
    pub base_angr: i32,
    pub base_hp: i32,
    pub armor: i32,
    pub handcard_count: i32,
    pub taunt: bool,
    pub divine_shield: bool,
    pub windfury: bool,
    pub mega_windfury: bool,
    pub stealth: bool,
    pub poisonous: bool,
    pub lifesteal: bool,
    pub rush: bool,
    pub charge: bool,
    pub reborn: bool,
    pub immune: bool,
    pub elusive: bool,
    pub titan: bool,
    pub untouchable: bool,
    pub ready: bool,
    pub just_played: bool,
    pub frozen: bool,
    pub attacking: bool,
    pub silenced: bool,
    pub transformed: bool,
    pub is_hero: bool,
    pub is_dormant: bool,
    pub num_attacks_this_turn: i32,
    pub num_turns_in_play: i32,
    pub zone_position: i32,
    pub handcard: Option<HandCard>,
    pub enchantments: Vec<Enchantment>,
    pub penality: i32,
    pub total_damage_dealt: i32,
    pub temp_angr_buff: i32,
    pub temp_hp_buff: i32,
    pub race: Race,
}

/// 附魔/Buff
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Enchantment {
    pub card_id: CardId,
    pub turns_remaining: i32,
    pub angr_buff: i32,
    pub hp_buff: i32,
    pub is_aura: bool,
    pub is_deathrattle: bool,
}

impl Minion {
    pub fn new_hero() -> Self {
        /* same as before */
        Self {
            entity_id: 0,
            card_id: 0,
            name_index: 0,
            hp: 30,
            max_hp: 30,
            angr: 0,
            base_angr: 0,
            base_hp: 30,
            armor: 0,
            handcard_count: 0,
            taunt: false,
            divine_shield: false,
            windfury: false,
            mega_windfury: false,
            stealth: false,
            poisonous: false,
            lifesteal: false,
            rush: false,
            charge: false,
            reborn: false,
            immune: false,
            elusive: false,
            titan: false,
            untouchable: false,
            ready: false,
            just_played: false,
            frozen: false,
            attacking: false,
            silenced: false,
            transformed: false,
            is_hero: true,
            is_dormant: false,
            num_attacks_this_turn: 0,
            num_turns_in_play: 0,
            zone_position: 0,
            handcard: None,
            enchantments: Vec::new(),
            penality: 0,
            total_damage_dealt: 0,
            temp_angr_buff: 0,
            temp_hp_buff: 0,
            race: Race::None,
        }
    }
    pub fn new_minion(card_id: CardId, angr: i32, hp: i32) -> Self {
        let mut m = Self::new_hero();
        m.is_hero = false;
        m.card_id = card_id;
        m.angr = angr;
        m.base_angr = angr;
        m.hp = hp;
        m.max_hp = hp;
        m.base_hp = hp;
        m
    }
    pub fn recalc_stats(&mut self) {
        /* same */
        let mut na = self.base_angr;
        let mut nh = self.base_hp;
        for e in &self.enchantments {
            na += e.angr_buff;
            nh += e.hp_buff;
        }
        self.angr = na.max(0);
        self.hp = nh.max(1);
        self.max_hp = self.hp.max(self.max_hp);
    }
    pub fn apply_enchantment(&mut self, ench: Enchantment) {
        self.enchantments.push(ench);
        self.recalc_stats();
    }
    pub fn remove_enchantments_by_source(&mut self, source_card_id: CardId) {
        self.enchantments.retain(|e| e.card_id != source_card_id);
        self.recalc_stats();
    }
    pub fn remove_all_enchantments(&mut self) {
        self.enchantments.clear();
        self.recalc_stats();
    }
    pub fn apply_silence(&mut self) {
        self.silenced = true;
        self.remove_all_enchantments();
        self.taunt = false;
        self.divine_shield = false;
        self.windfury = false;
        self.mega_windfury = false;
        self.stealth = false;
        self.poisonous = false;
        self.lifesteal = false;
        self.rush = false;
        self.charge = false;
        self.reborn = false;
        self.elusive = false;
        self.titan = false;
        self.temp_angr_buff = 0;
        self.temp_hp_buff = 0;
        self.angr = self.base_angr;
        self.hp = self.base_hp;
        self.max_hp = self.base_hp;
    }
    pub fn has_aura(&self) -> bool {
        self.enchantments.iter().any(|e| e.is_aura)
    }
    pub fn effective_angr(&self) -> i32 {
        (self.angr + self.temp_angr_buff).max(0)
    }
    pub fn effective_hp(&self) -> i32 {
        (self.hp + self.temp_hp_buff).max(1)
    }
    pub fn is_alive(&self) -> bool {
        self.hp > 0 || self.immune
    }
    pub fn can_attack(&self) -> bool {
        self.is_alive()
            && self.ready
            && !self.frozen
            && self.effective_angr() > 0
            && !self.is_dormant
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_minion_can_attack() {
        let mut m = Minion::new_minion(0, 3, 4);
        assert!(!m.can_attack());
        m.ready = true;
        assert!(m.can_attack());
        m.frozen = true;
        assert!(!m.can_attack());
    }
    #[test]
    fn test_effective_angr() {
        let m = Minion::new_minion(0, 5, 5);
        assert_eq!(m.effective_angr(), 5);
    }
}
