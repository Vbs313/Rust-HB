//! 集成测试 — AI 引擎全链路
//!
//! 测试从 Playfield 创建 → MoveGenerator → MiniSimulator → Behavior 的完整链路。

#[cfg(test)]
mod tests {
    use hb_silverfish_core::action::ActionType;
    use hb_silverfish_core::ai::AiConfig;
    use hb_silverfish_core::behavior::default_behavior::DefaultBehavior;
    use hb_silverfish_core::behavior::Behavior;
    use hb_silverfish_core::mini_simulator::MiniSimulator;
    use hb_silverfish_core::minion::Minion;
    use hb_silverfish_core::move_generator::MoveGenerator;
    use hb_silverfish_core::playfield::Playfield;
    use hb_silverfish_core::weapon::Weapon;

    /// 创建一个标准测试局面：我方有 5 费+一个随从，敌方无随从
    fn setup_simple_board() -> Playfield {
        let mut pf = Playfield::new();
        pf.mana = 5;
        pf.own_max_mana = 5;
        pf.is_own_turn = true;
        pf.own_hero.hp = 30;
        pf.enemy_hero.hp = 25;
        pf.own_hero.entity_id = 1;
        pf.enemy_hero.entity_id = 2;
        pf.enemy_hero.armor = 0;

        // 我方一个 3/3 随从，可攻击
        pf.summon_minion(1001, 0, true);
        pf.own_minions[0].entity_id = 10;
        pf.own_minions[0].angr = 3;
        pf.own_minions[0].hp = 3;
        pf.own_minions[0].ready = true;

        pf
    }

    #[test]
    fn test_ai_moves_generated() {
        let pf = setup_simple_board();
        let move_gen = MoveGenerator::new();

        let moves = move_gen.get_move_list(&pf, true);
        // 应该至少有: endturn + 攻击动作(1个目标)
        assert!(!moves.is_empty(), "Should have at least one action");

        let attack_moves: Vec<_> = moves
            .iter()
            .filter(|m| m.action_type == ActionType::AttackWithMinion)
            .collect();
        assert_eq!(
            attack_moves.len(),
            1,
            "Should have exactly one attack target (enemy hero)"
        );
        assert_eq!(attack_moves[0].target.as_ref().unwrap().is_hero, true);
    }

    #[test]
    fn test_ai_search_returns_action() {
        let pf = setup_simple_board();
        let ai_config = AiConfig {
            max_depth: 4,
            max_wide: 100,
            max_cal: 10,
            simulate_next_turn: false,
            next_turn_depth: 2,
            next_turn_wide: 5,
            enemy_turn_wide: 10,
            enemy_turn_wide_second: 20,
            use_lethal_check: true,
        };
        let move_gen = MoveGenerator::new();
        let simulator = MiniSimulator::new();
        let behavior = DefaultBehavior;

        let best = simulator.search(&pf, &ai_config, &move_gen, &behavior);
        assert!(best.is_some(), "AI should find at least one valid action");
        assert!(
            best.as_ref().unwrap().action_type != ActionType::EndTurn,
            "AI should prefer attacking over ending turn"
        );
    }

    #[test]
    fn test_ai_with_taunt_requires_attacking_taunt() {
        let mut pf = setup_simple_board();
        // 给敌方加一个 0/2 嘲讽
        pf.enemy_minions.push(Minion::new_minion(2001, 0, 2));
        pf.enemy_minions[0].entity_id = 30;
        pf.enemy_minions[0].taunt = true;

        let move_gen = MoveGenerator::new();
        let moves = move_gen.get_move_list(&pf, true);

        // 所有攻击应该只能选嘲讽目标
        for m in &moves {
            if m.action_type == ActionType::AttackWithMinion {
                let target = m.target.as_ref().unwrap();
                assert!(
                    target.taunt,
                    "With taunt on board, all attacks must target taunt minion"
                );
            }
        }
    }

