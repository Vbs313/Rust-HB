//! 局面状态 (Playfield)
//!
//! Silverfish AI 最核心的数据结构，表示一局游戏的完整快照。
//! 对应 C# 版 Playfield.cs 的 340+ 字段和所有操作方法。

use crate::action::{Action, ActionType};
use crate::card_db::CardDb;
use crate::minion::Minion;
use crate::weapon::Weapon;
use crate::{CardId, CardType, Race};
use serde::{Deserialize, Serialize};

/// 游戏局面快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playfield {
    // ===== 资源 =====
    /// 当前可用法力
    pub mana: i32,
    /// 我方最大法力水晶
    pub own_max_mana: i32,
    /// 敌方最大法力水晶
    pub enemy_max_mana: i32,
    /// 本回合已用法力
    pub mana_spent_this_turn: i32,
    /// 临时法力（生长/激活等）
    pub temporary_mana: i32,

    // ===== 英雄 =====
    pub own_hero: Minion,
    pub enemy_hero: Minion,
    pub own_hero_power: Option<CardId>,
    pub enemy_hero_power: Option<CardId>,

    // ===== 随从 =====
    pub own_minions: Vec<Minion>,
    pub enemy_minions: Vec<Minion>,

    // ===== 手牌 =====
    pub own_hand: Vec<HandCard>,
    pub enemy_hand_count: i32,

    // ===== 武器 =====
    pub own_weapon: Option<Weapon>,
    pub enemy_weapon: Option<Weapon>,

    // ===== 牌库 =====
    pub own_deck: Vec<CardId>,
    pub enemy_deck_count: i32,
    pub own_deck_count: i32,

    // ===== 秘密/任务 =====
    pub own_secrets: Vec<String>,
    pub enemy_secrets: Vec<(i32, i32)>, // (entity_id, class_tag)
    pub own_quest: Option<QuestItem>,
    pub enemy_quest: Option<QuestItem>,

    // ===== 状态 =====
    pub complete: bool,
    pub is_own_turn: bool,
    pub is_lethal_check: bool,
    pub turn_counter: i32,
    pub options_played_this_turn: i32,
    pub cards_played_this_turn: i32,
    pub minions_summoned_this_turn: i32,
    pub num_cards_drawn_this_turn: i32,
    pub damage_dealt_to_enemy_hero_this_turn: i32,

    // ===== 评估结果 =====
    pub value: f32,
    pub evaluate_penality: i32,
    pub play_actions: Vec<Action>,
    pub next_playfields: Vec<Playfield>,

    // ===== 搜索辅助 =====
    pub hash_code: i64,
    pub p_id_history: Vec<i64>,
    pub best_enemy_play: Option<Box<Playfield>>,
    pub end_turn_state: Option<Box<Playfield>>,
}

/// 手牌
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandCard {
    pub card_id: CardId,
    pub entity_id: i32,
    pub position: i32,      // 手牌位置 (0-based)
    pub cost: i32,          // 当前费用（含减费效果）
    pub original_cost: i32, // 原始费用
    pub attack: i32,
    pub health: i32,
    pub card_type: CardType,
    pub race: Race,
    pub is_choice: bool,    // 是否是抉择卡
    pub has_targets: bool,  // 是否需要选择目标
    pub is_tradeable: bool, // 可交易
    pub is_forge: bool,     // 可锻造
}

/// 任务/任务线
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestItem {
    pub card_id: CardId,
    pub progress: i32,
    pub max_progress: i32,
    pub is_questline: bool,
    pub reward_card_id: CardId,
}

impl Playfield {
    /// 创建空局面
    pub fn new() -> Self {
        Self {
            mana: 0,
            own_max_mana: 0,
            enemy_max_mana: 0,
            mana_spent_this_turn: 0,
            temporary_mana: 0,
            own_hero: Minion::new_hero(),
            enemy_hero: Minion::new_hero(),
            own_hero_power: None,
            enemy_hero_power: None,
            own_minions: Vec::with_capacity(7),
            enemy_minions: Vec::with_capacity(7),
            own_hand: Vec::with_capacity(10),
            enemy_hand_count: 0,
            own_weapon: None,
            enemy_weapon: None,
            own_deck: Vec::new(),
            enemy_deck_count: 0,
            own_deck_count: 0,
            own_secrets: Vec::new(),
            enemy_secrets: Vec::new(),
            own_quest: None,
            enemy_quest: None,
            complete: false,
            is_own_turn: true,
            is_lethal_check: false,
            turn_counter: 1,
            options_played_this_turn: 0,
            cards_played_this_turn: 0,
            minions_summoned_this_turn: 0,
            num_cards_drawn_this_turn: 0,
            damage_dealt_to_enemy_hero_this_turn: 0,
            value: 0.0,
            evaluate_penality: 0,
            play_actions: Vec::new(),
            next_playfields: Vec::new(),
            hash_code: 0,
            p_id_history: Vec::new(),
            best_enemy_play: None,
            end_turn_state: None,
        }
    }

