//! 惩罚值管理器
//!
//! 对应 C# 版的 PenalityManager.cs
//! 为特定动作分配惩罚值，引导 AI 避免或偏好某些行为。
//! - penalty >= 500: 禁止执行
//! - penalty > 0: 不推荐（越大越不推荐）
//! - penalty == 0: 中性
//! - penalty < 0: 推荐（越负越推荐）

use crate::action::{Action, ActionType};
use crate::playfield::Playfield;

/// 惩罚值管理器
pub struct PenalityManager;

impl PenalityManager {
    pub fn new() -> Self {
        Self
    }

    /// 评估动作的惩罚值
    pub fn evaluate(&self, action: &Action, _pf: &Playfield) -> i32 {
        match action.action_type {
            ActionType::PlayCard => self.eval_play_card(action),
            ActionType::AttackWithMinion => self.eval_attack_minion(action),
            ActionType::AttackWithHero => self.eval_attack_hero(action),
            ActionType::UseHeroPower => 0,
            ActionType::Trade => -10, // 交易通常值得做
            ActionType::EndTurn => 0,
            _ => 0,
        }
    }

    /// 出牌惩罚
    fn eval_play_card(&self, action: &Action) -> i32 {
        let card = match &action.hand_card {
            Some(c) => c,
            None => return 500, // 没有手牌信息 = 不可执行
        };

        let mut penalty = 0;

        // 费用效率惩罚：高费卡在低费回合出不太划算
        if card.cost > 5 && card.original_cost > 5 {
            penalty += 5; // 高费卡稍不推荐
        }

        // 手牌位置惩罚：靠右的卡可能优先级较低
        // (保留给 future 改进)

        penalty
    }

    /// 随从攻击惩罚
    fn eval_attack_minion(&self, action: &Action) -> i32 {
        let (attacker, target) = match (&action.source, &action.target) {
            (Some(a), Some(t)) => (a, t),
            _ => return 500,
        };

        let mut penalty = 0;

        // 攻击比自己攻击力高的目标 = 不推荐（可能被换掉）
        if target.effective_angr() > attacker.effective_angr()
            && target.effective_hp() > attacker.effective_angr()
        {
            penalty += 30; // 换不过
        }

        // 攻击有圣盾的目标 = 浪费伤害
        if target.divine_shield {
            penalty += 20;
        }

        // 攻击有剧毒的目标 = 非常不推荐（会被秒）
        if target.poisonous {
            penalty += 100;
        }

        // 攻击可攻击的敌方英雄 = 推荐（打脸）
        if target.is_hero {
            penalty -= 15;
        }

        // 攻击有嘲讽的目标 = 略微推荐（必须过墙）
        if target.taunt {
            penalty -= 10;
        }

        penalty
    }

    /// 英雄攻击惩罚
    fn eval_attack_hero(&self, action: &Action) -> i32 {
        let target = match &action.target {
            Some(t) => t,
            None => return 500,
        };

        let mut penalty = 0;

        // 攻击敌方英雄 = 推荐
        if target.is_hero {
            penalty -= 20;
        }

        // 攻击有剧毒的目标 = 不推荐
        if target.poisonous {
            penalty += 200; // 英雄被毒 = 基本输了
        }

        // 攻击有圣盾 = 浪费
        if target.divine_shield {
            penalty += 30;
        }

        penalty
    }
}
