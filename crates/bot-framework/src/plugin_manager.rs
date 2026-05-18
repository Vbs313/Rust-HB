//! 插件管理器
//!
//! 对应 C# 版的 PluginManager.cs

use std::sync::Arc;
use dashmap::DashMap;
use crate::{Plugin, BotError};

/// 插件管理器
pub struct PluginManager {
    plugins: DashMap<String, Arc<dyn Plugin>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self { plugins: DashMap::new() }
    }

    pub fn register(&self, plugin: Arc<dyn Plugin>) {
        self.plugins.insert(plugin.name().to_string(), plugin);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Plugin>> {
        self.plugins.get(name).map(|r| r.clone())
    }

    pub fn list(&self) -> Vec<String> {
        self.plugins.iter().map(|r| r.key().clone()).collect()
    }

    pub fn initialize_all(&self) -> Vec<Result<(), BotError>> {
        let mut results = Vec::new();
        for entry in self.plugins.iter() {
            results.push(entry.value().initialize());
        }
        results
    }

    pub fn deinitialize_all(&self) {
        for entry in self.plugins.iter() {
            if let Err(e) = entry.value().deinitialize() {
                tracing::warn!("Plugin deinit error: {e}");
            }
        }
    }
}
