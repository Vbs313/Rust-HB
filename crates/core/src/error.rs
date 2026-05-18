//! 错误类型定义

use std::fmt;

/// 全局错误类型
#[derive(Debug)]
pub enum Error {
    /// IO 错误
    Io(std::io::Error),
    /// 配置错误
    Config(String),
    /// 序列化错误
    Serde(serde_json::Error),
    /// 进程未找到
    ProcessNotFound(String),
    /// 内存读写错误
    Memory(String),
    /// Mono 交互错误
    Mono(String),
    /// AI 搜索错误
    Ai(String),
    /// 认证错误
    Auth(String),
    /// 插件错误
    Plugin(String),
    /// 通用运行时错误
    Runtime(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "IO error: {e}"),
            Error::Config(msg) => write!(f, "Config error: {msg}"),
            Error::Serde(e) => write!(f, "Serde error: {e}"),
            Error::ProcessNotFound(name) => write!(f, "Process not found: {name}"),
            Error::Memory(msg) => write!(f, "Memory error: {msg}"),
            Error::Mono(msg) => write!(f, "Mono error: {msg}"),
            Error::Ai(msg) => write!(f, "AI error: {msg}"),
            Error::Auth(msg) => write!(f, "Auth error: {msg}"),
            Error::Plugin(msg) => write!(f, "Plugin error: {msg}"),
            Error::Runtime(msg) => write!(f, "Runtime error: {msg}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::Serde(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Serde(e)
    }
}

impl From<toml::de::Error> for Error {
    fn from(e: toml::de::Error) -> Self {
        Error::Config(e.to_string())
    }
}

/// 便捷 Result 类型
pub type Result<T> = std::result::Result<T, Error>;
