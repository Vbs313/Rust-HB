//! 卡牌模拟模板
//!
//! 对应 C# 版的 SimTemplate.cs
//! 定义所有卡牌模拟的基类特性。具体的卡牌模拟通过 Rust 的 trait 实现。

use crate::minion::Minion;
use crate::playfield::Playfield;
use crate::CardId;

/// 卡牌模拟 trait
///
/// 每个具体的卡牌（如 北郡牧师、火球术）实现此 trait，
/// 重写需要的事件钩子。
pub trait CardSim: Send + Sync {
    /// 获取卡牌 ID
    fn card_id(&self) -> CardId;

    /// 打出卡牌时的效果
    fn on_card_play(&self, pf: &mut Playfield, own: bool, target: Option<&Minion>, choice: i32);

    /// 战吼效果
    fn get_battlecry_effect(
        &self,
        pf: &mut Playfield,
        own: Minion,
        target: Option<&Minion>,
        choice: i32,
    );

    /// 亡语效果
    fn on_deathrattle(&self, pf: &mut Playfield, source: Minion);

    /// 受伤触发
    fn on_minion_got_dmg_trigger(
        &self,
        pf: &mut Playfield,
        m: &Minion,
        anz_own: i32,
        anz_enemy: i32,
    );

    /// 治疗触发
    fn on_minion_got_healed_trigger(
        &self,
        pf: &mut Playfield,
        m: &Minion,
        anz_own: i32,
        anz_enemy: i32,
    );

    /// 召唤触发
    fn on_summon(&self, pf: &mut Playfield, m: &Minion);

    /// 回合开始触发
    fn on_turn_start_trigger(&self, pf: &mut Playfield, own: bool);

    /// 回合结束触发
    fn on_turn_end_trigger(&self, pf: &mut Playfield, own: bool);

    /// 休眠结束
    fn on_minion_is_going_to_sleep(&self, pf: &mut Playfield, m: &Minion);

    /// 发现/探底价值评估
    fn get_discover_val(&self, _card: crate::CardId, _pf: &Playfield) -> f32 {
        0.0
    }
}

/// 默认空实现（大多数卡牌不需要重写所有钩子）
#[macro_export]
macro_rules! impl_card_sim_defaults {
    () => {
        fn get_battlecry_effect(
            &self,
            _pf: &mut Playfield,
            _own: Minion,
            _target: Option<&Minion>,
            _choice: i32,
        ) {
        }
        fn on_deathrattle(&self, _pf: &mut Playfield, _source: Minion) {}
        fn on_minion_got_dmg_trigger(
            &self,
            _pf: &mut Playfield,
            _m: &Minion,
            _anz_own: i32,
            _anz_enemy: i32,
        ) {
        }
        fn on_minion_got_healed_trigger(
            &self,
            _pf: &mut Playfield,
            _m: &Minion,
            _anz_own: i32,
            _anz_enemy: i32,
        ) {
        }
        fn on_summon(&self, _pf: &mut Playfield, _m: &Minion) {}
        fn on_turn_start_trigger(&self, _pf: &mut Playfield, _own: bool) {}
        fn on_turn_end_trigger(&self, _pf: &mut Playfield, _own: bool) {}
        fn on_minion_is_going_to_sleep(&self, _pf: &mut Playfield, _m: &Minion) {}
    };
}

// 单例卡牌模拟注册
// 使用 inventory crate 模式在编译时注册所有卡牌模拟。
// 构建时从 1000+ C# SimTemplate 派生类生成对应 Rust 代码。