    /// 深拷贝（搜索时频繁调用）
    pub fn deep_clone(&self) -> Self {
        // 手动克隆所有字段（确保 Vec/Box 都是全新分配）
        self.clone()
    }

    // ===== 核心操作方法 =====

    /// 执行一个动作，更新场面
    pub fn do_action(&mut self, action: &Action, db: &CardDb) {
        match action.action_type {
            ActionType::PlayCard => self.play_card(action, db),
            ActionType::AttackWithMinion => self.minion_attack(action, db),
            ActionType::AttackWithHero => self.hero_attack(action, db),
            ActionType::UseHeroPower => self.use_hero_power(action, db),
            ActionType::EndTurn => self.end_turn(),
            ActionType::Trade => self.trade_card(action, db),
            ActionType::Forge => self.forge_card(action, db),
            ActionType::UseLocation => self.use_location(action, db),
            ActionType::UseTitanAbility => self.use_titan_ability(action, db),
            _ => {}
        }
        // 动作后处理：记录动作到历史
        self.play_actions.push(action.clone());
        self.options_played_this_turn += 1;
        self.hash_code = self.compute_hash();
    }

    // ===== 动作执行方法 =====

    /// 打出卡牌
    fn play_card(&mut self, action: &Action, db: &CardDb) {
        let hand_card = match &action.hand_card {
            Some(hc) => hc,
            None => return,
        };

        // 扣除费用
        self.mana -= hand_card.cost;
        self.mana_spent_this_turn += hand_card.cost;

        // 从手牌中移除
        if let Some(pos) = self
            .own_hand
            .iter()
            .position(|c| c.entity_id == hand_card.entity_id)
        {
            self.own_hand.remove(pos);
        }

        self.cards_played_this_turn += 1;

        // 根据卡牌类型执行不同操作
        match hand_card.card_type {
            CardType::Minion => {
                let pos = if action.position >= 0 {
                    action.position as usize
                } else {
                    self.own_minions.len()
                };
                self.summon_minion(hand_card.card_id, pos, true);
            }
            CardType::Spell => {
                // 法术：需要从 db 获取效果并应用
                // TODO: 调用 SimTemplate.on_card_play
                self.spell_damage(hand_card, action, db);
            }
            CardType::Weapon => {
                // 装备武器
                self.equip_weapon(hand_card.card_id, db);
            }
            CardType::Hero => {
                // 变身英雄
                self.transform_hero(hand_card.card_id, db);
            }
            CardType::Location => {
                // 放置地标
                // TODO: 地标系统
            }
            _ => {}
        }
    }

    /// 法术伤害处理
    fn spell_damage(&mut self, _card: &HandCard, action: &Action, _db: &CardDb) {
        if let Some(ref target) = action.target {
            // 对目标造成伤害
            self.minion_get_damage_or_heal(
                target.entity_id,
                target.is_hero,
                false, // 敌方
                _db.get_attack(_card.card_id),
                false,
            );
        }
    }

    /// 装备武器
    fn equip_weapon(&mut self, card_id: CardId, db: &CardDb) {
        if let Some(card) = db.get_by_id(card_id) {
            let weapon = Weapon {
                entity_id: -1,
                card_id,
                angr: card.attack,
                durability: card.health,
                base_angr: card.attack,
                base_durability: card.health,
                windfury: card.has_windfury,
                poisonous: card.has_poisonous,
                lifesteal: card.has_lifesteal,
                immune: card.has_immune,
                mega_windfury: card.has_mega_windfury,
            };
            self.own_weapon = Some(weapon);
        }
    }

    /// 变身英雄
    fn transform_hero(&mut self, _card_id: CardId, _db: &CardDb) {
        // TODO: 更新英雄属性、护甲、英雄技能
        self.own_hero.armor += 5; // 大多数英雄牌给 5 甲
    }

    /// 抽牌
    pub fn draw_card(&mut self, specific_card: Option<CardId>) -> Option<CardId> {
        self.num_cards_drawn_this_turn += 1;

        if self.own_deck.is_empty() {
            // 疲劳伤害（第 n 张抽牌 = 第 n 点疲劳）
            self.own_hero.hp -= self.num_cards_drawn_this_turn;
            self.own_deck_count = 0;
            return None;
        }

        let card = if let Some(cid) = specific_card {
            if let Some(pos) = self.own_deck.iter().position(|c| *c == cid) {
                self.own_deck.remove(pos)
            } else {
                self.own_deck.remove(0)
            }
        } else {
            self.own_deck.remove(0)
        };

        self.own_deck_count = self.own_deck.len() as i32;
        Some(card)
    }

