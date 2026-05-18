//! 策略管理器
//!
//! 对应 C# 版的 RoutineManager.cs

use std::sync::Arc;
use dashmap::DashMap;
use crate::{Routine, BotError};

pub struct RoutineManager {
    routines: DashMap<String, Arc<dyn Routine>>,
    active_routine: Option<String>,
}

impl RoutineManager {
    pub fn new() -> Self {
        Self { routines: DashMap::new(), active_routine: None }
    }

    pub fn register(&self, routine: Arc<dyn Routine>) {
        self.routines.insert(routine.name().to_string(), routine);
    }

    pub fn active(&self) -> Option<Arc<dyn Routine>> {
        self.active_routine.as_ref()
            .and_then(|name| self.routines.get(name).map(|r| r.clone()))
    }

    pub fn set_active(&mut self, name: &str) -> Result<(), BotError> {
        if self.routines.contains_key(name) {
            self.active_routine = Some(name.to_string());
            Ok(())
        } else {
            Err(BotError::RoutineNotFound(name.to_string()))
        }
    }
}
