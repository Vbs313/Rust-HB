//! # hb-core
//!
//! Hearthbuddy 核心运行库
//!
//! ## 职责
//! - 日志系统 (tracing 封装)
//! - 配置管理 (JSON/TOML 双格式)
//! - 通用工具函数
//! - 错误类型定义
//! - Win32 公共绑定

#![allow(non_snake_case, dead_code)]

pub mod config;
pub mod error;
pub mod log;
pub mod win32;

use std::path::PathBuf;

/// 项目根目录（从可执行文件路径推导）
pub fn project_root() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// 数据目录（配置、日志、缓存）
pub fn data_dir() -> PathBuf {
    project_root().join("data")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_root_exists() {
        let root = project_root();
        assert!(!root.as_os_str().is_empty(), "Project root should exist");
    }

    #[test]
    fn test_data_dir_contains_data() {
        let dir = data_dir();
        assert!(dir.to_string_lossy().contains("data"));
    }
}