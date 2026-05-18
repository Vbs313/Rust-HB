//! Mono 程序集映像
//!
//! 对应 C# 的 MonoImage / Assembly 概念。
//! 提供遍历类表、方法表、字段表的能力。

use crate::BridgeError;
use hb_core::win32::ProcessHandle;

/// Mono 程序集映像
#[derive(Debug)]
pub struct MonoImage {
    process: ProcessHandle,
    /// 映像对象在目标进程中的地址
    pub address: usize,
    /// 映像名称
    pub name: String,
}

impl MonoImage {
    /// 从地址构造
    pub fn new(process: ProcessHandle, address: usize, name: String) -> Self {
        Self {
            process,
            address,
            name,
        }
    }

    /// 遍历映像中的所有类
    pub fn enumerate_classes(&self) -> Result<Vec<MonoClassInfo>, BridgeError> {
        let classes = Vec::new();

        // 读取 MonoImage.class_cache 或 class_table
        // 结构: MonoImage → class_cache (MonoClass 数组)
        // 偏移量需根据 Mono 版本确定

        tracing::debug!("Enumerating classes in image: {}", self.name);
        Ok(classes)
    }

    /// 按名称查找类
    pub fn find_class(&self, _namespace: &str, _name: &str) -> Result<MonoClassInfo, BridgeError> {
        // 遍历 class_table，匹配 namespace + name
        Err(BridgeError::ClassNotFound(format!("{_namespace}.{_name}")))
    }
}

/// Mono 类信息（轻量）
#[derive(Debug, Clone)]
pub struct MonoClassInfo {
    /// MonoClass 对象地址
    pub address: usize,
    /// 命名空间
    pub namespace: String,
    /// 类名
    pub name: String,
    /// 父类地址
    pub parent_addr: usize,
    /// 实例大小
    pub instance_size: u32,
}