    /// 召唤随从（own=true 为我方）
    pub fn summon_minion(&mut self, card_id: CardId, position: usize, own: bool) -> Option<usize> {
        let minions = if own {
            &mut self.own_minions
        } else {
            &mut self.enemy_minions
        };

        if minions.len() >= 7 {
            return None; // 满场
        }

        // 创建随从
        let pos = position.min(minions.len());
        let minion = Minion::new_minion(card_id, 0, 1);

        minions.insert(pos, minion);

        if own {
            self.minions_summoned_this_turn += 1;
        }

        // 更新位置信息
        for (i, m) in minions.iter_mut().enumerate() {
            m.zone_position = i as i32;
        }

        Some(pos)
    }

    /// 从场上移除随从
    pub fn remove_minion(&mut self, entity_id: i32, own: bool) {
        let minions = if own {
            &mut self.own_minions
        } else {
            &mut self.enemy_minions
        };
        minions.retain(|m| m.entity_id != entity_id);
        // 重新编号位置
        for (i, m) in minions.iter_mut().enumerate() {
            m.zone_position = i as i32;
        }
    }

    /// 随从攻击（先读双方属性，分别应用伤害，最后统一清理死亡）
    fn minion_attack(&mut self, action: &Action, _db: &CardDb) {
        let attacker_id = action.source.as_ref().map(|s| s.entity_id).unwrap_or(-1);
        let target_id = action.target.as_ref().map(|t| t.entity_id).unwrap_or(-1);

        let Some(idx) = self
            .own_minions
            .iter()
            .position(|m| m.entity_id == attacker_id)
        else {
            return;
        };
        let target_is_enemy_hero = target_id == self.enemy_hero.entity_id;
        let angr = self.own_minions[idx].effective_angr();
        let has_lifesteal = self.own_minions[idx].lifesteal;
        let has_poisonous = self.own_minions[idx].poisonous;

        if target_is_enemy_hero {
            self.minion_get_damage_or_heal(target_id, true, true, angr, has_poisonous);
            if has_lifesteal {
                self.own_hero.hp = (self.own_hero.hp + angr).min(self.own_hero.max_hp);
            }
            self.own_minions[idx].num_attacks_this_turn += 1;
            if !self.own_minions[idx].windfury || self.own_minions[idx].num_attacks_this_turn >= 2 {
                self.own_minions[idx].ready = false;
            }
            self.damage_dealt_to_enemy_hero_this_turn += angr;
        } else {
            // 先读取双方属性，再用原始伤害值先后修改，最后统一清理
            let (target_hp_before, target_angr, target_has_divine, target_has_poison) =
                match self.enemy_minions.iter().find(|m| m.entity_id == target_id) {
                    Some(t) => (t.hp, t.effective_angr(), t.divine_shield, t.poisonous),
                    None => return,
                };

            // 1) 攻击方对目标造成伤害
            if target_has_divine && angr > 0 {
                if let Some(t) = self
                    .enemy_minions
                    .iter_mut()
                    .find(|m| m.entity_id == target_id)
                {
                    t.divine_shield = false;
                }
            } else if let Some(t) = self
                .enemy_minions
                .iter_mut()
                .find(|m| m.entity_id == target_id)
            {
                if has_poisonous {
                    t.hp = 0;
                } else {
                    t.hp -= angr;
                }
            }

            // 2) 反击伤害（同时结算——目标即使会死也仍然反击）
            if (target_hp_before > 0 || target_has_divine) && target_angr > 0 {
                if let Some(a) = self
                    .own_minions
                    .iter_mut()
                    .find(|m| m.entity_id == attacker_id)
                {
                    if target_has_poison {
                        a.hp = 0;
                    } else {
                        a.hp -= target_angr;
                    }
                }
            }

            // 吸血
            if has_lifesteal {
                self.own_hero.hp = (self.own_hero.hp + angr).min(self.own_hero.max_hp);
            }

            self.own_minions[idx].num_attacks_this_turn += 1;
            if !self.own_minions[idx].windfury || self.own_minions[idx].num_attacks_this_turn >= 2 {
                self.own_minions[idx].ready = false;
            }
        }

        // 统一清理死亡
        self.cleanup_dead();
    }

    /// 英雄攻击（有武器时）
    fn hero_attack(&mut self, action: &Action, _db: &CardDb) {
        let target_id = action.target.as_ref().map(|t| t.entity_id).unwrap_or(-1);
        let target_is_enemy_hero = target_id == self.enemy_hero.entity_id;

        let weapon = match &self.own_weapon {
            Some(w) => w.clone(),
            None => return,
        };

        let angr = weapon.angr;
        let target_is_enemy_minion = self.enemy_minions.iter().any(|m| m.entity_id == target_id);

        if target_is_enemy_hero {
            // 砍脸
            self.minion_get_damage_or_heal(target_id, true, true, angr, false);
            self.damage_dealt_to_enemy_hero_this_turn += angr;
        } else if target_is_enemy_minion {
            // 砍随从
            self.minion_get_damage_or_heal(target_id, false, true, angr, false);
        }

        // 武器减耐久
        if let Some(ref mut w) = self.own_weapon {
            w.durability -= 1;
            if w.durability <= 0 {
                self.own_weapon = None;
            }
        }

        // 英雄不能继续攻击
        self.own_hero.ready = false;
        self.own_hero.num_attacks_this_turn += 1;
    }

