//! i18n — 多语言支持（编译期求值，零运行时开销）
//!
//! 使用 phf::phf_map! 在编译期构建完美哈希表，
//! 运行时通过 t!() 宏或 tr() 函数查询。

use std::sync::atomic::{AtomicU8, Ordering};

static LANG: AtomicU8 = AtomicU8::new(0); // 0=en, 1=zh

/// 设置语言。"en" 或 "zh"
pub fn set_language(lang: &str) {
    match lang {
        "zh" | "zh-CN" | "zh-cn" => LANG.store(1, Ordering::Relaxed),
        _ => LANG.store(0, Ordering::Relaxed), // default English
    }
}

/// 查询当前语言的翻译
pub fn tr(key: &str) -> &'static str {
    let map = if LANG.load(Ordering::Relaxed) == 1 { &ZH } else { &EN };
    map.get(key).copied()
        .or_else(|| EN.get(key).copied())
        .unwrap_or("?")
}

/// 翻译宏：t!("key") 等价于 i18n::tr("key")
#[macro_export]
macro_rules! t {
    ($key:expr) => {
        $crate::i18n::tr($key)
    };
}

// ===== English (default) =====
pub(crate) static EN: phf::Map<&'static str, &'static str> = phf::phf_map! {
    // Header
    "app.title"               => "Hearthbuddy Rust Edition",

    // IPC / connection
    "ipc.connecting"          => "Connecting to IPC...",
    "ipc.connected"           => "IPC connected!",
    "ipc.failed"              => "IPC failed: {e}",
    "ipc.reconnecting"        => "Reconnecting...",
    "ipc.reconnected"         => "Reconnected after {n} retries",
    "status.connected"        => "Connected",
    "status.disconnected"     => "Disconnected",
    "status.ai_idle"          => "AI: Idle",
    "status.ai_searching"     => "AI: Searching...",

    // Game state panel
    "panel.game_state"        => "Game State",
    "panel.session"           => "Session",
    "panel.connection"        => "Connection",
    "panel.log"               => "Action Log",

    // Game state fields
    "hp"                      => "HP",
    "mana"                    => "Mana",
    "hand"                    => "Hand",
    "board"                   => "Board",
    "enemy_board"             => "Enemy Board",
    "turn"                    => "Turn",
    "armor"                   => "Armor",
    "attack"                  => "Atk",
    "deck"                    => "Deck",
    "scene"                   => "Scene",

    // Session stats
    "games"                   => "Games",
    "wins"                    => "Wins",
    "losses"                  => "Losses",
    "winrate"                 => "Win Rate",
    "streak"                  => "Streak",

    // Game loop messages
    "shutdown.received"       => "Shutdown signal received, exiting",
    "scene.waiting"           => "Scene: {s}",
    "turn.our"                => "Our turn! Mana: {m}/{max}",
    "turn.waiting"            => "Waiting for our turn... ({t})",
    "turn.number"             => "Turn {n}",

    // AI / actions
    "ai.decided"              => "AI chose: {a}",
    "ai.executed"             => "Action dispatched: {a}",
    "ai.skipped"              => "Action skipped: {e}",
    "ai.error"                => "Routine error: {e}",

    // Footer / keybinds
    "keybinds.tab"            => "Tab: Switch",
    "keybinds.quit"           => "Q: Quit",
    "keybinds.reset"          => "R: Reset",
    "keybinds.lang"           => "L: EN/ZH",

    // General
    "press_ctrl_c"            => "Ctrl+C to stop",
    "yes"                     => "Yes",
    "no"                      => "No",
};

// ===== Chinese (Simplified) =====
pub(crate) static ZH: phf::Map<&'static str, &'static str> = phf::phf_map! {
    "app.title"               => "炉石兄弟 Rust 版",

    "ipc.connecting"          => "正在连接 IPC...",
    "ipc.connected"           => "IPC 已连接！",
    "ipc.failed"              => "IPC 失败：{e}",
    "ipc.reconnecting"        => "正在重连...",
    "ipc.reconnected"         => "已重连（第 {n} 次）",
    "status.connected"        => "已连接",
    "status.disconnected"     => "已断开",
    "status.ai_idle"          => "AI：空闲",
    "status.ai_searching"     => "AI：搜索中...",

    "panel.game_state"        => "游戏状态",
    "panel.session"           => "对局统计",
    "panel.connection"        => "连接",
    "panel.log"               => "操作日志",

    "hp"                      => "血量",
    "mana"                    => "法力值",
    "hand"                    => "手牌",
    "board"                   => "场面",
    "enemy_board"             => "敌方场面",
    "turn"                    => "回合",
    "armor"                   => "护甲",
    "attack"                  => "攻击力",
    "deck"                    => "牌库",
    "scene"                   => "场景",

    "games"                   => "总场次",
    "wins"                    => "胜场",
    "losses"                  => "负场",
    "winrate"                 => "胜率",
    "streak"                  => "连胜",

    "shutdown.received"       => "收到关闭信号，正在退出",
    "scene.waiting"           => "当前场景：{s}",
    "turn.our"                => "我方回合！法力值：{m}/{max}",
    "turn.waiting"            => "等待我方回合...（{t}）",
    "turn.number"             => "第 {n} 回合",

    "ai.decided"              => "AI 选择：{a}",
    "ai.executed"             => "动作已执行：{a}",
    "ai.skipped"              => "动作跳过：{e}",
    "ai.error"                => "策略错误：{e}",

    "keybinds.tab"            => "Tab：切换",
    "keybinds.quit"           => "Q：退出",
    "keybinds.reset"          => "R：重置",
    "keybinds.lang"           => "L：中/EN",

    "press_ctrl_c"            => "按 Ctrl+C 停止",
    "yes"                     => "是",
    "no"                      => "否",
};
