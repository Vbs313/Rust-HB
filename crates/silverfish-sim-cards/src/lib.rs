#![allow(non_camel_case_types)]
//! # hb-silverfish-sim-cards
//!
//! 炉石传说卡牌模拟实现
//!
//! 替代 C# 版的 1000+ 卡牌模拟文件。
//! 每张卡牌实现 `SimTemplate` trait 中的钩子方法。
//!
//! ## 组织方式
//!
//! 按扩展包分模块：
//!
//! ```text
//! cards/
//! ├── classic/        # 经典（~220 张）
//! ├── tgt/            # 冠军的试炼（~100 张）
//! ├── loe/            # 探险者协会
//! ├── ung/            # 勇闯安戈洛
//! ├── cat/            # 大地的裂变
//! └── ...
//! ```
//!
//! ## 构建策略
//!
//! 短期内：手动移植关键卡牌（斩杀相关、常用卡）
//! 中期：用构建脚本从 C# 源码自动生成 Rust 代码
//! 长期：从 HearthstoneJSON 数据 + AI 辅助生成

pub mod classic;
pub mod tgt;

use hb_silverfish_core::minion::Minion;
use hb_silverfish_core::playfield::Playfield;
use hb_silverfish_core::sim_template::CardSim;
use hb_silverfish_core::CardId;

/// 全局卡牌模拟注册表
/// 通过 CardId 查找对应的 CardSim 实现
pub fn get_sim(card_id: CardId) -> Option<fn() -> Box<dyn CardSim>> {
    classic::get_sim(card_id).or_else(|| tgt::get_sim(card_id))
}

// 占位卡牌模拟：效果为空但可以打出
pub struct SimPlaceholder;

impl CardSim for SimPlaceholder {
    fn card_id(&self) -> CardId {
        0
    }
    fn on_card_play(
        &self,
        _pf: &mut Playfield,
        _own: bool,
        _target: Option<&Minion>,
        _choice: i32,
    ) {
    }
    hb_silverfish_core::impl_card_sim_defaults!();
}

/// 返回占位模拟（用于未实现的卡牌）
pub fn placeholder_sim() -> Box<dyn CardSim> {
    Box::new(SimPlaceholder)
}
