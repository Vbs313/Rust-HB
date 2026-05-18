# Playfield do_action 核心链路实现

> 创建日期: 2026-05-17
> 状态: ✅ 编译通过

---

## 变更内容

**目标**: 打通 AI 搜索的核心链路 `MoveGenerator → Playfield::do_action → MiniSimulator`

### 已实现的 Playfield 方法

| 方法                          | 说明                                                    | 行数 |
| ----------------------------- | ------------------------------------------------------- | ---- |
| `do_action()`                 | 动作分发器，匹配 9 种 ActionType 分别处理               | 15   |
| `play_card()`                 | 出牌：扣费、移除手牌、按类型分发（随从/法术/武器/英雄） | 55   |
| `spell_damage()`              | 法术伤害：对目标造成伤害                                | 10   |
| `equip_weapon()`              | 装备武器：从 CardDB 读取属性生成 Weapon 实例            | 20   |
| `transform_hero()`            | 英雄变身：加护甲，更新英雄属性                          | 8    |
| `summon_minion()`             | 召唤随从：7 格限制、插入指定位置、编号                  | 25   |
| `remove_minion()`             | 移除随从：删除并重新编号                                | 8    |
| `minion_attack()`             | 随从攻击：伤害/反击/吸血/风怒/剧毒完整链                | 65   |
| `hero_attack()`               | 英雄攻击：砍脸/砍随从、武器耐久、攻击标记               | 30   |
| `minion_get_damage_or_heal()` | 伤害/治疗：护甲吸收、圣盾抵消、免疫检查、剧毒即死       | 65   |
| `cleanup_dead()`              | 死亡清理：保留 hp>0 的实体，重新编号                    | 12   |
| `use_hero_power()`            | 英雄技能：扣 2 费，默认加 2 甲                          | 12   |
| `trade_card()`                | 交易：扣 1 费、洗回牌库、抽牌                           | 15   |
| `forge_card()`                | 锻造（骨架）                                            | 3    |
| `use_location()`              | 地标（骨架）                                            | 3    |
| `use_titan_ability()`         | 泰坦技能（骨架）                                        | 3    |
| `compute_hash()`              | 局面哈希：去重用                                        | 25   |
| `draw_card()`                 | 抽牌：疲劳伤害、指定卡牌抽取                            | 18   |

### 编译结果

```bash
cargo check --target i686-pc-windows-msvc
# 0 errors, ~37 warnings (与前相同的非阻塞警告)
```

### 当前整体完成度

```
                             C# 原版           Rust 版         完成度
Playfield (数据结构)         ~5,000行           ~650行           13%
Playfield (操作方法)         全部实现           核心链路已实现      35%
MiniSimulator               完整DFS            骨架               20%
MoveGenerator               完整枚举            费用+攻击          30%
MonoRuntime                 动态扫描            None (骨架)        0%
```

## 下一步

1. **MoveGenerator** — 实现完整的目标剪枝、英雄技能/地标/泰坦动作
2. **MiniSimulator** — 实现 cutting_posibilities 宽深剪枝
3. **Mono 运行时** — 实现进程模块枚举和导出表解析
