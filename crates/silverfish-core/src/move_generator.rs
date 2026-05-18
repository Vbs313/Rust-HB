//! 动作生成器
//!
//! 对应 C# 版的 Movegenerator.cs
//! 从当前局面生成所有合法动作，并计算每个动作的惩罚值。
//! 包含目标剪枝（cutAttackList），按威胁排序限制数量。

use crate::action::{Action, ActionType};
use crate::minion::Minion;
use crate::penality_manager::PenalityManager;
use crate::playfield::Playfield;

/// 动作生成器
pub struct MoveGenerator {
    penality_mgr: PenalityManager,
}

impl MoveGenerator {
    pub fn new() -> Self {
        Self {
            penality_mgr: PenalityManager::new(),
        }
    }

    /// 生成当前局面的所有合法动作
    pub fn get_move_list(&self, pf: &Playfield, use_penality: bool) -> Vec<Action> {
        let mut actions = Vec::new();

        if pf.complete || !pf.is_own_turn {
            return actions;
        }

        // 1. 出牌动作
        self.get_play_card_actions(pf, &mut actions);

        // 2. 交易动作（可交易卡牌）
        self.get_trade_actions(pf, &mut actions);

        // 3. 锻造动作
        self.get_forge_actions(pf, &mut actions);

        // 4. 随从攻击动作
        self.get_minion_attack_actions(pf, &mut actions);

        // 5. 英雄攻击动作（有武器时）
        self.get_hero_attack_actions(pf, &mut actions);

        // 6. 英雄技能动作
        self.get_hero_power_actions(pf, &mut actions);

        // 7. 地标动作
        self.get_location_actions(pf, &mut actions);

        // 8. 泰坦技能动作
        self.get_titan_actions(pf, &mut actions);

        // 9. 结束回合
        actions.push(Action {
            action_type: ActionType::EndTurn,
            hand_card: None,
            source: None,
            target: None,
            position: 0,
            penality: 0,
            choice: 0,
        });

        // 应用惩罚值
        if use_penality {
            for action in &mut actions {
                action.penality = self.penality_mgr.evaluate(action, pf);
            }
        }

        actions
    }

    // ============================================================
    // 出牌动作
    // ============================================================

    fn get_play_card_actions(&self, pf: &Playfield, actions: &mut Vec<Action>) {
        for (idx, card) in pf.own_hand.iter().enumerate() {
            // 费用检查
            if card.cost > pf.mana {
                continue;
            }

            // 根据卡牌类型生成不同动作
            match card.card_type {
                crate::CardType::Minion => {
                    // 随从：生成每个可用的场上位置
                    let max_positions = if pf.own_minions.len() >= 7 {
                        0 // 满场不能出随从
                    } else {
                        7 - pf.own_minions.len()
                    };
                    for pos in 0..=max_positions {
                        actions.push(Action {
                            action_type: ActionType::PlayCard,
                            hand_card: Some(card.clone()),
                            source: None,
                            target: None,
                            position: pos as i32,
                            penality: 0,
                            choice: 0,
                        });
                    }
                }
                crate::CardType::Spell => {
                    // 法术：如需要目标，为每个可能的目标生成动作
                    let has_target = card.has_targets;
                    if has_target {
                        // 有目标法术：生成对所有合法目标的动作
                        let targets = self.get_spell_targets(pf);
                        for target in targets {
                            actions.push(Action {
                                action_type: ActionType::PlayCard,
                                hand_card: Some(card.clone()),
                                source: None,
                                target: Some(target),
                                position: idx as i32,
                                penality: 0,
                                choice: 0,
                            });
                        }
                        // 如果发现/探底等无目标也有效的情况
                        // add no-target version too
                    } else {
                        // 无目标法术
                        actions.push(Action {
                            action_type: ActionType::PlayCard,
                            hand_card: Some(card.clone()),
                            source: None,
                            target: None,
                            position: idx as i32,
                            penality: 0,
                            choice: 0,
                        });
                    }
                }
                crate::CardType::Weapon => {
                    // 武器：总是替换当前武器
                    actions.push(Action {
                        action_type: ActionType::PlayCard,
                        hand_card: Some(card.clone()),
                        source: None,
                        target: None,
                        position: idx as i32,
                        penality: 0,
                        choice: 0,
                    });
                }
                crate::CardType::Hero => {
                    // 英雄牌
                    actions.push(Action {
                        action_type: ActionType::PlayCard,
                        hand_card: Some(card.clone()),
                        source: None,
                        target: None,
                        position: idx as i32,
                        penality: 0,
                        choice: 0,
                    });
                }
                crate::CardType::Location => {
                    // 地标
                    actions.push(Action {
                        action_type: ActionType::PlayCard,
                        hand_card: Some(card.clone()),
                        source: None,
                        target: None,
                        position: idx as i32,
                        penality: 0,
                        choice: 0,
                    });
                }
                _ => {}
            }
        }
    }

