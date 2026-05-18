//! 弹窗处理器（策略模式）
//!
//! 替代 C# 版的 Triton.DialogHandlers 策略模式实现。
//! 每个弹窗类型对应一个处理器，统一通过 DialogHandler 接口操作。

use crate::scene_detector::DialogType;
use crate::GameError;
use hb_input_sim::InputSimulator;
use hb_mono_bridge::MonoBridge;

/// 弹窗处理器 trait
pub trait DialogHandler: Send + Sync {
    /// 弹窗类型
    fn dialog_type(&self) -> DialogType;
    /// 检测弹窗是否存在
    fn is_active(&self, mono: &MonoBridge) -> Result<bool, GameError>;
    /// 处理弹窗（关闭/确认/选择）
    fn handle(&self, mono: &MonoBridge, input: &InputSimulator) -> Result<(), GameError>;
    /// 优先级（数字越小越优先）
    fn priority(&self) -> u32 {
        100
    }
}

/// 弹窗处理器管理
pub struct DialogHandlerManager {
    handlers: Vec<Box<dyn DialogHandler>>,
}

impl DialogHandlerManager {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    /// 注册处理器
    pub fn register(&mut self, handler: Box<dyn DialogHandler>) {
        self.handlers.push(handler);
    }

    /// 检测并处理当前弹窗
    pub fn detect_and_handle(
        &self,
        mono: &MonoBridge,
        input: &InputSimulator,
    ) -> Result<bool, GameError> {
        // 按优先级排序
        let mut sorted: Vec<_> = self.handlers.iter().collect();
        sorted.sort_by_key(|h| h.priority());

        for handler in sorted {
            if handler.is_active(mono)? {
                tracing::info!("Handling dialog: {:?}", handler.dialog_type());
                handler.handle(mono, input)?;
                return Ok(true);
            }
        }

        Ok(false)
    }
}

// ===== 具体处理器实现 =====

/// OK 弹窗处理器（最简单的确认弹窗）
pub struct OkDialogHandler;

impl DialogHandler for OkDialogHandler {
    fn dialog_type(&self) -> DialogType {
        DialogType::OkDialog
    }

    fn is_active(&self, _mono: &MonoBridge) -> Result<bool, GameError> {
        // TODO: 检测 OK 按钮的文本/位置
        Ok(false)
    }

    fn handle(&self, _mono: &MonoBridge, input: &InputSimulator) -> Result<(), GameError> {
        // 点击 OK 按钮
        input.click_left().map_err(GameError::Input)
    }

    fn priority(&self) -> u32 {
        0
    }
}

/// 卡牌选择弹窗处理器
pub struct DeckPickerHandler;

impl DialogHandler for DeckPickerHandler {
    fn dialog_type(&self) -> DialogType {
        DialogType::DeckPicker
    }

    fn is_active(&self, _mono: &MonoBridge) -> Result<bool, GameError> {
        // TODO: 检测选牌界面
        Ok(false)
    }

    fn handle(&self, _mono: &MonoBridge, _input: &InputSimulator) -> Result<(), GameError> {
        // 选择预定卡组
        Ok(())
    }
}

/// 奖励弹窗处理器
pub struct RewardDialogHandler;

impl DialogHandler for RewardDialogHandler {
    fn dialog_type(&self) -> DialogType {
        DialogType::Reward
    }

    fn is_active(&self, _mono: &MonoBridge) -> Result<bool, GameError> {
        Ok(false)
    }

    fn handle(&self, _mono: &MonoBridge, input: &InputSimulator) -> Result<(), GameError> {
        // 点击"确认"或"跳过"
        input.click_left().map_err(GameError::Input)
    }
}
