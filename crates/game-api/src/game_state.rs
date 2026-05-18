//! 游戏状态读取
//!
//! 对应 C# 版 TritonHs 的 GameState 结构。
//! 通过 Mono 运行时读取炉石客户端的完整游戏状态。

use crate::GameError;
// unused
use hb_mono_bridge::MonoBridge;

/// 游戏场景枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameScene {
    Login,
    Hub,
    Tournament,
    Battlegrounds,
    Arena,
    Adventure,
    TavernBrawl,
    Mercenaries,
    Gameplay,
    Unknown,
}

/// 完整游戏状态
#[derive(Debug, Clone)]
pub struct GameState {
    pub scene: GameScene,
    pub own_hero: Entity,
    pub enemy_hero: Entity,
    pub own_hand: Vec<Entity>,
    pub own_minions: Vec<Entity>,
    pub enemy_hand_count: u32,
    pub enemy_minions: Vec<Entity>,
    pub own_weapon: Option<Entity>,
    pub enemy_weapon: Option<Entity>,
    pub own_secrets: Vec<Entity>,
    pub enemy_secret_count: u32,
    pub own_mana: u32,
    pub own_max_mana: u32,
    pub enemy_mana: u32,
    pub own_deck_count: u32,
    pub enemy_deck_count: u32,
    pub turn: u32,
    pub is_own_turn: bool,
}

impl GameState {
    /// 从进程读取完整游戏状态
    pub fn read_from_process(mono: &MonoBridge) -> Result<Self, GameError> {
        let scene = crate::scene_detector::detect_scene(mono)?;

        if scene == GameScene::Gameplay {
            Self::read_gameplay_state(mono, scene)
        } else {
            Ok(Self::empty(scene))
        }
    }

    /// 读取对战中的完整状态
    fn read_gameplay_state(mono: &MonoBridge, scene: GameScene) -> Result<Self, GameError> {
        let _process = mono.process();
        let _runtime = mono.runtime();

        // Step 1: 定位 GameStateManager 单例
        // 在炉石中，实体列表通常通过 GameStateManager.Get()
        // GameState → m_entities (List<Entity>)
        // GameState → m_playerMap (Map<int, Player>)
        //
        // 或者通过 GameEntity 单例遍历实体树

        // Step 2: 枚举所有实体
        let entities = Self::enumerate_all_entities(mono)?;

        // Step 3: 按 Zone 分类
        let mut state = Self::empty(scene);

        for entity in &entities {
            let zone = entity.zone;
            let is_own = entity.controller == entity.get_tag(TAG_PLAYER_ID);

            match zone {
                Zone::Hand if is_own => state.own_hand.push(entity.clone()),
                Zone::Play if is_own => {
                    if entity.is_hero() {
                        state.own_hero = entity.clone();
                    } else if entity.is_weapon() {
                        state.own_weapon = Some(entity.clone());
                    } else {
                        state.own_minions.push(entity.clone());
                    }
                }
                Zone::Play if !is_own => {
                    if entity.is_hero() {
                        state.enemy_hero = entity.clone();
                    } else if entity.is_weapon() {
                        state.enemy_weapon = Some(entity.clone());
                    } else {
                        state.enemy_minions.push(entity.clone());
                    }
                }
                Zone::Hand if !is_own => state.enemy_hand_count += 1,
                Zone::Deck if is_own => state.own_deck_count += 1,
                Zone::Deck if !is_own => state.enemy_deck_count += 1,
                Zone::Secret if is_own => state.own_secrets.push(entity.clone()),
                Zone::Secret if !is_own => state.enemy_secret_count += 1,
                _ => {}
            }
        }

        // Step 4: 读取资源信息
        if state.own_hero.entity_id != 0 {
            state.own_mana = state.own_hero.get_tag(TAG_RESOURCES) as u32;
            state.own_max_mana = state.own_hero.get_tag(TAG_RESOURCES) as u32;
        }
        if state.enemy_hero.entity_id != 0 {
            state.enemy_mana = state.enemy_hero.get_tag(TAG_RESOURCES) as u32;
        }

        // Step 5: 检测当前是否己方回合
        state.is_own_turn = detect_current_player(mono, &state);
        state.turn = state.own_hero.get_tag(TAG_TURN) as u32;

        Ok(state)
    }