    /// 获取法术的合法目标列表
    fn get_spell_targets(&self, pf: &Playfield) -> Vec<Minion> {
        let mut targets = Vec::new();

        // 可对己方随从
        for m in &pf.own_minions {
            if m.is_alive() && !m.untouchable {
                targets.push(m.clone());
            }
        }
        // 可对敌方随从
        for m in &pf.enemy_minions {
            if m.is_alive() && !m.untouchable {
                targets.push(m.clone());
            }
        }
        // 可对己方英雄
        if !pf.own_hero.untouchable && pf.own_hero.immune == false {
            targets.push(pf.own_hero.clone());
        }
        // 可对敌方英雄
        if !pf.enemy_hero.untouchable && pf.enemy_hero.immune == false {
            targets.push(pf.enemy_hero.clone());
        }

        targets
    }

    // ============================================================
    // 交易/锻造
    // ============================================================

    fn get_trade_actions(&self, pf: &Playfield, actions: &mut Vec<Action>) {
        for card in &pf.own_hand {
            if card.is_tradeable && self.can_afford(pf, 1) {
                actions.push(Action {
                    action_type: ActionType::Trade,
                    hand_card: Some(card.clone()),
                    source: None,
                    target: None,
                    position: 0,
                    penality: 0,
                    choice: 0,
                });
            }
        }
    }

    fn get_forge_actions(&self, pf: &Playfield, actions: &mut Vec<Action>) {
        for card in &pf.own_hand {
            if card.is_forge && self.can_afford(pf, 2) {
                actions.push(Action {
                    action_type: ActionType::Forge,
                    hand_card: Some(card.clone()),
                    source: None,
                    target: None,
                    position: 0,
                    penality: 0,
                    choice: 0,
                });
            }
        }
    }

    // ============================================================
    // 攻击动作（含目标剪枝）
    // ============================================================

    fn get_minion_attack_actions(&self, pf: &Playfield, actions: &mut Vec<Action>) {
        for minion in &pf.own_minions {
            if !minion.can_attack() {
                continue;
            }

            let targets = self.get_attack_targets(pf, false);
            // 目标剪枝：对大量目标（5+）进行威胁排序，只保留最重要的
            let pruned = self.cut_attack_list(targets, 5);

            for target in pruned {
                actions.push(Action {
                    action_type: ActionType::AttackWithMinion,
                    hand_card: None,
                    source: Some(minion.clone()),
                    target: Some(target),
                    position: 0,
                    penality: 0,
                    choice: 0,
                });
            }
        }
    }

    fn get_hero_attack_actions(&self, pf: &Playfield, actions: &mut Vec<Action>) {
        if let Some(ref weapon) = pf.own_weapon {
            if weapon.is_equipped() && pf.own_hero.can_attack() {
                let targets = self.get_attack_targets(pf, true);
                let pruned = self.cut_attack_list(targets, 5);

                for target in pruned {
                    actions.push(Action {
                        action_type: ActionType::AttackWithHero,
                        hand_card: None,
                        source: Some(pf.own_hero.clone()),
                        target: Some(target),
                        position: 0,
                        penality: 0,
                        choice: 0,
                    });
                }
            }
        }
    }

