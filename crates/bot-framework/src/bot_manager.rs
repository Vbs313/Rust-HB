//! Bot 管理器
//!
//! 对应 C# 版的 BotManager.cs

// unused
use std::sync::Arc;
use dashmap::DashMap;
use crate::{Bot, BotError};

/// Bot 管理器
pub struct BotManager {
    bots: DashMap<String, Arc<dyn Bot>>,
    active_bot: Option<String>,
    running: bool,
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
        let bot = self.get(name)
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
