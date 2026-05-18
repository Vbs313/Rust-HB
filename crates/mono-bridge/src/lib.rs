//! # hb-mono-bridge
//!
//! Mono 运行时桥接库
//!
//! 替代 C# 版 GreyMagic.dll，实现跨进程读取 Unity/Mono 运行时内存。
//! 目标进程：Hearthstone.exe (32-bit x86)
//!
//! ## 架构
//!
//! ```text
//! hb-core::win32::ProcessHandle (OpenProcess + ReadProcessMemory)
//!     ↓
//! MonoRuntime (定位 Mono 运行时基址、域、映像)
//!     ↓
//! MonoImage (程序集元数据: 类/方法/字段表)
//!     ↓
//! MonoClass (运行时类实例读取: 字段值/方法调用)
//!     ↓
//! 游戏对象映射 (Actor, Card, Entity, ...)
//! ```

#![allow(non_snake_case, dead_code)]

pub mod mappers;
pub mod mono_class;
pub mod mono_image;
pub mod mono_runtime;

use hb_core::win32::ProcessHandle;

/// Mono 桥接主入口
pub struct MonoBridge {
    process: ProcessHandle,
    runtime: mono_runtime::MonoRuntime,
}

impl MonoBridge {
    /// 连接到炉石进程的 Mono 运行时
    pub fn attach(process: ProcessHandle) -> Result<Self, BridgeError> {
        let runtime = mono_runtime::MonoRuntime::find(&process)?;
        Ok(Self { process, runtime })
    }

    /// 获取进程句柄引用
    pub fn process(&self) -> &ProcessHandle {
        &self.process
    }

    /// 获取 Mono 运行时引用
    pub fn runtime(&self) -> &mono_runtime::MonoRuntime {
        &self.runtime
    }
}

/// Mono 桥接错误类型
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("Mono runtime not found in target process")]
    RuntimeNotFound,
    #[error("Mono domain not found")]
    DomainNotFound,
    #[error("Mono image not found: {0}")]
    ImageNotFound(String),
    #[error("Class not found: {0}")]
    ClassNotFound(String),
    #[error("Memory read failed: {0}")]
    Memory(String),
    #[error("Core error: {0}")]
    Core(#[from] hb_core::error::Error),
}
