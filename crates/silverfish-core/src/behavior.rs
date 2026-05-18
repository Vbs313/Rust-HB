//! 策略评估系统
//!
//! 对应 C# 版的 Behavior.cs
//! 定义局面价值的评估函数，不同策略有不同的评估侧重。

use crate::minion::Minion;
use crate::playfield::Playfield;

/// 策略评估 trait
pub trait Behavior: Send + Sync {
    /// 评估整个局面的价值（正值对我方有利）
    fn evaluate(&self, pf: &Playfield) -> f32;

    /// 策略名称
    fn name(&self) -> &'static str;
}

/// 默认策略（均衡型）
pub mod default_behavior {
    use super::*;

    pub struct DefaultBehavior;

    impl Behavior for DefaultBehavior {
        fn evaluate(&self, pf: &Playfield) -> f32 {
            let mut value = 0.0;

            // 斩杀检测
            if pf.enemy_hero.hp <= 0 && pf.enemy_hero.armor <= 0 {
                value += 10000.0;
            }
            if pf.own_hero.hp <= 0 && pf.own_hero.armor <= 0 {
                value -= 20000.0;
            }

            // 惩罚值扣减
            value -= pf.evaluate_penality as f32;

            // 法力水晶优势
            value += (pf.own_max_mana - pf.enemy_max_mana) as f32 * 20.0;

            // 武器价值
            if let Some(ref w) = pf.own_weapon {
                value += (w.angr * w.durability * 2) as f32;
            }
            if let Some(ref w) = pf.enemy_weapon {
                value -= (w.durability * w.angr) as f32;
            }

            // 随从价值
            for minion in &pf.own_minions {
                value += evaluate_own_minion(minion);
            }
            for minion in &pf.enemy_minions {
                value -= evaluate_enemy_minion(minion);
            }

            // 英雄血量
            value += ((pf.own_hero.hp + pf.own_hero.armor) * 2) as f32;
            value -= ((pf.enemy_hero.hp + pf.enemy_hero.armor) * 2) as f32;

            // 手牌数量（手牌优势）
            value += pf.own_hand.len() as f32 * 5.0;
            value -= pf.enemy_hand_count as f32 * 5.0;

            value
        }

        fn name(&self) -> &'static str {
            "Default (Balanced)"
        }
    }

    /// 评估我方单个随从的价值
    fn evaluate_own_minion(m: &Minion) -> f32 {
        let mut val = 0.0;

        // 基础价值：攻击 + 生命
        val += (m.effective_angr() * 2 + m.effective_hp() * 2) as f32;

        // 关键词附加值
        if m.taunt {
            val += 5.0;
        }
        if m.divine_shield {
            val += 8.0;
        }
        if m.charge {
            val += 6.0;
        }
        if m.windfury {
            val += 4.0;
        }
        if m.lifesteal {
            val += 4.0;
        }
        if m.poisonous {
            val += 5.0;
        }
        if m.rush {
            val += 2.0;
        }
        if m.reborn {
            val += 4.0;
        }
        if m.stealth {
            val += 2.0;
        }

        val
    }

    /// 评估敌方单个随从的威胁
    fn evaluate_enemy_minion(m: &Minion) -> f32 {
        // 基础威胁值
        let mut val = (m.effective_angr() * 3 + m.effective_hp() * 2) as f32;

        if m.taunt {
            val += 8.0;
        }
        if m.divine_shield {
            val += 6.0;
        }
        if m.windfury {
            val += 6.0;
        }
        if m.lifesteal {
            val += 3.0;
        }
        if m.poisonous {
            val += 8.0;
        }
        if m.charge {
            val += 4.0;
        }

        val
    }
}

/// 快攻策略
pub mod rush_behavior {
    use super::*;

    pub struct RushBehavior;

    impl Behavior for RushBehavior {
        fn evaluate(&self, pf: &Playfield) -> f32 {
            let mut value = default_behavior::DefaultBehavior.evaluate(pf);

            // 快攻倾向：更重视打脸和场面
            // 敌方英雄血量越低越好
            value += (30.0 - (pf.enemy_hero.hp + pf.enemy_hero.armor) as f32) * 3.0;

            // 更重视攻击力
            for minion in &pf.own_minions {
                value += minion.effective_angr() as f32 * 1.5;
            }

            value
        }

        fn name(&self) -> &'static str {
            "Rush (Aggro)"
        }
    }
}

/// 控制策略
pub mod control_behavior {
    use super::*;

    pub struct ControlBehavior;

    impl Behavior for ControlBehavior {
        fn evaluate(&self, pf: &Playfield) -> f32 {
            let mut value = default_behavior::DefaultBehavior.evaluate(pf);

            // 控制倾向：更重视清场和后期
            // 手牌优势更重要
            value += pf.own_hand.len() as f32 * 8.0;

            // 生命值更重要
            value += (pf.own_hero.hp + pf.own_hero.armor) as f32 * 3.0;

            value
        }

