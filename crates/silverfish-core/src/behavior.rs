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