    /// 枚举所有游戏实体
    fn enumerate_all_entities(_mono: &MonoBridge) -> Result<Vec<Entity>, GameError> {
        let entities = Vec::new();

        // 炉石实体枚举路径:
        // GameStateManager.Get() → GameState
        // GameState.m_entities → List<Entity>
        // 或通过 GameState.m_entityMap → Map<int, Entity>
        //
        // 实体结构:
        // Entity (MonoBehaviour 子类)
        //   ├── m_entityId: int
        //   ├── m_tags: Map<int, int> 或 Entity.Tags类型
        //   └── m_cardId: string
        //
        // 由于需要运行时偏移量，这里构建框架结构
        // 实际连接时从 GameStateManager 逐步遍历

        // 框架：返回空实体列表
        // TODO: 实现完整的 GameStateManager → Entity 链
        tracing::warn!("enumerate_all_entities not implemented");

        Ok(entities)
    }

    fn empty(scene: GameScene) -> Self {
        Self {
            scene,
            own_hero: Entity::empty(),
            enemy_hero: Entity::empty(),
            own_hand: Vec::new(),
            own_minions: Vec::new(),
            enemy_hand_count: 0,
            enemy_minions: Vec::new(),
            own_weapon: None,
            enemy_weapon: None,
            own_secrets: Vec::new(),
            enemy_secret_count: 0,
            own_mana: 0,
            own_max_mana: 0,
            enemy_mana: 0,
            own_deck_count: 0,
            enemy_deck_count: 0,
            turn: 0,
            is_own_turn: false,
        }
    }
}

