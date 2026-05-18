//! 默认策略 — 使用 Silverfish AI 引擎做决策
//!
//! 实现 Routine trait，集成 hb-silverfish-core 的搜索算法。
//! 在无游戏连接时使用模拟数据测试。

use crate::{BotError, Routine};
use std::sync::Arc;

/// 默认策略（Silverfish AI）
pub struct DefaultRoutine {
    name: &'static str,
    author: &'static str,
    description: &'static str,
}

impl Default for DefaultRoutine {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultRoutine {
    pub fn new() -> Self {
        Self {
            name: "DefaultRoutine",
            author: "Hearthbuddy Rust",
            description: "Silverfish AI engine - Hearthstone bot decision maker",
        }
    }
}

impl Routine for DefaultRoutine {
    fn name(&self) -> &'static str {
        self.name
    }
    fn author(&self) -> &'static str {
        self.author
    }
    fn description(&self) -> &'static str {
        self.description
    }

    fn our_turn_logic(&self, state: &hb_ipc::GameStateData) -> Result<(), BotError> {
        tracing::info!("DefaultRoutine: turn={} scene={:?}", state.turn, state.scene);

        let mut pf = hb_silverfish_core::playfield::Playfield::new();
        pf.mana = state.own_mana as i32;
        pf.own_max_mana = state.own_max_mana as i32;
        pf.enemy_max_mana = state.own_max_mana as i32;
        pf.is_own_turn = state.is_own_turn;
        pf.own_hero.hp = state.own_hero.health;
        pf.own_hero.armor = state.own_hero.armor;
        pf.own_hero.angr = state.own_hero.attack;
        pf.own_hero.entity_id = state.own_hero.entity_id;
        pf.enemy_hero.hp = state.enemy_hero.health;
        pf.enemy_hero.armor = state.enemy_hero.armor;
        pf.enemy_hero.angr = state.enemy_hero.attack;
        pf.enemy_hero.entity_id = state.enemy_hero.entity_id;
        
        for (i, card) in state.own_hand.iter().enumerate() {
            pf.own_hand.push(hb_silverfish_core::playfield::HandCard {
                card_id: card.card_id.parse::<u32>().unwrap_or(0),
                entity_id: card.entity_id, position: i as i32,
                cost: card.cost, original_cost: card.cost,
                attack: card.attack, health: card.health,
                card_type: hb_silverfish_core::CardType::Minion,
                race: hb_silverfish_core::Race::None,
                is_choice: false, has_targets: false,
                is_tradeable: false, is_forge: false,
            });
        }
        for minion in &state.own_minions {
            let mut m = hb_silverfish_core::minion::Minion::new_minion(
                minion.card_id.parse::<u32>().unwrap_or(0), minion.attack, minion.health);
            m.entity_id = minion.entity_id; m.taunt = minion.has_taunt;
            m.divine_shield = minion.has_divine_shield; m.stealth = minion.has_stealth;
            m.poisonous = minion.has_poisonous; m.lifesteal = minion.has_lifesteal;
            m.ready = !minion.is_exhausted;
            pf.own_minions.push(m);
        }
        for minion in &state.enemy_minions {
            let mut m = hb_silverfish_core::minion::Minion::new_minion(
                minion.card_id.parse::<u32>().unwrap_or(0), minion.attack, minion.health);
            m.entity_id = minion.entity_id; m.taunt = minion.has_taunt;
            m.divine_shield = minion.has_divine_shield; m.stealth = minion.has_stealth;
            m.poisonous = minion.has_poisonous; m.lifesteal = minion.has_lifesteal;
            pf.enemy_minions.push(m);
        }
        
        let ai = hb_silverfish_core::ai::Ai::new();
        match ai.do_all(&pf) {
            Some(action) => tracing::info!("AI chose: {:?} (p={})", action.action_type, action.penality),
            None => tracing::warn!("AI no action"),
        }
        Ok(())
    }

    fn mulligan_logic(&self) -> Result<Vec<i32>, BotError> {
        // TODO: 实现留牌逻辑
        tracing::info!("DefaultRoutine: mulligan_logic called");
        Ok(vec![]) // 不留牌
    }
}

/// 创建默认策略实例
pub fn create_default() -> Arc<dyn Routine> {
    Arc::new(DefaultRoutine::new())
}
