//! Bot 管理器
//!
//! 对应 C# 版的 BotManager.cs

// unused
use crate::{Bot, BotError};
use dashmap::DashMap;
use std::sync::Arc;

/// Bot 管理器
pub struct BotManager {
    bots: DashMap<String, Arc<dyn Bot>>,
    active_bot: Option<String>,
    running: bool,
}

impl Default for BotManager {
    fn default() -> Self {
        Self::new()
    }
}

impl BotManager {
    pub fn new() -> Self {
        Self {
            bots: DashMap::new(),
            active_bot: None,
            running: false,
        }
    }

    /// 注册 Bot
    pub fn register(&self, bot: Arc<dyn Bot>) {
        self.bots.insert(bot.name().to_string(), bot);
        tracing::info!("Bot registered");
    }

    /// 获取 Bot
    pub fn get(&self, name: &str) -> Option<Arc<dyn Bot>> {
        self.bots.get(name).map(|r| r.clone())
    }

    /// 列表
    pub fn list(&self) -> Vec<String> {
        self.bots.iter().map(|r| r.key().clone()).collect()
    }

    /// 启动指定 Bot
    pub fn start(&mut self, name: &str) -> Result<(), BotError> {
        let bot = self
            .get(name)
            .ok_or_else(|| BotError::BotNotFound(name.to_string()))?;

        if self.running {
            return Err(BotError::AlreadyRunning);
        }

        bot.start()?;
        self.active_bot = Some(name.to_string());
        self.running = true;
        tracing::info!("Bot '{name}' started");
        Ok(())
    }

    /// 停止当前 Bot
    pub fn stop(&mut self) -> Result<(), BotError> {
        if let Some(ref name) = self.active_bot {
            if let Some(bot) = self.get(name) {
                bot.stop()?;
            }
        }
        self.running = false;
        self.active_bot = None;
        tracing::info!("Bot stopped");
        Ok(())
    }

    /// 脉冲
    pub fn pulse(&self) -> Result<(), BotError> {
        if let Some(ref name) = self.active_bot {
            if let Some(bot) = self.get(name) {
                bot.pulse()?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bot, BotError};
    use std::sync::Arc;

    struct TestBot;

    impl Bot for TestBot {
        fn name(&self) -> &'static str {
            "TestBot"
        }
        fn author(&self) -> &'static str {
            "Tester"
        }
        fn description(&self) -> &'static str {
            "Test bot for unit tests"
        }
        fn start(&self) -> Result<(), BotError> {
            Ok(())
        }
        fn stop(&self) -> Result<(), BotError> {
            Ok(())
        }
        fn pulse(&self) -> Result<(), BotError> {
            Ok(())
        }
        fn is_running(&self) -> bool {
            true
        }
    }

    #[test]
    fn test_bot_manager_new() {
        let mgr = BotManager::new();
        assert!(mgr.list().is_empty());
    }

    #[test]
    fn test_bot_register_and_list() {
        let mgr = BotManager::new();
        mgr.register(Arc::new(TestBot));
        let list = mgr.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0], "TestBot");
    }

    #[test]
    fn test_bot_get() {
        let mgr = BotManager::new();
        mgr.register(Arc::new(TestBot));
        let bot = mgr.get("TestBot");
        assert!(bot.is_some());
        assert_eq!(bot.unwrap().name(), "TestBot");

        let missing = mgr.get("NonExistent");
        assert!(missing.is_none());
    }

    #[test]
    fn test_bot_manager_start_stop() {
        let mut mgr = BotManager::new();
        mgr.register(Arc::new(TestBot));

        assert!(mgr.start("TestBot").is_ok());
        assert!(
            mgr.start("TestBot").is_err(),
            "Should error: already running"
        );

        assert!(mgr.stop().is_ok());
    }

    #[test]
    fn test_bot_manager_start_missing() {
        let mut mgr = BotManager::new();
        let result = mgr.start("MissingBot");
        assert!(result.is_err());
        match result {
            Err(BotError::BotNotFound(_)) => {}
            _ => panic!("Expected BotNotFound"),
        }
    }

    #[test]
    fn test_bot_pulse_without_start() {
        let mgr = BotManager::new();
        // No active bot, pulse should be a no-op
        assert!(mgr.pulse().is_ok());
    }
}