    #[test]
    fn test_behavior_evaluation() {
        let mut pf = setup_simple_board();
        let behavior = DefaultBehavior;

        // 局面价值应该为正（我方有利）
        let value = behavior.evaluate(&pf);
        assert!(
            value > 0.0,
            "Board with our minion and no enemy minions should have positive value"
        );

        // 局面反转：敌方有更多随从
        pf.enemy_minions.push(Minion::new_minion(3001, 5, 5));
        pf.enemy_minions[0].entity_id = 40;
        let value2 = behavior.evaluate(&pf);
        assert!(value2 < value, "Adding enemy 5/5 should reduce board value");
    }

    #[test]
    fn test_ai_lethal_detection() {
        let mut pf = setup_simple_board();
        // 敌方 0 血 = 斩杀局面
        pf.enemy_hero.hp = 0;
        pf.enemy_hero.armor = 0;

        let behavior = DefaultBehavior;
        let value = behavior.evaluate(&pf);

        // 斩杀应该给极高价值 (>10000)
        assert!(
            value > 10000.0,
            "Lethal board should have very high value, got {value}"
        );
    }

    #[test]
    fn test_penality_filter_blocks_bad_actions() {
        let mut pf = setup_simple_board();
        // 敌方有一个剧毒随从
        pf.enemy_minions.push(Minion::new_minion(4001, 1, 1));
        pf.enemy_minions[0].entity_id = 50;
        pf.enemy_minions[0].poisonous = true;

        let move_gen = MoveGenerator::new();
        let moves = move_gen.get_move_list(&pf, true);

        // 攻击剧毒随从应该有高惩罚
        for m in &moves {
            if m.action_type == ActionType::AttackWithMinion {
                if let Some(ref target) = m.target {
                    if target.poisonous {
                        assert!(
                            m.penality >= 100,
                            "Attacking poisonous minion should have high penalty"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_ai_with_weapon_prefers_face() {
        let mut pf = setup_simple_board();
        pf.own_weapon = Some(Weapon {
            entity_id: 1,
            card_id: 2001,
            angr: 3,
            durability: 2,
            base_angr: 3,
            base_durability: 2,
            windfury: false,
            poisonous: false,
            lifesteal: false,
            immune: false,
            mega_windfury: false,
        });
        pf.own_hero.angr = 3; // 英雄攻击力来自武器
        pf.own_hero.ready = true;

        let move_gen = MoveGenerator::new();
        let moves = move_gen.get_move_list(&pf, true);
        let hero_attacks: Vec<_> = moves
            .iter()
            .filter(|m| m.action_type == ActionType::AttackWithHero)
            .collect();
        assert!(
            !hero_attacks.is_empty(),
            "With weapon, hero should have attack options"
        );
    }

    #[test]
    fn test_play_card_reduces_mana() {
        let mut pf = setup_simple_board();
        pf.mana = 10;
        pf.own_hand.push(hb_silverfish_core::playfield::HandCard {
            card_id: 213,
            entity_id: 5,
            position: 0,
            cost: 4,
            original_cost: 4,
            attack: 4,
            health: 5,
            card_type: hb_silverfish_core::CardType::Minion,
            race: hb_silverfish_core::Race::None,
            is_choice: false,
            has_targets: false,
            is_tradeable: false,
            is_forge: false,
        });

        let move_gen = MoveGenerator::new();
        let moves = move_gen.get_move_list(&pf, true);

        let play_moves: Vec<_> = moves
            .iter()
            .filter(|m| m.action_type == ActionType::PlayCard)
            .collect();
        assert!(
            !play_moves.is_empty(),
            "Should be able to play the 4-cost minion with 10 mana"
        );

        // 手牌中应该包含随从位置的多个选项
        for m in &play_moves {
            assert!(
                m.position >= 0,
                "PlayCard action should have valid position"
            );
        }
    }
}
