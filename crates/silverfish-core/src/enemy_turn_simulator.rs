//! 敌方回合模拟器
//!
//! 对应 C# 版的 EnemyTurnSimulator.cs
//! 模拟敌方在其回合内的最佳操作，用于评估局面价值。

use crate::ai::AiConfig;
use crate::playfield::Playfield;

/// 敌方回合模拟器
pub struct EnemyTurnSimulator;

impl EnemyTurnSimulator {
    /// 模拟敌方回合，返回敌方造成的价值损失
    ///
    /// 敌方回合模拟两步：
    /// 1. 敌方最佳清场（消灭我方最有威胁的随从）
    /// 2. 敌方最佳抢脸（对我方英雄造成直伤）
    pub fn simulate(&self, my_state: &Playfield, config: &AiConfig) -> f32 {
        let _ = config;
        self.estimate_enemy_threat(my_state)
    }

    /// 估算敌方当前场面的威胁值
    fn estimate_enemy_threat(&self, pf: &Playfield) -> f32 {
        let mut threat = 0.0;

        // 敌方随从的总攻击力威胁
        for minion in &pf.enemy_minions {
            if !minion.is_alive() {
                continue;
            }

            let mut m_threat = minion.effective_angr() as f32;

            // 关键词附加值
            if minion.charge {
                m_threat *= 1.5;
            } // 冲锋：即刻威胁
            if minion.windfury {
                m_threat *= 1.3;
            } // 风怒：双倍攻击
            if minion.poisonous {
                m_threat += 5.0;
            } // 剧毒：能换任何随从
            if minion.lifesteal {
                m_threat += 3.0;
            } // 吸血：还能回血
            if minion.rush {
                m_threat *= 1.2;
            } // 突袭：当回合可攻击

            threat += m_threat;
        }

        // 敌方武器威胁
        if let Some(ref w) = pf.enemy_weapon {
            if w.is_equipped() {
                threat += w.angr as f32 * 1.5;
            }
        }

        // 敌方英雄技能威胁（默认值）
        if pf.enemy_hero_power.is_some() {
            threat += 3.0; // 大多数英雄技能约值 3 点伤害
        }

        threat
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::AiConfig;

    fn empty_pf() -> Playfield {
        Playfield::new()
    }

    #[test]
    fn test_no_enemy_threat() {
        let simulator = EnemyTurnSimulator;
        let config = AiConfig::default();
        let pf = empty_pf();
        let threat = simulator.simulate(&pf, &config);
        assert_eq!(threat, 0.0, "Empty board should have 0 threat");
    }

    #[test]
    fn test_enemy_minion_threat() {
        let simulator = EnemyTurnSimulator;
        let config = AiConfig::default();
        let mut pf = empty_pf();

        pf.enemy_minions.push(crate::minion::Minion::new_minion(1, 3, 3));
        pf.enemy_minions[0].entity_id = 100;

        let threat = simulator.simulate(&pf, &config);
        assert!(threat > 0.0, "Enemy minion should contribute threat");
        assert!(threat < 20.0, "A 3/3 shouldn't be that threatening");
    }

    #[test]
    fn test_charge_minion_higher_threat() {
        let simulator = EnemyTurnSimulator;
        let config = AiConfig::default();
        let mut pf_normal = empty_pf();
        let mut pf_charge = empty_pf();

        pf_normal.enemy_minions.push(crate::minion::Minion::new_minion(1, 3, 3));
        pf_normal.enemy_minions[0].entity_id = 100;

        pf_charge.enemy_minions.push(crate::minion::Minion::new_minion(1, 3, 3));
        pf_charge.enemy_minions[0].entity_id = 101;
        pf_charge.enemy_minions[0].charge = true;

        let normal = simulator.simulate(&pf_normal, &config);
        let charge = simulator.simulate(&pf_charge, &config);
        assert!(charge > normal, "Charge minion should be more threatening");
    }

    #[test]
    fn test_weapon_threat() {
        let simulator = EnemyTurnSimulator;
        let config = AiConfig::default();
        let mut pf = empty_pf();

        pf.enemy_weapon = Some(crate::weapon::Weapon {
            angr: 5,
            durability: 2,
            ..Default::default()
        });

        let threat = simulator.simulate(&pf, &config);
        assert!(threat > 0.0, "Enemy weapon should contribute threat");
    }

    #[test]
    fn test_poisonous_additional_threat() {
        let simulator = EnemyTurnSimulator;
        let config = AiConfig::default();
        let mut pf = empty_pf();

        pf.enemy_minions.push(crate::minion::Minion::new_minion(1, 1, 1));
        pf.enemy_minions[0].entity_id = 100;
        pf.enemy_minions[0].poisonous = true;

        let threat = simulator.simulate(&pf, &config);
        // 1/1 基础威胁 = 1，剧毒 +5，总计约 6
        assert!(threat > 5.0, "Poisonous minion threat should include poison bonus");
    }
}
