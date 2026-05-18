//! 配置管理模块
//!
//! 支持 JSON 和 TOML 双格式加载，支持分层覆盖：
//! 1. 默认配置（编译时内嵌）
//! 2. 用户配置文件
//! 3. 命令行参数覆盖

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 全局应用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// 日志级别
    pub log_level: String,
    /// 日志文件路径
    pub log_file: Option<PathBuf>,
    /// 炉石传说进程名
    pub hearthstone_process: String,
    /// 轮询间隔（毫秒）
    pub poll_interval_ms: u64,
    /// 鼠标移动速度模式
    pub mouse_speed_mode: MouseSpeedMode,
    /// 认证配置
    pub auth: AuthConfig,
    /// 机器人配置
    pub bot: BotConfig,
    /// AI 引擎配置
    pub ai: AiConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            log_level: "info".into(),
            log_file: None,
            hearthstone_process: "Hearthstone".into(),
            poll_interval_ms: 50,
            mouse_speed_mode: MouseSpeedMode::HumanLike,
            auth: AuthConfig::default(),
            bot: BotConfig::default(),
            ai: AiConfig::default(),
        }
    }
}

/// 鼠标速度模式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MouseSpeedMode {
    /// 极速（不模拟人类）
    Instant,
    /// 人类模拟（贝塞尔曲线）
    HumanLike,
    /// 自定义速度
    Custom {
        min_delay_ms: u64,
        max_delay_ms: u64,
    },
}

/// 认证配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthConfig {
    pub enabled: bool,
    pub server_url: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            server_url: "https://auth.hearthbuddy.dev".into(),
        }
    }
}

/// 机器人配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BotConfig {
    pub default_bot: String,
    pub default_routine: String,
    pub plugins: Vec<String>,
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            default_bot: "DefaultBot".into(),
            default_routine: "DefaultRoutine".into(),
            plugins: vec![],
        }
    }
}

/// AI 引擎配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AiConfig {
    /// 最大搜索深度
    pub max_depth: u32,
    /// 搜索宽度（每步最大场面数）
    pub max_wide: u32,
    /// 每步保留局面数
    pub max_cal: u32,
    /// 是否开启下回合模拟
    pub simulate_next_turn: bool,
    /// 下回合搜索深度
    pub next_turn_depth: u32,
    /// 敌方模拟宽度
    pub enemy_turn_wide: u32,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            max_depth: 12,
            max_wide: 3000,
            max_cal: 60,
            simulate_next_turn: true,
            next_turn_depth: 6,
            enemy_turn_wide: 40,
        }
    }
}

impl AppConfig {
    /// 从文件加载配置
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, crate::error::Error> {
        let content = std::fs::read_to_string(path.as_ref())?;
        let ext = path
            .as_ref()
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("json");

        match ext {
            "json" => Ok(serde_json::from_str(&content)?),
            "toml" => Ok(toml::from_str(&content)?),
            _ => Err(crate::error::Error::Config(format!(
                "Unsupported config format: {ext}"
            ))),
        }
    }

    /// 加载配置链（默认 → 用户配置 → 项目配置）
    pub fn load_chain() -> Result<Self, crate::error::Error> {
        let mut config = AppConfig::default();

        // 用户级配置（$HOME/.config/hearthbuddy/config.json 或自定义路径）
        let home_config = std::env::var("HB_CONFIG_DIR")
            .map(PathBuf::from)
            .or_else(|_| {
                std::env::var("HOME").map(|h| {
                    PathBuf::from(h)
                        .join(".config")
                        .join("hearthbuddy")
                        .join("config.json")
                })
            })
            .or_else(|_| {
                std::env::var("USERPROFILE").map(|h| {
                    PathBuf::from(h)
                        .join(".config")
                        .join("hearthbuddy")
                        .join("config.json")
                })
            });
        if let Ok(path) = home_config {
            if path.exists() {
                let user: AppConfig = AppConfig::from_file(&path)?;
                config.merge(user);
            }
        }

        // 项目级配置
        let local_config = PathBuf::from("hb-config.json");
        if local_config.exists() {
            let local: AppConfig = AppConfig::from_file(&local_config)?;
            config.merge(local);
        }

        Ok(config)
    }

    /// 合并另一个配置（非默认值覆盖）
    fn merge(&mut self, other: AppConfig) {
        // 简单字段覆盖
        if other.log_level != AppConfig::default().log_level {
            self.log_level = other.log_level;
        }
        if other.log_file.is_some() {
            self.log_file = other.log_file;
        }
        if other.hearthstone_process != AppConfig::default().hearthstone_process {
            self.hearthstone_process = other.hearthstone_process;
        }
        if other.poll_interval_ms != AppConfig::default().poll_interval_ms {
            self.poll_interval_ms = other.poll_interval_ms;
        }
        // 嵌套结构暂不深度合并
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.log_level, "info");
        assert_eq!(config.hearthstone_process, "Hearthstone");
        assert_eq!(config.ai.max_depth, 12);
    }

    #[test]
    fn test_config_serde_roundtrip() {
        let config = AppConfig::default();
        let json = serde_json::to_string_pretty(&config).unwrap();
        let deserialized: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.log_level, config.log_level);
    }
}