    /// 对指定实体造成伤害或治疗
    /// entity_id: 目标实体 ID
    /// is_hero: 是否是英雄
    /// is_enemy: 是否是敌方
    /// amount: 伤害/治疗量（正=伤害，负=治疗）
    /// is_poisonous: 是否有剧毒（直接消灭）
    pub fn minion_get_damage_or_heal(
        &mut self,
        entity_id: i32,
        is_hero: bool,
        is_enemy: bool,
        amount: i32,
        is_poisonous: bool,
    ) {
        if amount <= 0 {
            return;
        }

        if is_hero {
            let hero = if is_enemy {
                &mut self.enemy_hero
            } else {
                &mut self.own_hero
            };

            // 免疫检查
            if hero.immune {
                return;
            }

            // 先扣护甲
            let mut remaining = amount;
            if hero.armor > 0 {
                let armor_damage = hero.armor.min(remaining);
                hero.armor -= armor_damage;
                remaining -= armor_damage;
            }
            hero.hp -= remaining;

            // 圣盾检查
            if hero.divine_shield && remaining > 0 {
                hero.divine_shield = false;
                hero.hp += remaining; // 圣盾抵消伤害
            }

            // 受伤触发
            if remaining > 0 {
                self.trigger_on_damage(entity_id, remaining);
            }
        } else if is_enemy {
            // 敌方随从
            if let Some(minion) = self
                .enemy_minions
                .iter_mut()
                .find(|m| m.entity_id == entity_id)
            {
                if minion.immune {
                    return;
                }
                if is_poisonous {
                    minion.hp = 0;
                } else if minion.divine_shield && amount > 0 {
                    minion.divine_shield = false;
                } else {
                    minion.hp -= amount;
                }
                self.trigger_on_damage(entity_id, amount);
            }
        } else {
            // 我方随从
            if let Some(minion) = self
                .own_minions
                .iter_mut()
                .find(|m| m.entity_id == entity_id)
            {
                if minion.immune {
                    return;
                }
                if is_poisonous {
                    minion.hp = 0;
                } else if minion.divine_shield && amount > 0 {
                    minion.divine_shield = false;
                } else {
                    minion.hp -= amount;
                }
                self.trigger_on_damage(entity_id, amount);
            }
        }

        // 清理死亡实体
        self.cleanup_dead();
    }

    /// 清理死亡实体 + 触发亡语
    fn cleanup_dead(&mut self) {
        // 收集已死亡的实体（亡语触发需要它们在移除前存在）
        let dead_own: Vec<(i32, usize)> = self
            .own_minions
            .iter()
            .enumerate()
            .filter(|(_, m)| m.hp <= 0)
            .map(|(i, m)| (m.entity_id, i))
            .collect();
        let dead_enemy: Vec<(i32, usize)> = self
            .enemy_minions
            .iter()
            .enumerate()
            .filter(|(_, m)| m.hp <= 0)
            .map(|(i, m)| (m.entity_id, i))
            .collect();

        // 触发亡语（在移除前调用，使亡语效果能访问死亡随从的状态）
        for &(eid, _) in &dead_own {
            self.trigger_deathrattle(eid, true);
        }
        for &(eid, _) in &dead_enemy {
            self.trigger_deathrattle(eid, false);
        }

        // 移除死亡实体
        self.own_minions.retain(|m| m.hp > 0);
        self.enemy_minions.retain(|m| m.hp > 0);

        // 重新编号
        for (i, m) in self.own_minions.iter_mut().enumerate() {
            m.zone_position = i as i32;
        }
        for (i, m) in self.enemy_minions.iter_mut().enumerate() {
            m.zone_position = i as i32;
        }
    }

    // ============================================================
    // 触发系统
    // ============================================================

    /// 触发随从的亡语
    fn trigger_deathrattle(&mut self, _entity_id: i32, _own: bool) {
        // 查找亡语卡牌的 SimTemplate 并调用 on_deathrattle
        // 目前还未实现卡牌模拟注册表
        // TODO: 从 sim_cards crate 获取 CardSim 实例
        //     let sim = get_sim(card_id);
        //     sim.on_deathrattle(self, dead_minion);
    }

    /// 触发受伤事件（调用所有需要监听受伤的 SimTemplate）
    pub fn trigger_on_damage(&mut self, _target_entity_id: i32, _amount: i32) {
        // 遍历场上所有随从，对有关联 SimTemplate 的调用 on_minion_got_dmg_trigger
        // TODO: 实现完整的触发链
    }