/// 检测当前是否是己方回合
fn detect_current_player(_mono: &MonoBridge, state: &GameState) -> bool {
    let own = state.own_hero.get_tag(TAG_CURRENT_PLAYER);
    let enemy = state.enemy_hero.get_tag(TAG_CURRENT_PLAYER);
    own > enemy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zone_from_i32() {
        assert_eq!(Zone::from_i32(0), Zone::Invalid);
        assert_eq!(Zone::from_i32(1), Zone::Play);
        assert_eq!(Zone::from_i32(2), Zone::Deck);
        assert_eq!(Zone::from_i32(3), Zone::Hand);
        assert_eq!(Zone::from_i32(4), Zone::Graveyard);
        assert_eq!(Zone::from_i32(5), Zone::RemovedFromGame);
        assert_eq!(Zone::from_i32(6), Zone::Setaside);
        assert_eq!(Zone::from_i32(7), Zone::Secret);
        assert_eq!(Zone::from_i32(42), Zone::Invalid);
    }

    #[test]
    fn test_entity_empty() {
        let e = Entity::empty();
        assert_eq!(e.entity_id, 0);
        assert_eq!(e.address, 0);
        assert_eq!(e.card_id, "");
        assert!(e.tags.is_empty());
    }

    #[test]
    fn test_entity_get_tag() {
        let e = Entity {
            tags: vec![
                (44, 5),  // TAG_DAMAGE
                (45, 30), // TAG_HEALTH
                (47, 3),  // TAG_ATK
                (48, 2),  // TAG_COST
            ],
            ..Entity::empty()
        };
        assert_eq!(e.get_tag(TAG_DAMAGE), 5);
        assert_eq!(e.get_tag(TAG_HEALTH), 30);
        assert_eq!(e.get_tag(TAG_ATK), 3);
        assert_eq!(e.get_tag(TAG_COST), 2);
        assert_eq!(e.get_tag(TAG_ZONE), 0, "Missing tag should return 0");
    }

    #[test]
    fn test_entity_is_hero() {
        let mut e = Entity::empty();
        // TAG_CARDTYPE = 202, CARDTYPE_HERO = 3
        assert!(!e.is_hero(), "Empty entity is not a hero");
        e.tags.push((202, 3));
        assert!(e.is_hero());
    }

    #[test]
    fn test_entity_is_minion() {
        let mut e = Entity::empty();
        assert!(!e.is_minion());
        e.tags.push((202, 4));
        assert!(e.is_minion());
    }

    #[test]
    fn test_entity_is_weapon() {
        let mut e = Entity::empty();
        assert!(!e.is_weapon());
        e.tags.push((202, 7));
        assert!(e.is_weapon());
    }

    #[test]
    fn test_entity_is_spell() {
        let mut e = Entity::empty();
        assert!(!e.is_spell());
        e.tags.push((202, 5));
        assert!(e.is_spell());
    }

    #[test]
    fn test_entity_type_mutual_exclusive() {
        let mut e = Entity::empty();
        e.tags.push((202, 3)); // hero
        assert!(e.is_hero());
        assert!(!e.is_minion());
        assert!(!e.is_weapon());
        assert!(!e.is_spell());
    }

    #[test]
    fn test_game_state_empty() {
        let state = GameState::empty(GameScene::Unknown);
        assert_eq!(state.scene, GameScene::Unknown);
        assert_eq!(state.own_mana, 0);
        assert_eq!(state.own_hand.len(), 0);
        assert_eq!(state.own_minions.len(), 0);
        assert!(state.own_weapon.is_none());
        assert!(!state.is_own_turn);
    }

    #[test]
    fn test_game_state_classify_entities() {
        let mut state = GameState::empty(GameScene::Gameplay);

        let hero = Entity {
            entity_id: 1,
            tags: vec![(202, 3)], // hero
            ..Entity::empty()
        };
        let enemy_hero = Entity {
            entity_id: 2,
            tags: vec![(202, 3)], // hero
            ..Entity::empty()
        };
        let minion_on_board = Entity {
            entity_id: 3,
            tags: vec![(50, 1), (49, 1), (202, 4)], // controller=1, zone=Play, type=Minion
            ..Entity::empty()
        };
        let weapon = Entity {
            entity_id: 4,
            tags: vec![(50, 1), (49, 1), (202, 7)], // weapon in play
            ..Entity::empty()
        };

        // 手动分类
        state.own_hero = hero.clone();
        state.enemy_hero = enemy_hero;
        state.own_minions.push(minion_on_board);
        state.own_weapon = Some(weapon);

        assert_eq!(state.own_hero.entity_id, 1);
        assert_eq!(state.own_minions.len(), 1);
        assert!(state.own_weapon.is_some());
    }

    #[test]
    fn test_tag_constants() {
        assert_eq!(TAG_PLAYER_ID, 30);
        assert_eq!(TAG_ZONE, 49);
        assert_eq!(TAG_CARDTYPE, 202);
        assert_eq!(TAG_HEALTH, 45);
        assert_eq!(TAG_ATK, 47);
        assert_eq!(TAG_COST, 48);
        assert_eq!(TAG_DURABILITY, 187);
        assert_eq!(TAG_DAMAGE, 44);
        assert_eq!(TAG_TAUNT, 190);
        assert_eq!(TAG_DIVINE_SHIELD, 194);
        assert_eq!(TAG_STEALTH, 191);
        assert_eq!(TAG_WINDFURY, 189);
        assert_eq!(TAG_POISONOUS, 363);
        assert_eq!(TAG_NUM_ATTACKS, 297);
        assert_eq!(TAG_FROZEN, 260);
    }

    // detect_current_player 需要 MonoBridge 实例（外部 crate），集成测试覆盖
}

// ============================================================
// 游戏 TAG 常量（对应 C# 的 GAME_TAG 枚举值）
// ============================================================

pub const TAG_PLAYER_ID: i32 = 30;
pub const TAG_ZONE: i32 = 49;
pub const TAG_CONTROLLER: i32 = 50;
pub const TAG_ENTITY_ID: i32 = 53;
pub const TAG_CARD_ID: i32 = 186;
pub const TAG_CARDTYPE: i32 = 202;
pub const TAG_HEALTH: i32 = 45;
pub const TAG_ATK: i32 = 47;
pub const TAG_COST: i32 = 48;
pub const TAG_DURABILITY: i32 = 187;
pub const TAG_ARMOR: i32 = 292;
pub const TAG_DAMAGE: i32 = 44;
pub const TAG_RESOURCES: i32 = 26;
pub const TAG_RESOURCES_USED: i32 = 25;
pub const TAG_TURN: i32 = 20;
pub const TAG_CURRENT_PLAYER: i32 = 23;
pub const TAG_EXHAUSTED: i32 = 43;
pub const TAG_FROZEN: i32 = 260;
pub const TAG_STEALTH: i32 = 191;
pub const TAG_DIVINE_SHIELD: i32 = 194;
pub const TAG_TAUNT: i32 = 190;
pub const TAG_WINDFURY: i32 = 189;
pub const TAG_POISONOUS: i32 = 363;
pub const TAG_LIFESTEAL: i32 = 372;
pub const TAG_RUSH: i32 = 791;
pub const TAG_CHARGE: i32 = 197;
pub const TAG_REBORN: i32 = 273;
pub const TAG_SILENCED: i32 = 188;
pub const TAG_NUM_ATTACKS: i32 = 297;
pub const TAG_JUST_PLAYED: i32 = 261;