        fn name(&self) -> &'static str {
            "Control"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_board() -> Playfield {
        Playfield::new()
    }

    fn board_with_our_minion() -> Playfield {
        let mut pf = Playfield::new();
        pf.mana = 5;
        pf.is_own_turn = true;
        pf.own_hero.hp = 30;
        pf.enemy_hero.hp = 25;
        pf.own_hero.entity_id = 1;
        pf.enemy_hero.entity_id = 2;
        pf.summon_minion(1001, 0, true);
        pf.own_minions[0].angr = 3;
        pf.own_minions[0].hp = 3;
        pf.own_minions[0].ready = true;
        pf
    }

    #[test]
    fn test_default_behavior_empty() {
        let pf = empty_board();
        let v = default_behavior::DefaultBehavior.evaluate(&pf);
        // 空局面：双方英雄 30/30，无手牌
        // 30*2 - 30*2 + 0 = 0
        assert_eq!(v, 0.0);
    }

    #[test]
    fn test_default_behavior_our_minion_advantage() {
        let pf = board_with_our_minion();
        let v = default_behavior::DefaultBehavior.evaluate(&pf);
        // 我方有 3/3，敌方无随从 = 正数
        assert!(v > 0.0);
    }

    #[test]
    fn test_default_behavior_lethal() {
        let mut pf = board_with_our_minion();
        pf.enemy_hero.hp = 0;
        pf.enemy_hero.armor = 0;
        let v = default_behavior::DefaultBehavior.evaluate(&pf);
        assert!(v > 5000.0, "Lethal should give very high score");
    }

    #[test]
    fn test_default_behavior_enemy_lethal_us() {
        let mut pf = board_with_our_minion();
        pf.own_hero.hp = 0;
        let v = default_behavior::DefaultBehavior.evaluate(&pf);
        assert!(v < -10000.0, "We're dead, should be very negative");
    }

    #[test]
    fn test_rush_behavior() {
        let pf = board_with_our_minion();
        let rush_v = rush_behavior::RushBehavior.evaluate(&pf);
        let default_v = default_behavior::DefaultBehavior.evaluate(&pf);
        assert!(rush_v > default_v, "Rush should value aggro board higher");
    }

    #[test]
    fn test_control_behavior() {
        let mut pf = board_with_our_minion();
        pf.own_hand.push(crate::playfield::HandCard {
            card_id: 213,
            entity_id: 5,
            position: 0,
            cost: 4,
            original_cost: 4,
            attack: 0,
            health: 0,
            card_type: crate::CardType::Spell,
            race: crate::Race::None,
            is_choice: false,
            has_targets: false,
            is_tradeable: false,
            is_forge: false,
        });
        pf.own_hand.push(crate::playfield::HandCard {
            card_id: 602,
            entity_id: 6,
            position: 1,
            cost: 2,
            original_cost: 2,
            attack: 1,
            health: 3,
            card_type: crate::CardType::Minion,
            race: crate::Race::None,
            is_choice: false,
            has_targets: false,
            is_tradeable: false,
            is_forge: false,
        });

        let ctrl_v = control_behavior::ControlBehavior.evaluate(&pf);
        let default_v = default_behavior::DefaultBehavior.evaluate(&pf);
        // Control 对手牌加权更高 = 总体更高
        let diff = ctrl_v - default_v;
        assert!(diff > 0.0, "Control values hand cards more, diff = {diff}");
    }

    #[test]
    fn test_behavior_names() {
        assert_eq!(
            default_behavior::DefaultBehavior.name(),
            "Default (Balanced)"
        );
        assert_eq!(rush_behavior::RushBehavior.name(), "Rush (Aggro)");
        assert_eq!(control_behavior::ControlBehavior.name(), "Control");
    }

    #[test]
    fn test_taunt_bonus() {
        let mut pf = board_with_our_minion();
        pf.own_minions[0].taunt = true;
        let with_taunt = default_behavior::DefaultBehavior.evaluate(&pf);
        pf.own_minions[0].taunt = false;
        let without_taunt = default_behavior::DefaultBehavior.evaluate(&pf);
        assert!(with_taunt > without_taunt, "Taunt should add value");
    }

    #[test]
    fn test_divine_shield_bonus() {
        let mut pf = board_with_our_minion();
        pf.own_minions[0].divine_shield = true;
        let with_ds = default_behavior::DefaultBehavior.evaluate(&pf);
        pf.own_minions[0].divine_shield = false;
        let without_ds = default_behavior::DefaultBehavior.evaluate(&pf);
        assert!(with_ds > without_ds, "Divine shield should add value");
    }
}
