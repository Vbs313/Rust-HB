//! Supervisor — 生命周期管理
//!
//! 持有关闭信号，处理 Ctrl+C 信号，
//! 确保所有子任务和插件在退出时被正确清理。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Notify;

/// 监督者：管理应用生命周期
pub struct Supervisor {
    shutting_down: Arc<AtomicBool>,
    notify: Arc<Notify>,
    handler_installed: Arc<AtomicBool>,
}

impl Supervisor {
    /// 创建 Supervisor
    ///
    /// Ctrl+C 信号处理器在第一次调用 is_shutting_down() 时惰性注册，
    /// 避免在测试中因无 tokio runtime 而 panic。
    pub fn new() -> Self {
        Self {
            shutting_down: Arc::new(AtomicBool::new(false)),
            notify: Arc::new(Notify::new()),
            handler_installed: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 惰性注册 Ctrl+C 处理器（最多一次）
    fn ensure_handler(&self) {
        if self.handler_installed.load(Ordering::SeqCst) {
            return;
        }
        // 尝试原子地标记已安装
        if self.handler_installed.swap(true, Ordering::SeqCst) {
            return; // 另一个线程已经安装了
        }
        // 只有在 tokio runtime 中才安装
        if tokio::runtime::Handle::try_current().is_err() {
            return;
        }
        let sd = self.shutting_down.clone();
        let n = self.notify.clone();
        tokio::spawn(async move {
            match tokio::signal::ctrl_c().await {
                Ok(()) => {
                    tracing::info!("Ctrl+C received, shutting down gracefully...");
                    sd.store(true, Ordering::SeqCst);
                    n.notify_waiters();
                }
                Err(e) => {
                    tracing::error!("Failed to install Ctrl+C handler: {e}");
                }
            }
        });
    }

    /// 是否正在关闭
    pub fn is_shutting_down(&self) -> bool {
        self.ensure_handler();
        self.shutting_down.load(Ordering::SeqCst)
    }

    /// 等待关闭信号（Ctrl+C 或显式调用 shutdown）
    pub async fn wait_for_shutdown(&self) {
        self.ensure_handler();
        if !self.is_shutting_down() {
            self.notify.notified().await;
        }
    }

    /// 显式触发关闭（供测试或内部使用）
    pub fn shutdown(&self) {
        self.shutting_down.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    /// 获取关闭标志（用于传递给子任务）
    pub fn shutdown_flag(&self) -> Arc<AtomicBool> {
        self.shutting_down.clone()
    }
}



impl Default for Supervisor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_supervisor_new_not_cancelled() {
        let sv = Supervisor::new();
        assert!(!sv.is_shutting_down());
    }

    #[tokio::test]
    async fn test_supervisor_shutdown() {
        let sv = Supervisor::new();
        sv.shutdown();
        assert!(sv.is_shutting_down());
    }

    #[tokio::test]
    async fn test_supervisor_wait_returns_immediately_if_shutdown() {
        let sv = Supervisor::new();
        sv.shutdown();
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            sv.wait_for_shutdown(),
        ).await.expect("wait_for_shutdown should return immediately");
    }

    #[tokio::test]
    async fn test_supervisor_wait_blocks_until_shutdown() {
        let sv = Arc::new(Supervisor::new());
        let sv_clone = sv.clone();

        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            sv_clone.shutdown();
        });

        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            sv.wait_for_shutdown(),
        ).await.expect("wait_for_shutdown should return after shutdown");
    }
}
