//! 日志系统
//!
//! 基于 tracing crate，支持:
//! - 控制台输出（带颜色）
//! - 文件日志（轮转）
//! - JSON 格式（用于分析）

use tracing_subscriber::{fmt, prelude::*, EnvFilter, Registry};

/// 初始化日志系统
pub fn init(log_level: &str, log_file: Option<&std::path::Path>) {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));

    let subscriber = Registry::default().with(env_filter);

    // 控制台输出
    let stdout_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .pretty();

    // 文件输出（可选）
    if let Some(path) = log_file {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("Failed to open log file");

        let file_layer = fmt::layer()
            .with_writer(std::sync::Mutex::new(file))
            .json()
            .with_target(true)
            .with_thread_ids(true);

        subscriber.with(stdout_layer).with(file_layer).init();
    } else {
        subscriber.with(stdout_layer).init();
    }

    tracing::info!("Logging initialized (level: {log_level})");
}
