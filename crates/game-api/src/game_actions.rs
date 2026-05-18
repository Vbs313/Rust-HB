//! 游戏操作
//!
//! 对应 C# 版 TritonHs 中的 PlayCard / AttackWithMinion / UseHeroPower 等。

use crate::GameError;
use hb_input_sim::InputSimulator;

/// 游戏操作接口
pub struct GameActions<'a> {
    input: &'a InputSimulator,
}

/// 出牌操作参数
pub struct PlayCardParams {
    /// 手牌位置（0-based）
    pub hand_index: u32,
    /// 目标实体 ID（0 = 无目标）
    pub target_id: i32,
    /// 场上位置（随从）
    pub position: i32,
    /// 抉择选项
    pub choice: i32,
}

/// 攻击操作参数
pub struct AttackParams {
    /// 攻击者实体 ID
    pub attacker_id: i32,
    /// 目标实体 ID
    pub target_id: i32,
}

impl<'a> GameActions<'a> {
    pub fn new(input: &'a InputSimulator) -> Self {
        Self { input }
    }

    /// 打出卡牌
    pub fn play_card(&self, _params: PlayCardParams) -> Result<(), GameError> {
        // 1. 点击手牌
        // 2. 如有目标，点击目标
        // 3. 如有抉择，点击选项
        todo!("Card play coordinate calculation requires runtime calibration")
    }

    /// 随从攻击
    pub fn attack(&self, _params: AttackParams) -> Result<(), GameError> {
        // 1. 点击攻击者
        // 2. 点击目标
        todo!("Attack requires entity position mapping")
    }

    /// 使用英雄技能
    pub fn use_hero_power(&self, _target_id: i32) -> Result<(), GameError> {
        todo!("Hero power needs coordinate calibration")
    }

    /// 结束回合
    pub fn end_turn(&self) -> Result<(), GameError> {
        // 点击"结束回合"按钮
        let _ = self.input;
        Ok(())
    }
}
