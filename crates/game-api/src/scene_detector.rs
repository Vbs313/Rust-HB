//! 场景检测器
//!
//! 通过 Mono 运行时读取当前游戏场景。
//! 检测逻辑基于 SceneMgr 单例的 m_scene 字段。

use crate::game_state::GameScene;
use crate::GameError;
use hb_mono_bridge::MonoBridge;

/// 检测当前游戏场景
pub fn detect_scene(mono: &MonoBridge) -> Result<GameScene, GameError> {
    // 方案 1: 通过 Box Dungeon Manager (BDK) 或 SceneMgr
    // 炉石客户端使用 BoxSingleton<SceneMgr> 管理场景
    // SceneMgr 的 m_scene 字段是一个 SceneType 枚举

    // 方案 2: 通过 UI 层级栈 (OverlayUI / UIContext)
    // 不同的场景有不同的 UI 结构

    // 方案 3: 通过场景特异的 Mono 对象存在性判断
    if let Ok(scene) = detect_via_scene_mgr(mono) {
        return Ok(scene);
    }

    // 回退：通过弹窗/UI 判断
    detect_via_ui(mono)
}

/// 通过 SceneMgr 单例检测
fn detect_via_scene_mgr(mono: &MonoBridge) -> Result<GameScene, GameError> {
    let runtime = mono.runtime();

    // 尝试查找 SceneMgr 类
    // 在 Assembly-CSharp 中: SceneMgr 类
    // 通常通过 BoxSingleton<SceneMgr>.Get() 访问

    // Step 1: 定位 SceneMgr 的 MonoClass
    // 查找模式: Assembly-CSharp 中类名包含 "SceneMgr"
    let _image = runtime.corlib_addr; // 先用 corlib，实际应该在 Assembly-CSharp

    // 尝试从 Assembly-CSharp 映像查找 SceneMgr
    // 这里简化：通过已知偏移读取当前场景类型
    // 在实际实现中，需要:
    // 1. 获取 Assembly-CSharp 映像
    // 2. 查找 SceneMgr 类
    // 3. 获取单例实例
    // 4. 读取 m_scene 字段

    // 对于调试/测试，返回 Gameplay
    // TODO: 实现完整的 SceneMgr 检测链
    Err(GameError::StateRead("SceneMgr not found".into()))
}

/// 通过 UI 元素判断场景
fn detect_via_ui(mono: &MonoBridge) -> Result<GameScene, GameError> {
    // 检查各种 UI 对象的存在性

    // 登录界面: LoginButton / AccountIcon
    if check_ui_object(mono, "LoginButton")? {
        return Ok(GameScene::Login);
    }

    // 主界面: PlayButton / CollectionButton
    if check_ui_object(mono, "PlayButton")? {
        return Ok(GameScene::Hub);
    }

    // 对战中: EndTurnButton / TrayDisplay
    if check_ui_object(mono, "EndTurnButton")? || check_ui_object(mono, "TrayDisplay")? {
        return Ok(GameScene::Gameplay);
    }

    // 默认
    Ok(GameScene::Unknown)
}

/// 检查特定名称的 UI MonoBehavior 是否存在
fn check_ui_object(mono: &MonoBridge, _name: &str) -> Result<bool, GameError> {
    // 遍历 Mono 堆上的对象，检查类型名称
    // 简化实现
    let _ = mono;
    Ok(false)
}

/// 检测当前是否有活动弹窗
pub fn detect_active_dialog(mono: &MonoBridge) -> Result<Option<DialogType>, GameError> {
    // 遍历常见弹窗检测器
    let checks: &[(DialogType, &dyn Fn(&MonoBridge) -> Result<bool, GameError>)] = &[
        (DialogType::OkDialog, &|m| {
            check_dialog_by_class(m, "OKButton")
        }),
        (DialogType::DeckPicker, &|m| {
            check_dialog_by_class(m, "DeckPickerTray")
        }),
        (DialogType::Reward, &|m| {
            check_dialog_by_class(m, "RewardXp")
        }),
        (DialogType::QuestDialog, &|m| {
            check_dialog_by_class(m, "QuestDialog")
        }),
    ];

    for (dialog_type, checker) in checks {
        if checker(mono)? {
            return Ok(Some(*dialog_type));
        }
    }

    Ok(None)
}

/// 通过 Mono 类名检测弹窗
fn check_dialog_by_class(_mono: &MonoBridge, _class_name: &str) -> Result<bool, GameError> {
    // 检查特定弹窗类是否有活跃实例
    // TODO: 使用 MonoImage.find_class + 实例枚举
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_scene_variants() {
        let scenes = [
            GameScene::Login,
            GameScene::Hub,
            GameScene::Tournament,
            GameScene::Battlegrounds,
            GameScene::Arena,
            GameScene::Adventure,
            GameScene::TavernBrawl,
            GameScene::Mercenaries,
            GameScene::Gameplay,
            GameScene::Unknown,
        ];
        assert_eq!(scenes.len(), 10);
    }

    #[test]
    fn test_dialog_type_variants() {
        let dialogs = [
            DialogType::GamemodeSelection,
            DialogType::CardBackSelection,
            DialogType::DeckPicker,
            DialogType::Reward,
            DialogType::LevelUp,
            DialogType::OpenPack,
            DialogType::QuestDialog,
            DialogType::OkDialog,
            DialogType::YesNoDialog,
            DialogType::ChooseOneDialog,
            DialogType::GenericPopup,
            DialogType::AdventureSelect,
            DialogType::TavernBrawlSelect,
            DialogType::BgsHeroSelect,
            DialogType::BgsQuestSelect,
            DialogType::BgsReward,
            DialogType::BgsTriplesBoard,
            DialogType::BgsTeammateBoard,
            DialogType::ArenaDraft,
            DialogType::ArenaReward,
            DialogType::ArenaScore,
            DialogType::MercenariesTask,
            DialogType::MercenariesReward,
        ];
        assert_eq!(dialogs.len(), 23);
        // all distinct
        for i in 0..dialogs.len() {
            for j in i + 1..dialogs.len() {
                assert_ne!(dialogs[i], dialogs[j]);
            }
        }
    }

    // detect_scene 和 detect_active_dialog 需要真实 Mono 连接外部进程，跳过测试
}

/// 弹窗类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogType {
    GamemodeSelection,
    CardBackSelection,
    DeckPicker,
    Reward,
    LevelUp,
    OpenPack,
    QuestDialog,
    OkDialog,
    YesNoDialog,
    ChooseOneDialog,
    GenericPopup,
    AdventureSelect,
    TavernBrawlSelect,
    BgsHeroSelect,
    BgsQuestSelect,
    BgsReward,
    BgsTriplesBoard,
    BgsTeammateBoard,
    ArenaDraft,
    ArenaReward,
    ArenaScore,
    MercenariesTask,
    MercenariesReward,
}
