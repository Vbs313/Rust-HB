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

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Plugin, BotError};
    use std::sync::Arc;

    struct TestPlugin;

    impl Plugin for TestPlugin {
        fn name(&self) -> &'static str { "TestPlugin" }
        fn author(&self) -> &'static str { "Tester" }
        fn description(&self) -> &'static str { "Test plugin" }
        fn initialize(&self) -> Result<(), BotError> { Ok(()) }
        fn deinitialize(&self) -> Result<(), BotError> { Ok(()) }
        fn is_enabled(&self) -> bool { true }
    }

    #[test]
    fn test_plugin_manager_new() {
        let mgr = PluginManager::new();
        assert!(mgr.list().is_empty());
    }

    #[test]
    fn test_plugin_register_and_list() {
        let mgr = PluginManager::new();
        mgr.register(Arc::new(TestPlugin));
        let list = mgr.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0], "TestPlugin");
    }

    #[test]
    fn test_plugin_get() {
        let mgr = PluginManager::new();
        mgr.register(Arc::new(TestPlugin));
        let p = mgr.get("TestPlugin");
        assert!(p.is_some());
        assert_eq!(p.unwrap().name(), "TestPlugin");
    }

    #[test]
    fn test_plugin_initialize_all() {
        let mgr = PluginManager::new();
        mgr.register(Arc::new(TestPlugin));
        let results = mgr.initialize_all();
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
    }

    #[test]
    fn test_plugin_deinitialize_all_runs() {
        let mgr = PluginManager::new();
        mgr.register(Arc::new(TestPlugin));
        // Should not panic
        mgr.deinitialize_all();
    }
}
