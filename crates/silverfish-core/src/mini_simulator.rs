//! 迷你模拟器（DFS 搜索 + 剪枝）
//!
//! 对应 C# 版的 MiniSimulator.cs
//! 实现深度优先搜索 + 宽深剪枝 + 敌方回合模拟。

use crate::action::{Action, ActionType};
use crate::ai::AiConfig;
use crate::behavior::Behavior;
use crate::move_generator::MoveGenerator;
use crate::playfield::Playfield;

const NEXT_TURN_MAX_WIDE: usize = 20;
const NEXT_TURN_TOTAL_BOARDS: u32 = 200;

/// DFS 搜索模拟器
pub struct MiniSimulator;

impl Default for MiniSimulator {
    fn default() -> Self {
        Self::new()
    }
}

impl MiniSimulator {
    pub fn new() -> Self {
        Self
    }

    /// 执行搜索
    pub fn search(
        &self,
        root: &Playfield,
        config: &AiConfig,
        move_gen: &MoveGenerator,
        behavior: &dyn Behavior,
    ) -> Option<Action> {
        let initial_moves = move_gen.get_move_list(root, true);

        // 过滤掉惩罚值 >= 500 的禁止动作
        let valid_moves: Vec<&Action> = initial_moves.iter().filter(|a| a.penality < 500).collect();

        if valid_moves.is_empty() {
            return Some(Action {
                action_type: ActionType::EndTurn,
                hand_card: None,
                source: None,
                target: None,
                position: 0,
                penality: 0,
                choice: 0,
            });
        }

        // 如果只有一个合法动作（且不是 endturn），直接返回
        if valid_moves.len() == 1 && valid_moves[0].action_type != ActionType::EndTurn {
            return Some(valid_moves[0].clone());
        }

        let mut best_action: Option<Action> = None;
        let mut best_value = f32::NEG_INFINITY;
        let mut calculated: u32 = 0;

        for action in valid_moves {
            if action.action_type == ActionType::EndTurn {
                // 评估直接结束回合
                let pf_after_end = root.deep_clone();
                // 简化：不深入搜索 endturn 后的变化
                let value = behavior.evaluate(&pf_after_end);
                if value > best_value {
                    best_value = value;
                    best_action = Some(action.clone());
                }
                continue;
            }

            let mut child = root.deep_clone();
            // 使用默认 CardDb（空，实际场景需要真实数据）
            let db = crate::card_db::CardDb::default();
            child.do_action(action, &db);

            // 模拟敌方回合
            let enemy_value = self.simulate_enemy_turn(&child, config, move_gen);
            child.value = enemy_value;

            // Minimax 递归
            let value = self.minimax(&child, 1, config, move_gen, behavior, &mut calculated);

            if value > best_value {
                best_value = value;
                best_action = Some(action.clone());
            }
        }

        best_action
    }

    /// Minimax 递归 + 剪枝
    fn minimax(
        &self,
        pf: &Playfield,
        depth: u32,
        config: &AiConfig,
        move_gen: &MoveGenerator,
        behavior: &dyn Behavior,
        calculated: &mut u32,
    ) -> f32 {
        // 剪枝条件
        *calculated += 1;

        // 达到最大深度 → 评估局面
        if depth >= config.max_depth || pf.complete {
            return behavior.evaluate(pf);
        }

        // 计算量限制
        if *calculated >= config.max_cal * config.max_depth {
            return behavior.evaluate(pf);
        }

        // 获取动作列表并剪枝
        let moves = move_gen.get_move_list(pf, true);
        let valid: Vec<&Action> = moves.iter().filter(|a| a.penality < 500).collect();

        if valid.is_empty() || (valid.len() == 1 && valid[0].action_type == ActionType::EndTurn) {
            return behavior.evaluate(pf);
        }

        // 宽剪枝：只保留前 N 个动作
        let max_wide = config.max_wide as usize;
        let candidates = if valid.len() > max_wide {
            // 按惩罚值排序，保留最好的 max_wide 个
            let mut sorted: Vec<&&Action> = valid
                .iter()
                .filter(|a| a.action_type != ActionType::EndTurn)
                .collect();
            sorted.sort_by(|a, b| a.penality.cmp(&b.penality));

            // 如果所有非 endturn 动作都被剪掉了，就考虑 endturn
            if sorted.is_empty() {
                vec![valid[valid.len() - 1]] // 最后一个通常是 endturn
            } else {
                sorted.truncate(max_wide);
                sorted.into_iter().copied().collect()
            }
        } else {
            valid
        };

        let mut best_value = f32::NEG_INFINITY;
        let db = crate::card_db::CardDb::default();

        for action in candidates {
            let mut child = pf.deep_clone();
            child.do_action(action, &db);

            // 如果是 endturn，模拟敌方回合
            let value = if action.action_type == ActionType::EndTurn {
                let enemy_sim = self.simulate_enemy_turn(&child, config, move_gen);
                child.value = enemy_sim;
                enemy_sim
            } else {
                self.minimax(&child, depth + 1, config, move_gen, behavior, calculated)
            };

            if value > best_value {
                best_value = value;
            }
        }

        best_value
    }

