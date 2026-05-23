//! ActionExecutor — AI 动作执行器（双模式回退）
//!
//! 将 AI 引擎的决策转换为游戏操作。支持两种执行模式：
//!
//! - IPC 模式: 通过命名管道发送 ActionCommand 到 BepInEx 插件
//! - InputSim 模式: 通过鼠标/键盘模拟直接操作 UI（需要坐标校准）
//!
//! 默认优先 IPC，IPC 不可用时自动回退到 InputSim。

use hb_game_api::game_actions::{GameActions, PlayCardParams, AttackParams};
use hb_input_sim::InputSimulator;
use hb_ipc::{ActionCommand, GameStateData};
use hb_silverfish_core::{
    action::{Action, ActionType},
    ai::Ai,
    minion::Minion,
    playfield::{HandCard, Playfield},
    CardType, Race,
};
use std::sync::Arc;

use crate::ipc_client_wrapper::IpcClientOps;

// ===== Public API =====

/// 对当前局面进行 AI 搜索并执行动作（自动回退）
///
/// 尝试顺序：
/// 1. IPC 模式（直接发送到游戏内部 API）
/// 2. InputSim 模式（通过 UI 模拟，需坐标校准）
pub async fn decide_and_execute(
    ipc: Option<Arc<dyn IpcClientOps>>,
    input: Option<&InputSimulator>,
    state: &GameStateData,
) -> Result<(), String> {
    // 1. 构建 Playfield 并运行 AI
    let pf = state_to_playfield(state);
    let ai = Ai::new();
    let action = ai.do_all(&pf).ok_or("AI returned no action, turn may be complete")?;
    tracing::info!("AI decided: {:?}", action.action_type);

    // 2. 优先 IPC 模式
    if let Some(ref ipc) = ipc {
        let cmd = action_to_command(state, &action);
        match ipc.perform_action(cmd).await {
            Ok(true) => {
                tracing::info!("IPC action executed: {:?}", action.action_type);
                return Ok(());
            }
            Ok(false) => tracing::warn!("IPC rejected action, falling back..."),
            Err(e) => tracing::warn!("IPC failed ({}), falling back...", e),
        }
    }

    // 3. 回退到 InputSim 模式
    if let Some(input) = input {
        match execute_via_input_sim(input, state, &action) {
            Ok(()) => {
                tracing::info!("InputSim action executed: {:?}", action.action_type);
                return Ok(());
            }
            Err(e) => tracing::warn!("InputSim also failed: {e}"),
        }
    }

    Err("All execution paths failed".into())
}

// ===== InputSim 执行路径 =====

/// 通过 InputSimulator 执行 AI 动作
///
/// 当前为骨架实现，所有具体操作返回 "not calibrated" 错误。
/// 完成坐标校准后，这里映射手牌/随从到屏幕坐标并模拟点击。
fn execute_via_input_sim(
    input: &InputSimulator,
    _state: &GameStateData,
    action: &Action,
) -> Result<(), String> {
    let actions = GameActions::new(input);

    match action.action_type {
        ActionType::EndTurn => {
            actions.end_turn().map_err(|e| format!("end_turn: {e}"))
        }
        ActionType::PlayCard => {
            let params = PlayCardParams {
                hand_index: action.hand_card.as_ref().map(|c| c.position as u32).unwrap_or(0),
                target_id: action.target.as_ref().map(|m| m.entity_id).unwrap_or(0),
                position: action.position,
                choice: action.choice,
            };
            actions.play_card(params).map_err(|e| format!("play_card: {e}"))
        }
        ActionType::AttackWithMinion | ActionType::AttackWithHero => {
            let params = AttackParams {
                attacker_id: action.source.as_ref().map(|m| m.entity_id).unwrap_or(0),
                target_id: action.target.as_ref().map(|m| m.entity_id).unwrap_or(0),
            };
            actions.attack(params).map_err(|e| format!("attack: {e}"))
        }
        ActionType::UseHeroPower => {
            let target_id = action.target.as_ref().map(|m| m.entity_id).unwrap_or(0);
            actions.use_hero_power(target_id).map_err(|e| format!("hero_power: {e}"))
        }
        // 其他动作类型暂不支持 InputSim 模式
        ActionType::Trade => Err("Trade not supported via InputSim".into()),
        ActionType::Forge => Err("Forge not supported via InputSim".into()),
        ActionType::UseLocation => Err("Location not supported via InputSim".into()),
        ActionType::UseTitanAbility => Err("Titan not supported via InputSim".into()),
        ActionType::LaunchStarship => Err("Starship not supported via InputSim".into()),
        ActionType::Rewind => Err("Rewind not supported via InputSim".into()),
    }
}

