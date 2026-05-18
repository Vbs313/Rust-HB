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