    /// 敌方回合模拟
    /// 在深度 0 或 endturn 后，假设敌方做出最佳应对
    fn simulate_enemy_turn(
        &self,
        pf: &Playfield,
        _config: &AiConfig,
        _move_gen: &MoveGenerator,
    ) -> f32 {
        let enemy_value = pf.value;
        let mut remaining_value = enemy_value;

        let mut own_threats: Vec<f32> = pf
            .own_minions
            .iter()
            .map(|m| (m.effective_angr() * 3 + m.effective_hp() * 2) as f32)
            .collect();
        own_threats.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

        for (i, threat) in own_threats.iter().enumerate() {
            if i < 2 && *threat > 0.0 {
                remaining_value -= threat * 0.7;
            }
        }

        remaining_value
    }

    // ============================================================
    // 下回合前瞻搜索
    // ============================================================

    /// 对已结束的本方回合进行浅层下回合搜索
    /// 评估在当前最佳行动后，下回合对方的价值响应
    pub fn next_turn_lookahead(
        &self,
        pf: &Playfield,
        own_depth: u32,
        config: &AiConfig,
        move_gen: &MoveGenerator,
        behavior: &dyn Behavior,
    ) -> f32 {
        // 只能在对方回合做前瞻（end_turn 后）
        if !pf.complete || pf.is_own_turn {
            return behavior.evaluate(pf);
        }

        let mut simulated = 0u32;
        let db = crate::card_db::CardDb::default();

        // 假设对方回合结束回到我方回合（简化：skip 对方回合）
        // 做一些我方的浅层搜索
        self.next_turn_dfs(
            pf,
            0,
            own_depth,
            config,
            move_gen,
            behavior,
            &db,
            &mut simulated,
        )
    }

    /// 下回合 DFS（深度限制在 NEXT_TURN_DEEP，宽度限制在 NEXT_TURN_MAX_WIDE）
    fn next_turn_dfs(
        &self,
        pf: &Playfield,
        depth: u32,
        max_depth: u32,
        config: &AiConfig,
        move_gen: &MoveGenerator,
        behavior: &dyn Behavior,
        _db: &crate::card_db::CardDb,
        simulated: &mut u32,
    ) -> f32 {
        *simulated += 1;
        if depth >= max_depth || *simulated >= NEXT_TURN_TOTAL_BOARDS {
            return behavior.evaluate(pf);
        }

        let moves = move_gen.get_move_list(pf, true);
        let valid: Vec<&Action> = moves.iter().filter(|a| a.penality < 500).collect();

        if valid.is_empty() || (valid.len() == 1 && valid[0].action_type == ActionType::EndTurn) {
            return behavior.evaluate(pf);
        }

        let max_wide = if depth == 0 {
            config.max_wide as usize
        } else {
            NEXT_TURN_MAX_WIDE
        };
        let candidates: Vec<&Action> = if valid.len() > max_wide {
            valid[..max_wide].to_vec()
        } else {
            valid
        };

        let mut best_value = f32::NEG_INFINITY;
        let db = crate::card_db::CardDb::default();

        for action in candidates {
            let mut child = pf.deep_clone();
            child.do_action(action, &db);

            let value = if action.action_type == ActionType::EndTurn {
                behavior.evaluate(&child)
            } else {
                self.next_turn_dfs(
                    &child,
                    depth + 1,
                    max_depth,
                    config,
                    move_gen,
                    behavior,
                    &db,
                    simulated,
                )
            };

            if value > best_value {
                best_value = value;
            }
        }

        best_value
    }
}