    /// 目标剪枝：按威胁值排序，只保留最重要的 N 个目标
    fn cut_attack_list(&self, mut targets: Vec<Minion>, max_count: usize) -> Vec<Minion> {
        if targets.len() <= max_count {
            return targets;
        }

        // 按威胁值排序（敌方英雄最优先，然后按攻击力+关键词）
        targets.sort_by(|a, b| {
            let ta = self.threat_value(a);
            let tb = self.threat_value(b);
            tb.cmp(&ta) // 降序
        });

        targets.truncate(max_count);
        targets
    }

    /// 计算目标的威胁值（用于剪枝排序）
    fn threat_value(&self, m: &Minion) -> i32 {
        let mut val = 0;

        if m.is_hero {
            // 敌方英雄始终是最高优先级
            val += 10000;
        }

        // 攻击力威胁
        val += m.effective_angr() * 3;

        // 关键词威胁
        if m.taunt {
            val += 50;
        }
        if m.divine_shield {
            val += 30;
        }
        if m.windfury {
            val += 40;
        }
        if m.poisonous {
            val += 80;
        } // 剧毒极高威胁
        if m.lifesteal {
            val += 25;
        }
        if m.charge {
            val += 20;
        } // 潜在的爆发
        if m.mega_windfury {
            val += 70;
        }

        val
    }

    // ============================================================
    // 英雄技能
    // ============================================================

    fn get_hero_power_actions(&self, pf: &Playfield, actions: &mut Vec<Action>) {
        // 英雄技能通常消耗 2 费
        if !self.can_afford(pf, 2) {
            return;
        }
        if pf.own_hero_power.is_none() {
            return;
        }

        // 根据不同的英雄技能生成不同目标
        // 默认英雄技能：加 2 甲（战士/德鲁伊），生成 1 个无目标动作
        actions.push(Action {
            action_type: ActionType::UseHeroPower,
            hand_card: None,
            source: None,
            target: None,
            position: 0,
            penality: 0,
            choice: 0,
        });
    }

    // ============================================================
    // 地标 & 泰坦
    // ============================================================

    fn get_location_actions(&self, pf: &Playfield, actions: &mut Vec<Action>) {
        // 地标：场上具有 charge 标记的特殊随从（简化：地标用 charge 标记可用次数）
        for minion in &pf.own_minions {
            if minion.titan {
                continue;
            }
            if minion.is_alive() && minion.charge {
                actions.push(Action {
                    action_type: ActionType::UseLocation,
                    hand_card: None,
                    source: Some(minion.clone()),
                    target: None,
                    position: 0,
                    penality: 0,
                    choice: 0,
                });
            }
        }
    }

    fn get_titan_actions(&self, pf: &Playfield, actions: &mut Vec<Action>) {
        for minion in &pf.own_minions {
            if minion.titan && minion.is_alive() && minion.num_attacks_this_turn < 3 {
                for ability_no in 0..3 {
                    actions.push(Action {
                        action_type: ActionType::UseTitanAbility,
                        hand_card: None,
                        source: Some(minion.clone()),
                        target: None,
                        position: 0,
                        penality: 0,
                        choice: ability_no,
                    });
                }
            }
        }
    }

    // ============================================================
    // 攻击目标获取
    // ============================================================

    /// 获取可攻击的目标列表（含嘲讽检查）
    fn get_attack_targets(&self, pf: &Playfield, _is_hero_attack: bool) -> Vec<Minion> {
        let mut targets = Vec::new();

        let has_taunt = pf.enemy_minions.iter().any(|m| m.taunt && m.is_alive());

        if has_taunt {
            for minion in &pf.enemy_minions {
                if minion.taunt && minion.is_alive() && !minion.untouchable {
                    targets.push(minion.clone());
                }
            }
        } else {
            for minion in &pf.enemy_minions {
                if minion.is_alive() && !minion.untouchable && !minion.stealth {
                    targets.push(minion.clone());
                }
            }
            // 英雄总是可攻击的（除非有潜行免疫等）
            if !pf.enemy_hero.untouchable && pf.enemy_hero.immune == false {
                targets.push(pf.enemy_hero.clone());
            }
        }

        targets
    }

    // ============================================================
    // 辅助方法
    // ============================================================

    fn can_afford(&self, pf: &Playfield, cost: i32) -> bool {
        pf.mana >= cost
    }
}
