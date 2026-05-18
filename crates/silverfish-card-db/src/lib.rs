//! # hb-silverfish-card-db
//!
//! 炉石传说卡牌数据库
//!
//! 替代 C# 版的 CardDB。卡牌数据在构建时从 `card_data.json` 生成，
//! 编译为静态数组，运行时零解析开销。

#![allow(non_upper_case_globals)]

use hb_silverfish_core::CardId;

// 包含 build.rs 生成的代码（包含 CardData 结构体、CARD_DATA 数组和查找函数）
include!(concat!(env!("OUT_DIR"), "/card_data.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_card_count() {
        // 至少有核心卡牌
        assert!(CARD_COUNT > 1000, "CardDB should have at least 1000 cards, got {}", CARD_COUNT);
    }

    #[test]
    fn test_card_by_id() {
        // 查找一个经典卡牌（CS1_069 = 602，北郡牧师）
        if let Some(card) = card_by_raw_id(602) {
            assert!(card.cost >= 0);
            // 北郡牧师应该是 2 费 1/3 随从
            println!("Card 602: {:?}", card);
        }
    }

    #[test]
    fn test_sorted_by_id() {
        // 验证数组按 ID 升序排列
        for i in 1..CARD_DATA.len() {
            assert!(
                CARD_DATA[i-1].id < CARD_DATA[i].id,
                "Card data not sorted at index {}: {} vs {}",
                i, CARD_DATA[i-1].id, CARD_DATA[i].id
            );
        }
    }

    #[test]
    fn test_first_card() {
        if CARD_COUNT > 0 {
            let first = &CARD_DATA[0];
            println!("First card: id={}, name_cn={}, name_en={}",
                     first.id, first.name_cn, first.name_en);
        }
    }

    #[test]
    fn test_all_cards_fn() {
        let cards = all_cards();
        assert_eq!(cards.len(), CARD_COUNT);
    }

    #[test]
    fn test_card_name_not_empty() {
        // 检查前 100 张卡牌都有中文名
        let limit = std::cmp::min(100, CARD_COUNT);
        for i in 0..limit {
            assert!(!CARD_DATA[i].name_cn.is_empty(),
                    "Card {} (id={}) has empty CN name", i, CARD_DATA[i].id);
        }
    }
}
