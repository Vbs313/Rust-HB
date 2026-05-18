//! 内置插件
//!
//! 对应 C# 版的 AutoStop / Monitor / Quest / Stats 四个插件。
//! 所有插件实现 hb_bot_framework::Plugin trait。

pub mod auto_stop;
pub mod monitor;
pub mod quest;
pub mod stats;

use std::sync::Arc;
use hb_bot_framework::Plugin;

/// 注册所有内置插件
pub fn register_all() -> Vec<Arc<dyn Plugin>> {
    vec![
        Arc::new(auto_stop::AutoStopPlugin::new()),
        Arc::new(monitor::MonitorPlugin::new()),
        Arc::new(quest::QuestPlugin::new()),
        Arc::new(stats::StatsPlugin::new()),
    ]
}
