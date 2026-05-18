//! 经典扩展包卡牌模拟
//!
//! 包含常用的经典卡牌实现，作为编写卡牌模拟的参考。
//!
//! 命名规范: `sim_{卡牌ID小写}`

use hb_silverfish_core::minion::Minion;
use hb_silverfish_core::playfield::Playfield;
use hb_silverfish_core::sim_template::CardSim;
use hb_silverfish_core::CardId;

// ===== 注册表 =====

type SimFn = fn() -> Box<dyn CardSim>;

pub fn get_sim(card_id: CardId) -> Option<SimFn> {
    match card_id {
        // 以下数字来自 CardDB 的枚举值
        602 => Some(|| Box::new(Sim_CS1_069)),  // 北郡牧师
        1032 => Some(|| Box::new(Sim_CS2_029)), // 火球术
        213 => Some(|| Box::new(Sim_CS2_182)),  // 冰风雪人
        328 => Some(|| Box::new(Sim_CS2_172)),  // 工程师学徒
        1072 => Some(|| Box::new(Sim_CS2_189)), // 精灵龙
        1004 => Some(|| Box::new(Sim_CS1_002)), // 盾牌卫士
        _ => None,
    }
}

// ===== 北郡牧师 (CS1_069): 每当一个随从获得治疗时，抽一张牌 =====

pub struct Sim_CS1_069;

impl CardSim for Sim_CS1_069 {
    fn card_id(&self) -> CardId {
        602
    }
    fn on_card_play(&self, pf: &mut Playfield, own: bool, _target: Option<&Minion>, _choice: i32) {
        let pos = pf.own_minions.len();
        pf.summon_minion(self.card_id(), pos, own);
    }
    fn on_minion_got_healed_trigger(
        &self,
        pf: &mut Playfield,
        _m: &Minion,
        anz_own: i32,
        _anz_enemy: i32,
    ) {
        for _ in 0..anz_own {
            pf.draw_card(None);
        }
    }
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
    fn on_summon(&self, _pf: &mut Playfield, _m: &Minion) {}
    fn on_turn_start_trigger(&self, _pf: &mut Playfield, _own: bool) {}
    fn on_turn_end_trigger(&self, _pf: &mut Playfield, _own: bool) {}
    fn on_minion_is_going_to_sleep(&self, _pf: &mut Playfield, _m: &Minion) {}
}

// ===== 火球术 (CS2_029): 造成 6 点伤害 =====

pub struct Sim_CS2_029;

impl CardSim for Sim_CS2_029 {
    fn card_id(&self) -> CardId {
        1032
    }
    fn on_card_play(&self, pf: &mut Playfield, _own: bool, target: Option<&Minion>, _choice: i32) {
        if let Some(t) = target {
            pf.minion_get_damage_or_heal(t.entity_id, t.is_hero, !_own, 6, false);
        }
    }
    hb_silverfish_core::impl_card_sim_defaults!();
}

// ===== 冰风雪人 (CS2_182): 4/5 白板 =====

pub struct Sim_CS2_182;

impl CardSim for Sim_CS2_182 {
    fn card_id(&self) -> CardId {
        213
    }
    fn on_card_play(&self, pf: &mut Playfield, own: bool, _target: Option<&Minion>, _choice: i32) {
        let pos = pf.own_minions.len();
        pf.summon_minion(self.card_id(), pos, own);
    }
    hb_silverfish_core::impl_card_sim_defaults!();
}

// ===== 工程师学徒 (CS2_172): 战吼：抽一张牌 =====

pub struct Sim_CS2_172;

impl CardSim for Sim_CS2_172 {
    fn card_id(&self) -> CardId {
        328
    }
    fn on_card_play(&self, pf: &mut Playfield, own: bool, _target: Option<&Minion>, _choice: i32) {
        let pos = pf.own_minions.len();
        pf.summon_minion(self.card_id(), pos, own);
        pf.draw_card(None); // 战吼抽牌
    }
    hb_silverfish_core::impl_card_sim_defaults!();
}

// ===== 精灵龙 (CS2_189): 3/2 无法成为法术或英雄技能的目标 =====

pub struct Sim_CS2_189;

impl CardSim for Sim_CS2_189 {
    fn card_id(&self) -> CardId {
        1072
    }
    fn on_card_play(&self, pf: &mut Playfield, own: bool, _target: Option<&Minion>, _choice: i32) {
        let pos = pf.own_minions.len();
        pf.summon_minion(self.card_id(), pos, own);
        // 精灵龙的无法被选中是 innate 属性，在创建 Minion 时设置
        if own {
            if let Some(last) = pf.own_minions.last_mut() {
                last.elusive = true;
            }
        }
    }
    hb_silverfish_core::impl_card_sim_defaults!();
}

// ===== 盾牌卫士 (CS1_002): 0/4 嘲讽 =====

pub struct Sim_CS1_002;

impl CardSim for Sim_CS1_002 {
    fn card_id(&self) -> CardId {
        1004
    }
    fn on_card_play(&self, pf: &mut Playfield, own: bool, _target: Option<&Minion>, _choice: i32) {
        let pos = pf.own_minions.len();
        pf.summon_minion(self.card_id(), pos, own);
        if own {
            if let Some(last) = pf.own_minions.last_mut() {
                last.taunt = true;
            }
        }
    }
    hb_silverfish_core::impl_card_sim_defaults!();
}
