//! Mono 类映射定义
//!
//! 对应 C# 版的 Triton.Game.Mapping (1600+ 文件)。
//! 定义炉石传说游戏对象在 Mono 运行时中的结构映射。
//!
//! ## 设计
//!
//! 利用 Rust 的泛型和过程宏，将运行时的字段偏移声明为编译时常量：
//!
//! ```rust,ignore
//! #[mono_class("Assembly-CSharp", "Actor")]
//! struct Actor {
//!     #[mono_field(offset = 0x38)] m_entity: Option<MonoObject>,
//!     #[mono_field(offset = 0x50)] m_card: Option<MonoObject>,
//! }
//! ```

/// TAG 值枚举（对应炉石客户端的 GAME_TAGs）
/// 用于读取实体属性，值来自 C# HREngine.Bots.GAME_TAGs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum GameTag {
    // 基础
    Premium = 12,
    Playstate = 17,
    Step = 19,
    Turn = 20,
    Fatigue = 22,
    CurrentPlayer = 23,
    FirstPlayer = 24,
    ResourcesUsed = 25,
    Resources = 26,
    HeroEntity = 27,
    MaxHandSize = 28,
    StartHandSize = 29,
    PlayerId = 30,
    TeamId = 31,

    // 属性
    Exhausted = 43,
    Damage = 44,
    Health = 45,
    Atk = 47,
    Cost = 48,
    Zone = 49,
    Controller = 50,
    Owner = 51,
    EntityId = 53,
    Durability = 187,
    Armor = 292,

    // 关键词
    Taunt = 190,
    DivineShield = 194,
    Windfury = 189,
    Stealth = 191,
    Poisonous = 363,
    Lifesteal = 372,
    Rush = 791,
    Charge = 197,
    Reborn = 273,
    Immune = 240,
    MegaWindfury = 277,
    Spellpower = 192,
    Freeze = 208,

    // 状态
    Frozen = 260,
    Silenced = 188,
    Enraged = 212,
    JustPlayed = 261,
    NumAttacksThisTurn = 297,
    NumTurnsInPlay = 271,
    Summoned = 205,
    Creator = 313,
    CreatedBy = 318,
    DisplayedCreator = 385,

    // 类型/分类
    CardType = 202,
    Class = 199,
    Race = 200,
    Faction = 201,
    CardSet = 183,
    Rarity = 203,
    State = 204,
    CardId = 186,

    // 其他
    Overload = 215,
    Deathrattle = 217,
    Battlecry = 218,
    Secret = 219,
    Combo = 220,
    Inspire = 403,
}

impl GameTag {
    /// 从 i32 转换为枚举
    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            12 => Some(Self::Premium),
            17 => Some(Self::Playstate),
            19 => Some(Self::Step),
            20 => Some(Self::Turn),
            22 => Some(Self::Fatigue),
            23 => Some(Self::CurrentPlayer),
            24 => Some(Self::FirstPlayer),
            26 => Some(Self::Resources),
            27 => Some(Self::HeroEntity),
            43 => Some(Self::Exhausted),
            44 => Some(Self::Damage),
            45 => Some(Self::Health),
            47 => Some(Self::Atk),
            48 => Some(Self::Cost),
            49 => Some(Self::Zone),
            50 => Some(Self::Controller),
            51 => Some(Self::Owner),
            53 => Some(Self::EntityId),
            183 => Some(Self::CardSet),
            186 => Some(Self::CardId),
            187 => Some(Self::Durability),
            188 => Some(Self::Silenced),
            189 => Some(Self::Windfury),
            190 => Some(Self::Taunt),
            191 => Some(Self::Stealth),
            192 => Some(Self::Spellpower),
            194 => Some(Self::DivineShield),
            197 => Some(Self::Charge),
            199 => Some(Self::Class),
            200 => Some(Self::Race),
            201 => Some(Self::Faction),
            202 => Some(Self::CardType),
            203 => Some(Self::Rarity),
            204 => Some(Self::State),
            205 => Some(Self::Summoned),
            208 => Some(Self::Freeze),
            212 => Some(Self::Enraged),
            215 => Some(Self::Overload),
            217 => Some(Self::Deathrattle),
            218 => Some(Self::Battlecry),
            219 => Some(Self::Secret),
            220 => Some(Self::Combo),
            240 => Some(Self::Immune),
            260 => Some(Self::Frozen),
            261 => Some(Self::JustPlayed),
            271 => Some(Self::NumTurnsInPlay),
            273 => Some(Self::Reborn),
            277 => Some(Self::MegaWindfury),
            292 => Some(Self::Armor),
            297 => Some(Self::NumAttacksThisTurn),
            313 => Some(Self::Creator),
            318 => Some(Self::CreatedBy),
            363 => Some(Self::Poisonous),
            372 => Some(Self::Lifesteal),
            385 => Some(Self::DisplayedCreator),
            403 => Some(Self::Inspire),
            791 => Some(Self::Rush),
            _ => None,
        }
    }
}

/// 游戏实体 TAG 映射器
pub struct TagMapper;

impl TagMapper {
    /// 从实体读取指定 TAG 值
    pub fn read_tag(
        _process: &hb_core::win32::ProcessHandle,
        _entity_addr: usize,
        _tag: GameTag,
    ) -> Result<i32, hb_core::error::Error> {
        // 通过 Entity → m_tags → TagMap[tag] 读取
        // 需要知道正确的偏移链
        todo!("Tag reading requires runtime offset discovery")
    }
}