// ===== 状态映射 =====

/// 将 GameStateData 映射为 Playfield（与 DefaultRoutine 映射保持一致）
fn state_to_playfield(state: &GameStateData) -> Playfield {
    let mut pf = Playfield::new();
    pf.mana = state.own_mana as i32;
    pf.own_max_mana = state.own_max_mana as i32;
    pf.enemy_max_mana = state.own_max_mana as i32;
    pf.is_own_turn = state.is_own_turn;
    pf.own_hero.hp = state.own_hero.health;
    pf.own_hero.armor = state.own_hero.armor;
    pf.own_hero.angr = state.own_hero.attack;
    pf.own_hero.entity_id = state.own_hero.entity_id;
    pf.enemy_hero.hp = state.enemy_hero.health;
    pf.enemy_hero.armor = state.enemy_hero.armor;
    pf.enemy_hero.angr = state.enemy_hero.attack;
    pf.enemy_hero.entity_id = state.enemy_hero.entity_id;

    for (i, card) in state.own_hand.iter().enumerate() {
        pf.own_hand.push(HandCard {
            card_id: card.card_id.parse::<u32>().unwrap_or(0),
            entity_id: card.entity_id,
            position: i as i32,
            cost: card.cost,
            original_cost: card.cost,
            attack: card.attack,
            health: card.health,
            card_type: CardType::Minion,
            race: Race::None,
            is_choice: false,
            has_targets: false,
            is_tradeable: false,
            is_forge: false,
        });
    }
    for minion in &state.own_minions {
        let mut m = Minion::new_minion(
            minion.card_id.parse::<u32>().unwrap_or(0),
            minion.attack,
            minion.health,
        );
        m.entity_id = minion.entity_id;
        m.taunt = minion.has_taunt;
        m.divine_shield = minion.has_divine_shield;
        m.stealth = minion.has_stealth;
        m.poisonous = minion.has_poisonous;
        m.lifesteal = minion.has_lifesteal;
        m.ready = !minion.is_exhausted;
        pf.own_minions.push(m);
    }
    for minion in &state.enemy_minions {
        let mut m = Minion::new_minion(
            minion.card_id.parse::<u32>().unwrap_or(0),
            minion.attack,
            minion.health,
        );
        m.entity_id = minion.entity_id;
        m.taunt = minion.has_taunt;
        m.divine_shield = minion.has_divine_shield;
        m.stealth = minion.has_stealth;
        m.poisonous = minion.has_poisonous;
        m.lifesteal = minion.has_lifesteal;
        pf.enemy_minions.push(m);
    }
    pf
}

// ===== 动作转换 =====

/// 将 AI Action 转换为 IPC ActionCommand
fn action_to_command(state: &GameStateData, action: &Action) -> ActionCommand {
    let action_type = match action.action_type {
        ActionType::EndTurn => "EndTurn".into(),
        ActionType::PlayCard => "PlayCard".into(),
        ActionType::AttackWithHero => "AttackWithHero".into(),
        ActionType::UseHeroPower => "UseHeroPower".into(),
        ActionType::AttackWithMinion => "AttackWithMinion".into(),
        ActionType::Trade => "Trade".into(),
        ActionType::UseLocation => "UseLocation".into(),
        ActionType::UseTitanAbility => "UseTitanAbility".into(),
        ActionType::Forge => "Forge".into(),
        ActionType::LaunchStarship => "LaunchStarship".into(),
        ActionType::Rewind => "Rewind".into(),
    };

    let hand_index = action.hand_card.as_ref().map(|c| {
        state.own_hand.iter().position(|hc| hc.entity_id == c.entity_id)
            .unwrap_or(0) as u32
    });

    let target_id = action.target.as_ref().map(|m| m.entity_id);

    ActionCommand {
        action_type,
        hand_index,
        target_id,
        position: Some(action.position as u32),
        choice: Some(action.choice as u32),
    }
}