    /// 触发治疗事件
    pub fn trigger_on_heal(&mut self, _target_entity_id: i32, _amount: i32) {
        // TODO: 同 trigger_on_damage
    }

    /// 触发召唤事件
    pub fn trigger_on_summon(&mut self, _summoned_entity_id: i32) {
        // TODO: 遍历场上随从，调用 on_summon
    }

    /// 触发回合开始事件
    pub fn trigger_turn_start(&mut self) {
        // TODO: 遍历有回合开始触发的随从
    }

    /// 触发回合结束事件
    pub fn trigger_turn_end(&mut self) {
        // TODO: 遍历有回合结束触发的随从
    }

    /// 使用英雄技能
    fn use_hero_power(&mut self, _action: &Action, _db: &CardDb) {
        if self.mana < 2 {
            return; // 英雄技能通常消耗 2 费
        }
        self.mana -= 2;
        self.mana_spent_this_turn += 2;

        // TODO: 根据不同的英雄技能执行不同效果
        // 默认给英雄加 2 甲（对应战士/德鲁伊等）
        self.own_hero.armor += 2;
    }

    /// 交易
    fn trade_card(&mut self, action: &Action, _db: &CardDb) {
        let hand_card = match &action.hand_card {
            Some(hc) => hc,
            None => return,
        };
        // 交易：消耗 1 费，将该牌洗回牌库，抽一张新牌
        if self.mana < 1 {
            return;
        }
        self.mana -= 1;
        self.mana_spent_this_turn += 1;

        // 从手牌移除
        self.own_hand.retain(|c| c.entity_id != hand_card.entity_id);
        // 洗回牌库（放入随机位置）
        self.own_deck.push(hand_card.card_id);
        // 抽一张牌
        self.draw_card(None);
    }

    /// 锻造
    fn forge_card(&mut self, _action: &Action, _db: &CardDb) {
        // TODO: 锻造：消耗 2 费，升级手牌中的可锻造牌
    }

    /// 使用地标
    fn use_location(&mut self, _action: &Action, _db: &CardDb) {
        // 地标：需要检查可用次数（耐久）和使用费用
        // 当前简化：消耗 1 次地标耐久
        // TODO: 从场上找到对应地标实体，调用其卡牌效果
    }

    /// 使用泰坦技能
    fn use_titan_ability(&mut self, _action: &Action, _db: &CardDb) {
        // 泰坦：消耗 1 次可用技能次数
        // 回合开始时恢复 3 次可用次数
        // TODO: 从场上找到对应泰坦随从，执行指定技能编号的效果
    }

    // ============================================================
    // 光环系统
    // ============================================================

    /// 刷新全场光环效果（先清空临时加成，再重新计算）
    pub fn update_auras(&mut self) {
        // 1. 清空临时加成
        for m in &mut self.own_minions {
            m.temp_angr_buff = 0;
            m.temp_hp_buff = 0;
        }
        for m in &mut self.enemy_minions {
            m.temp_angr_buff = 0;
            m.temp_hp_buff = 0;
        }
        self.own_hero.temp_angr_buff = 0;
        self.own_hero.temp_hp_buff = 0;
        self.enemy_hero.temp_angr_buff = 0;
        self.enemy_hero.temp_hp_buff = 0;

        // 2. 收集光环来源（先取 ID 列表避开借用冲突）
        let own_auras: Vec<i32> = self
            .own_minions
            .iter()
            .filter(|m| m.has_aura())
            .map(|m| m.entity_id)
            .collect();
        let enemy_auras: Vec<i32> = self
            .enemy_minions
            .iter()
            .filter(|m| m.has_aura())
            .map(|m| m.entity_id)
            .collect();

        // 3. 应用我方光环（默认 +1/+1，后续根据具体卡牌细化）
        for aura_id in &own_auras {
            for m in &mut self.own_minions {
                if m.entity_id != *aura_id {
                    m.temp_angr_buff += 1;
                    m.temp_hp_buff += 1;
                }
            }
        }
        // 4. 应用敌方光环
        for aura_id in &enemy_auras {
            for m in &mut self.enemy_minions {
                if m.entity_id != *aura_id {
                    m.temp_angr_buff += 1;
                    m.temp_hp_buff += 1;
                }
            }
        }
    }

    /// 计算局面哈希（用于去重）
    fn compute_hash(&self) -> i64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();

        // 散列关键状态
        self.mana.hash(&mut hasher);
        self.own_hero.hp.hash(&mut hasher);
        self.enemy_hero.hp.hash(&mut hasher);
        self.own_hero.armor.hash(&mut hasher);
        self.enemy_hero.armor.hash(&mut hasher);

        // 散列随从
        for m in &self.own_minions {
            m.entity_id.hash(&mut hasher);
            m.hp.hash(&mut hasher);
            m.angr.hash(&mut hasher);
            m.taunt.hash(&mut hasher);
            m.divine_shield.hash(&mut hasher);
        }
        for m in &self.enemy_minions {
            m.entity_id.hash(&mut hasher);
            m.hp.hash(&mut hasher);
            m.angr.hash(&mut hasher);
        }

