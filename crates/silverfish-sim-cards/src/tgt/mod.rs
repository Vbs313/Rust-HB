//! 冠军的试炼扩展包卡牌模拟

use hb_silverfish_core::sim_template::CardSim;
use hb_silverfish_core::CardId;

pub fn get_sim(_card_id: CardId) -> Option<fn() -> Box<dyn CardSim>> {
    // TODO: 添加冠军的试炼卡牌模拟注册
    None
}