/// 实体区域枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    Invalid = 0,
    Play = 1,
    Deck = 2,
    Hand = 3,
    Graveyard = 4,
    RemovedFromGame = 5,
    Setaside = 6,
    Secret = 7,
}

impl Zone {
    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => Zone::Play,
            2 => Zone::Deck,
            3 => Zone::Hand,
            4 => Zone::Graveyard,
            5 => Zone::RemovedFromGame,
            6 => Zone::Setaside,
            7 => Zone::Secret,
            _ => Zone::Invalid,
        }
    }
}

/// 卡牌类型（对应 C# 的 TAG_CARDTYPE 值）
const CARDTYPE_HERO: i32 = 3;
const CARDTYPE_MINION: i32 = 4;
const CARDTYPE_SPELL: i32 = 5;
const CARDTYPE_WEAPON: i32 = 7;
const CARDTYPE_HERO_POWER: i32 = 10;
const CARDTYPE_LOCATION: i32 = 39;

/// 游戏实体
#[derive(Debug, Clone)]
pub struct Entity {
    /// Mono 对象地址
    pub address: usize,
    /// 实体 ID
    pub entity_id: i32,
    /// 卡牌 ID 字符串
    pub card_id: String,
    /// 区域
    pub zone: Zone,
    /// 控制者 ID
    pub controller: i32,
    /// 原始 TAG 键值对
    pub tags: Vec<(i32, i32)>,
}

impl Entity {
    pub fn empty() -> Self {
        Self {
            address: 0,
            entity_id: 0,
            card_id: String::new(),
            zone: Zone::Invalid,
            controller: 0,
            tags: Vec::new(),
        }
    }

    /// 读取实体的 TAG 值
    pub fn get_tag(&self, tag_id: i32) -> i32 {
        // 线性搜索（实体 TAG 数量通常 < 50，O(n) 足够快）
        for &(k, v) in &self.tags {
            if k == tag_id {
                return v;
            }
        }
        0
    }

    /// 是否是英雄
    pub fn is_hero(&self) -> bool {
        self.get_tag(TAG_CARDTYPE) == CARDTYPE_HERO
    }

    /// 是否是随从
    pub fn is_minion(&self) -> bool {
        self.get_tag(TAG_CARDTYPE) == CARDTYPE_MINION
    }

    /// 是否是武器
    pub fn is_weapon(&self) -> bool {
        self.get_tag(TAG_CARDTYPE) == CARDTYPE_WEAPON
    }

    /// 是否是法术
    pub fn is_spell(&self) -> bool {
        self.get_tag(TAG_CARDTYPE) == CARDTYPE_SPELL
    }

    /// 从 Mono 对象读取实体
    pub fn from_mono_object(_mono: &MonoBridge, _mono_addr: usize) -> Result<Self, GameError> {
        let mut entity = Entity::empty();
        entity.address = 0;

        // 读取路径:
        // MonoObject → Entity 实例
        // 1. 读取 m_entityId 字段（偏移约 0x38）
        // 2. 读取 m_cardId 字段（字符串指针，偏移约 0x3C）
        // 3. 读取 m_tags（偏移约 0x40，视具体版本可能变化）

        // 实际偏移量需要运行时检测（通过 MonoClass 的字段信息）
        // 这里构建框架，连接时使用运行时获取的偏移

        tracing::warn!("Entity::from_mono_object not fully implemented");
        Ok(entity)
    }
}
