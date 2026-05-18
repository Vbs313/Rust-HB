//! AI 主控制器
//!
//! 对应 C# 版的 Ai.cs
//! 管理搜索参数、线程池、模拟器实例

use crate::action::Action;
use crate::behavior::default_behavior::DefaultBehavior;
use crate::behavior::Behavior;
use crate::mini_simulator::MiniSimulator;
use crate::move_generator::MoveGenerator;
use crate::playfield::Playfield;

/// AI 搜索参数
#[derive(Debug, Clone)]
pub struct AiConfig {
    /// 最大搜索深度
    pub max_depth: u32,
    /// 每步最大考虑局面数
    pub max_wide: u32,
    /// 每步保留局面数
    pub max_cal: u32,
    /// 下回合模拟开关
    pub simulate_next_turn: bool,
    /// 下回合深度
    pub next_turn_depth: u32,
    /// 下回合宽度
    pub next_turn_wide: u32,
    /// 敌方模拟宽度
    pub enemy_turn_wide: u32,
    /// 敌方模拟第二步宽度
    pub enemy_turn_wide_second: u32,
    /// 是否使用斩杀检查
    pub use_lethal_check: bool,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            max_depth: 12,
            max_wide: 3000,
            max_cal: 60,
            simulate_next_turn: true,
            next_turn_depth: 6,
            next_turn_wide: 20,
            enemy_turn_wide: 40,
            enemy_turn_wide_second: 200,
            use_lethal_check: false,
        }
    }
}

/// AI 控制器（单例）
pub struct Ai {
    pub config: AiConfig,
    move_generator: MoveGenerator,
    simulator: MiniSimulator,
    behavior: Box<dyn Behavior>,
}

impl Default for Ai {
    fn default() -> Self {
        Self::new()
    }
}

impl Ai {
    pub fn new() -> Self {
        Self {
            config: AiConfig::default(),
            move_generator: MoveGenerator::new(),
            simulator: MiniSimulator::new(),
            behavior: Box::new(DefaultBehavior),
        }
    }

    /// 设置策略
    pub fn set_behavior(&mut self, behavior: Box<dyn Behavior>) {
        self.behavior = behavior;
    }

    /// 对给定的局面进行完整搜索，返回最优动作
    pub fn do_all(&self, root: &Playfield) -> Option<Action> {
        tracing::info!(
            "AI search start: depth={}, wide={}, cal={}",
            self.config.max_depth,
            self.config.max_wide,
            self.config.max_cal
        );

        let best = self.simulator.search(
            root,
            &self.config,
            &self.move_generator,
            self.behavior.as_ref(),
        );

        if let Some(action) = &best {
            tracing::info!("AI search complete: best action = {:?}", action.action_type);
        } else {
            tracing::warn!("AI search complete: no valid action found");
        }

        best
    }

    /// 快速搜索（单步）
    pub fn do_something(&self, root: &Playfield) -> Option<Action> {
        let mut quick_config = self.config.clone();
        quick_config.max_depth = 3;
        quick_config.max_wide = 100;
        quick_config.simulate_next_turn = false;

        self.simulator.search(
            root,
            &quick_config,
            &self.move_generator,
            self.behavior.as_ref(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::ActionType;

    #[test]
    fn test_ai_config_default() {
        let config = AiConfig::default();
        assert_eq!(config.max_depth, 12);
        assert_eq!(config.max_wide, 3000);
        assert_eq!(config.max_cal, 60);
        assert!(config.simulate_next_turn);
        assert!(!config.use_lethal_check);
    }

    #[test]
    fn test_ai_new() {
        let ai = Ai::new();
        assert_eq!(ai.config.max_depth, 12);
        assert_eq!(ai.behavior.name(), "Default (Balanced)");
    }

    #[test]
    fn test_ai_set_behavior() {
        let mut ai = Ai::new();
        ai.set_behavior(Box::new(crate::behavior::rush_behavior::RushBehavior));
        assert_eq!(ai.behavior.name(), "Rush (Aggro)");
    }

    #[test]
    fn test_ai_do_something_with_empty_board() {
        let ai = Ai::new();
        let pf = Playfield::new();
        let result = ai.do_something(&pf);
        // 空局面应该至少给出 EndTurn
        assert!(result.is_some());
        assert_eq!(result.unwrap().action_type, ActionType::EndTurn);
    }

    #[test]
    fn test_ai_config_clone() {
        let c1 = AiConfig {
            max_depth: 6,
            max_wide: 500,
            max_cal: 30,
            simulate_next_turn: false,
            next_turn_depth: 2,
            next_turn_wide: 10,
            enemy_turn_wide: 20,
            enemy_turn_wide_second: 50,
            use_lethal_check: true,
        };
        let c2 = c1.clone();
        assert_eq!(c1.max_depth, c2.max_depth);
        assert_eq!(c1.max_wide, c2.max_wide);
        assert_eq!(c1.use_lethal_check, c2.use_lethal_check);
    }
}