        hasher.finish() as i64
    }

    /// 结束回合
    pub fn end_turn(&mut self) {
        self.complete = true;
        self.is_own_turn = false;
        // 重置回合相关状态
        self.options_played_this_turn = 0;
        self.cards_played_this_turn = 0;
        self.minions_summoned_this_turn = 0;
        self.num_cards_drawn_this_turn = 0;
        self.damage_dealt_to_enemy_hero_this_turn = 0;

        // 随从获得"可以行动"标记
        for minion in &mut self.own_minions {
            if !minion.just_played {
                minion.ready = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::ActionType;
    use crate::minion::Enchantment;

    fn make_minion(eid: i32, angr: i32, hp: i32) -> Minion {
        let mut m = Minion::new_minion(eid as CardId, angr, hp);
        m.entity_id = eid;
        m.hp = hp;
        m.angr = angr;
        m
    }

    #[test]
    fn test_new_playfield() {
        let pf = Playfield::new();
        assert_eq!(pf.mana, 0);
        assert!(pf.own_minions.is_empty());
        assert_eq!(pf.own_hand.capacity(), 10);
    }

    #[test]
    fn test_end_turn_resets_state() {
        let mut pf = Playfield::new();
        pf.cards_played_this_turn = 3;
        pf.damage_dealt_to_enemy_hero_this_turn = 10;
        pf.end_turn();
        assert_eq!(pf.cards_played_this_turn, 0);
        assert_eq!(pf.damage_dealt_to_enemy_hero_this_turn, 0);
        assert!(pf.complete);
        assert!(!pf.is_own_turn);
    }

    #[test]
    fn test_summon_minion_max_seven() {
        let mut pf = Playfield::new();
        for i in 0..7 {
            assert!(pf.summon_minion(1000 + i, i as usize, true).is_some());
        }
        assert!(pf.summon_minion(2000, 0, true).is_none());
        assert_eq!(pf.own_minions.len(), 7);
    }

    #[test]
    fn test_damage_kills_minion() {
        let mut pf = Playfield::new();
        pf.summon_minion(1001, 0, true);
        let eid = pf.own_minions[0].entity_id;
        pf.minion_get_damage_or_heal(eid, false, false, 3, false);
        assert!(pf.own_minions.is_empty());
    }

    #[test]
    fn test_divine_shield_blocks_damage() {
        let mut pf = Playfield::new();
        pf.summon_minion(1001, 0, true);
        let eid = pf.own_minions[0].entity_id;
        pf.own_minions[0].divine_shield = true;
        pf.own_minions[0].hp = 5;
        pf.minion_get_damage_or_heal(eid, false, false, 3, false);
        assert!(!pf.own_minions.is_empty());
        assert!(!pf.own_minions[0].divine_shield);
        assert_eq!(pf.own_minions[0].hp, 5);
    }

    #[test]
    fn test_poisonous_instakill() {
        let mut pf = Playfield::new();
        pf.summon_minion(1001, 0, true);
        let eid = pf.own_minions[0].entity_id;
        pf.own_minions[0].hp = 999;
        pf.minion_get_damage_or_heal(eid, false, false, 1, true);
        assert!(pf.own_minions.is_empty());
    }

    #[test]
    fn test_armor_absorbs() {
        let mut pf = Playfield::new();
        pf.own_hero.armor = 5;
        pf.own_hero.hp = 30;
        pf.minion_get_damage_or_heal(pf.own_hero.entity_id, true, false, 7, false);
        assert_eq!(pf.own_hero.armor, 0);
        assert_eq!(pf.own_hero.hp, 28);
    }

    #[test]
    fn test_immune_blocks() {
        let mut pf = Playfield::new();
        pf.own_hero.immune = true;
        pf.own_hero.hp = 10;
        pf.minion_get_damage_or_heal(pf.own_hero.entity_id, true, false, 100, false);
        assert_eq!(pf.own_hero.hp, 10);
    }

    #[test]
    fn test_minion_attacks_hero() {
        let mut pf = Playfield::new();
        pf.enemy_hero.hp = 30;
        pf.enemy_hero.entity_id = 100;
        pf.summon_minion(1, 0, true);
        pf.own_minions[0].entity_id = 1;
        pf.own_minions[0].angr = 5;
        pf.own_minions[0].ready = true;
        pf.own_minions[0].hp = 5;

        let a = Action {
            action_type: ActionType::AttackWithMinion,
            hand_card: None,
            source: Some(pf.own_minions[0].clone()),
            target: Some(pf.enemy_hero.clone()),
            position: 0,
            penality: 0,
            choice: 0,
        };
        pf.do_action(&a, &CardDb::default());
        assert_eq!(pf.enemy_hero.hp, 25);
        assert!(!pf.own_minions[0].ready);
    }

    #[test]
    fn test_attack_with_retaliation() {
        let mut pf = Playfield::new();
        pf.enemy_hero.entity_id = 100;
        pf.summon_minion(1, 0, true);
        pf.own_minions[0].entity_id = 1;
        pf.own_minions[0].angr = 5;
        pf.own_minions[0].ready = true;
        pf.own_minions[0].hp = 5;
        pf.enemy_minions.push(make_minion(10, 3, 3));

        let a = Action {
            action_type: ActionType::AttackWithMinion,
            hand_card: None,
            source: Some(pf.own_minions[0].clone()),
            target: Some(pf.enemy_minions[0].clone()),
            position: 0,
            penality: 0,
            choice: 0,
        };
        pf.do_action(&a, &CardDb::default());
        assert!(pf.enemy_minions.is_empty());
        assert_eq!(pf.own_minions[0].hp, 2);
    }

    #[test]
    fn test_lifesteal_heals() {
        let mut pf = Playfield::new();
        pf.enemy_hero.entity_id = 100;
        pf.own_hero.hp = 20;
        pf.own_hero.max_hp = 30;
        pf.summon_minion(1, 0, true);
        pf.own_minions[0].entity_id = 1;
        pf.own_minions[0].angr = 5;
        pf.own_minions[0].ready = true;
        pf.own_minions[0].hp = 5;
        pf.own_minions[0].lifesteal = true;

        let a = Action {
            action_type: ActionType::AttackWithMinion,
            hand_card: None,
            source: Some(pf.own_minions[0].clone()),
            target: Some(pf.enemy_hero.clone()),
            position: 0,
            penality: 0,
            choice: 0,
        };
        pf.do_action(&a, &CardDb::default());
        assert_eq!(pf.enemy_hero.hp, 25);
        assert_eq!(pf.own_hero.hp, 25);
    }

    #[test]
    fn test_fatigue_damage() {
        let mut pf = Playfield::new();
        pf.own_hero.hp = 30;
        assert!(pf.draw_card(None).is_none());
        assert_eq!(pf.own_hero.hp, 29);
        assert!(pf.draw_card(None).is_none());
        assert_eq!(pf.own_hero.hp, 27);
    }

    #[test]
    fn test_hero_attack_damages_weapon() {
        let mut pf = Playfield::new();
        pf.enemy_hero.hp = 30;
        pf.enemy_hero.entity_id = 100;
        pf.own_hero.ready = true;
        pf.own_weapon = Some(Weapon {
            entity_id: 1,
            card_id: 2001,
            angr: 3,
            durability: 2,
            base_angr: 3,
            base_durability: 2,
            windfury: false,
            poisonous: false,
            lifesteal: false,
            immune: false,
            mega_windfury: false,
        });

        let a = Action {
            action_type: ActionType::AttackWithHero,
            hand_card: None,
            source: Some(pf.own_hero.clone()),
            target: Some(pf.enemy_hero.clone()),
            position: 0,
            penality: 0,
            choice: 0,
        };
        pf.do_action(&a, &CardDb::default());
        assert_eq!(pf.enemy_hero.hp, 27);
        assert_eq!(pf.own_weapon.as_ref().unwrap().durability, 1);
    }

    #[test]
    fn test_enchantment_buffs_stats() {
        let mut pf = Playfield::new();
        pf.summon_minion(1, 0, true);
        pf.own_minions[0].base_angr = 3;
        pf.own_minions[0].base_hp = 4;
        pf.own_minions[0].angr = 3;
        pf.own_minions[0].hp = 4;
        pf.own_minions[0].recalc_stats();

        let ench = Enchantment {
            card_id: 999,
            turns_remaining: -1,
            angr_buff: 2,
            hp_buff: 1,
            is_aura: false,
            is_deathrattle: false,
        };
        pf.own_minions[0].apply_enchantment(ench);
        assert_eq!(pf.own_minions[0].angr, 5); // 3+2
        assert_eq!(pf.own_minions[0].hp, 5); // 4+1
    }

    #[test]
    fn test_silence_removes_enchantments_and_keywords() {
        let mut pf = Playfield::new();
        pf.summon_minion(1, 0, true);
        pf.own_minions[0].base_angr = 3;
        pf.own_minions[0].base_hp = 4;
        pf.own_minions[0].angr = 5; // buffed
        pf.own_minions[0].hp = 6;
        pf.own_minions[0].taunt = true;
        pf.own_minions[0].divine_shield = true;

        pf.own_minions[0].apply_silence();
        assert_eq!(pf.own_minions[0].angr, 3); // back to base
        assert_eq!(pf.own_minions[0].hp, 4);
        assert!(!pf.own_minions[0].taunt);
        assert!(!pf.own_minions[0].divine_shield);
        assert!(pf.own_minions[0].silenced);
    }

    #[test]
    fn test_windfury_double_attack() {
        let mut pf = Playfield::new();
        pf.enemy_hero.hp = 30;
        pf.enemy_hero.entity_id = 100;
        pf.summon_minion(1, 0, true);
        pf.own_minions[0].entity_id = 1;
        pf.own_minions[0].angr = 3;
        pf.own_minions[0].ready = true;
        pf.own_minions[0].windfury = true;
        pf.own_minions[0].hp = 5;

        let a = Action {
            action_type: ActionType::AttackWithMinion,
            hand_card: None,
            source: Some(pf.own_minions[0].clone()),
            target: Some(pf.enemy_hero.clone()),
            position: 0,
            penality: 0,
            choice: 0,
        };
        pf.do_action(&a, &CardDb::default());
        assert_eq!(pf.enemy_hero.hp, 27); // first hit
        assert!(pf.own_minions[0].ready); // windfury: still ready
        assert_eq!(pf.own_minions[0].num_attacks_this_turn, 1);
    }

    #[test]
    fn test_stealth_protects_from_attack() {
        let _pf = Playfield::new();
        // Stealth minions should not be in attack target list
        // Test via MoveGenerator's get_attack_targets
    }

    #[test]
    fn test_auras_apply_temp_buffs() {
        let mut pf = Playfield::new();
        pf.summon_minion(1, 0, true);
        pf.summon_minion(2, 1, true);
        pf.own_minions[0].entity_id = 10;
        pf.own_minions[1].entity_id = 20;

        // Make 1st minion an aura source
        pf.own_minions[0].enchantments.push(Enchantment {
            card_id: 0,
            turns_remaining: -1,
            angr_buff: 0,
            hp_buff: 0,
            is_aura: true,
            is_deathrattle: false,
        });

        pf.update_auras();

        // Aura source doesn't buff itself
        assert_eq!(pf.own_minions[0].temp_angr_buff, 0);
        // Other minion gets +1/+1
        assert_eq!(pf.own_minions[1].temp_angr_buff, 1);
        assert_eq!(pf.own_minions[1].temp_hp_buff, 1);
    }

    #[test]
    fn test_trade_card_consumes_mana() {
        let mut pf = Playfield::new();
        pf.mana = 10;
        pf.own_deck.push(100);
        pf.own_hand.push(HandCard {
            card_id: 99,
            entity_id: 1,
            position: 0,
            cost: 2,
            original_cost: 2,
            attack: 0,
            health: 0,
            card_type: CardType::Spell,
            race: Race::None,
            is_choice: false,
            has_targets: false,
            is_tradeable: true,
            is_forge: false,
        });

        let trade = Action {
            action_type: ActionType::Trade,
            hand_card: Some(pf.own_hand[0].clone()),
            source: None,
            target: None,
            position: 0,
            penality: 0,
            choice: 0,
        };
        pf.do_action(&trade, &CardDb::default());

        assert_eq!(pf.mana, 9); // spent 1 mana
        assert_eq!(pf.own_deck.len(), 1); // put back + drew
    }

    #[test]
    fn test_play_card_minion_into_position() {
        let mut pf = Playfield::new();
        pf.mana = 10;
        pf.own_hand.push(HandCard {
            card_id: 1001,
            entity_id: 1,
            position: 0,
            cost: 3,
            original_cost: 3,
            attack: 4,
            health: 5,
            card_type: CardType::Minion,
            race: Race::None,
            is_choice: false,
            has_targets: false,
            is_tradeable: false,
            is_forge: false,
        });

        let play = Action {
            action_type: ActionType::PlayCard,
            hand_card: Some(pf.own_hand[0].clone()),
            source: None,
            target: None,
            position: 0,
            penality: 0,
            choice: 0,
        };
        pf.do_action(&play, &CardDb::default());

        assert_eq!(pf.mana, 7); // 10 - 3
        assert_eq!(pf.own_minions.len(), 1);
        assert!(pf.own_hand.is_empty());
        assert_eq!(pf.cards_played_this_turn, 1);
    }

    #[test]
    fn test_enemy_minions_have_entity_ids() {
        let mut pf = Playfield::new();
        pf.enemy_minions.push(Minion::new_minion(2001, 3, 4));
        pf.enemy_minions[0].entity_id = 50;
        assert_eq!(pf.enemy_minions[0].entity_id, 50);
        assert!(pf.enemy_minions[0].is_alive());
    }

    #[test]
    fn test_serialize_deserialize() {
        let mut pf = Playfield::new();
        pf.mana = 5;
        pf.own_hero.hp = 25;
        pf.enemy_hero.hp = 20;
        pf.summon_minion(100, 0, true);

        let json = serde_json::to_string(&pf).expect("serialize");
        let restored: Playfield = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(restored.mana, 5);
        assert_eq!(restored.own_hero.hp, 25);
        assert_eq!(restored.enemy_hero.hp, 20);
        assert_eq!(restored.own_minions.len(), 1);
    }
}
