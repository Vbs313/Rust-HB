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

    fn our_turn_logic(&self) -> Result<(), BotError> {
        tracing::info!("DefaultRoutine: our_turn_logic called");

        // 创建一个模拟局面用于测试
        // 实际游戏中，这里会从 game-api 读取实时局面
        let mut pf = hb_silverfish_core::playfield::Playfield::new();

        // 设置基本状态
        pf.mana = 10;
        pf.own_max_mana = 10;
        pf.enemy_max_mana = 10;
        pf.is_own_turn = true;
        pf.own_hero.hp = 30;
        pf.enemy_hero.hp = 30;
        pf.own_hero.entity_id = 1;
        pf.enemy_hero.entity_id = 2;

        // 添加一些测试手牌
        pf.own_hand.push(hb_silverfish_core::playfield::HandCard {
            card_id: 602, // 北郡牧师
            entity_id: 10,
            position: 0,
            cost: 2,
            original_cost: 2,
            attack: 1,
            health: 3,
            card_type: hb_silverfish_core::CardType::Minion,
            race: hb_silverfish_core::Race::None,
            is_choice: false,
            has_targets: false,
            is_tradeable: false,
            is_forge: false,
        });

        // 添加一些敌方随从
        pf.enemy_minions
            .push(hb_silverfish_core::minion::Minion::new_minion(1001, 3, 3));
        pf.enemy_minions[0].entity_id = 20;

        // 使用 AI 引擎搜索最佳动作
        let ai = hb_silverfish_core::ai::Ai::new();
        let _move_gen = hb_silverfish_core::move_generator::MoveGenerator::new();
        let _behavior = hb_silverfish_core::behavior::default_behavior::DefaultBehavior;

        tracing::info!("Starting AI search...");
        let result = ai.do_all(&pf);
        match result {
            Some(action) => tracing::info!("AI chose: {:?}", action.action_type),
            None => tracing::warn!("AI found no valid action"),
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
