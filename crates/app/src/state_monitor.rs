//! StateMonitor — 游戏状态监视器
//!
//! 后台任务持续读取游戏状态，通过 watch channel 广播给主循环。
//! 自动处理重连和退避，通过 ConnectionStatus 追踪连接状态。
//! 使用 IpcClientOps trait 支持测试 mock。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use hb_ipc::GameStateData;
use tokio::sync::watch;

use crate::ipc_client_wrapper::IpcClientOps;

/// 连接状态
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    /// 已连接，正常读取中
    Connected,
    /// 连接断开，正在尝试重连（第 N 次）
    Disconnected { retry_count: u32 },
}

/// 状态监视器 — 持续读取游戏状态并广播
pub struct StateMonitor {
    cancel: Arc<AtomicBool>,
    ipc: Arc<dyn IpcClientOps>,
    state_tx: watch::Sender<Option<GameStateData>>,
    status_tx: watch::Sender<ConnectionStatus>,
    poll_interval: Duration,
    max_backoff: Duration,
}

impl StateMonitor {
    /// 启动状态监视器
    ///
    /// - `cancel`: 关闭信号
    /// - `ipc`: IPC 客户端（支持 mock）
    /// - `poll_interval`: 成功时的轮询间隔（默认 50ms）
    /// - `max_backoff`: 重连最大退避间隔（默认 5s）
    ///
    /// 返回 `(state_rx, status_rx)` — 状态流和连接状态流
    pub fn spawn(
        cancel: Arc<AtomicBool>,
        ipc: Arc<dyn IpcClientOps>,
        poll_interval: Duration,
        max_backoff: Duration,
    ) -> (watch::Receiver<Option<GameStateData>>, watch::Receiver<ConnectionStatus>) {
        let (state_tx, state_rx) = watch::channel(None);
        let (status_tx, status_rx) = watch::channel(ConnectionStatus::Connected);
        let monitor = Self {
            cancel,
            ipc,
            state_tx,
            status_tx,
            poll_interval,
            max_backoff,
        };
        tokio::spawn(monitor.run());
        (state_rx, status_rx)
    }

    /// 使用默认参数的 spawn（poll_interval=50ms, max_backoff=5s）
    pub fn spawn_default(
        cancel: Arc<AtomicBool>,
        ipc: Arc<dyn IpcClientOps>,
    ) -> (watch::Receiver<Option<GameStateData>>, watch::Receiver<ConnectionStatus>) {
        Self::spawn(
            cancel,
            ipc,
            Duration::from_millis(50),
            Duration::from_secs(5),
        )
    }

    /// 运行主循环
    async fn run(self) {
        tracing::info!("StateMonitor started (interval={:?})", self.poll_interval);
        let mut retry_count: u32 = 0;

        while !self.cancel.load(Ordering::SeqCst) {
            // 计算本次延迟（退避算法：成功 = 固定间隔，失败 = 指数退避 + 上限）
            let delay = if retry_count == 0 {
                self.poll_interval
            } else {
                // 指数退避：50ms * 2^(retry_count-1)，上限 max_backoff
                let ms = self.poll_interval.as_millis() as u64;
                let backoff_ms = (ms * 2u64.pow(retry_count - 1)).min(self.max_backoff.as_millis() as u64);
                Duration::from_millis(backoff_ms)
            };
            tokio::time::sleep(delay).await;
            if self.cancel.load(Ordering::SeqCst) { break; }

            match self.ipc.get_game_state().await {
                Ok(state) => {
                    if retry_count > 0 {
                        tracing::info!("StateMonitor: Reconnected after {retry_count} retries");
                        retry_count = 0;
                        self.status_tx.send_replace(ConnectionStatus::Connected);
                    }
                    self.state_tx.send_replace(Some(state));
                }
                Err(e) => {
                    retry_count += 1;
                    if retry_count <= 3 {
                        tracing::warn!("StateMonitor: IPC error ({retry_count}): {e}");
                    } else if retry_count.is_power_of_two() {
                        // 每 2^n 次才打日志，避免刷屏
                        tracing::warn!("StateMonitor: IPC error x{retry_count}, still retrying... ({e})");
                    }
                    self.state_tx.send_replace(None);
                    self.status_tx.send_replace(ConnectionStatus::Disconnected { retry_count });

                    // 尝试重连
                    if self.ipc.reconnect(Duration::from_secs(5)).await.is_ok() {
                        tracing::info!("StateMonitor: Reconnect call succeeded");
                    }
                }
            }
        }

        let final_status = if retry_count > 0 {
            format!("disconnected after {retry_count} retries")
        } else {
            "connected".into()
        };
        tracing::info!("StateMonitor stopped ({final_status})");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc_client_wrapper::mock::MockIpcClient;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_state_monitor_publishes_state() {
        let cancel = Arc::new(AtomicBool::new(false));
        let mock = Arc::new(MockIpcClient::new());
        let (mut state_rx, _) = StateMonitor::spawn(
            cancel.clone(),
            mock.clone(),
            Duration::from_millis(10),
            Duration::from_secs(1),
        );

        // 等待一次状态发布
        tokio::time::timeout(Duration::from_millis(500), state_rx.changed())
            .await
            .expect("state_rx should receive update");

        let state = state_rx.borrow_and_update().clone();
        assert!(state.is_some());
        assert!(mock.call_count.load(Ordering::SeqCst) > 0);

        cancel.store(true, Ordering::SeqCst);
    }

    #[tokio::test]
    async fn test_state_monitor_disconnect_sends_none() {
        let cancel = Arc::new(AtomicBool::new(false));
        let mock = Arc::new(MockIpcClient::new());
        mock.should_fail.store(true, Ordering::SeqCst);

        let (mut state_rx, mut status_rx) = StateMonitor::spawn(
            cancel.clone(),
            mock.clone(),
            Duration::from_millis(10),
            Duration::from_millis(50),
        );

        // 等待状态变成 None
        tokio::time::timeout(Duration::from_millis(500), state_rx.changed())
            .await
            .expect("state_rx should change");
        let state = state_rx.borrow_and_update().clone();
        assert!(state.is_none());

        // 验证连接状态
        tokio::time::timeout(Duration::from_millis(500), status_rx.changed())
            .await
            .expect("status_rx should change");
        let status = status_rx.borrow_and_update().clone();
        assert_eq!(status, ConnectionStatus::Disconnected { retry_count: 1 });

        cancel.store(true, Ordering::SeqCst);
    }

    #[tokio::test]
    async fn test_state_monitor_reconnect_recovers() {
        let cancel = Arc::new(AtomicBool::new(false));
        let mock = Arc::new(MockIpcClient::new());

        // 先失败，再恢复
        mock.should_fail.store(true, Ordering::SeqCst);

        let (mut state_rx, mut status_rx) = StateMonitor::spawn(
            cancel.clone(),
            mock.clone(),
            Duration::from_millis(10),
            Duration::from_millis(50),
        );

        // 等待断开
        tokio::time::timeout(Duration::from_millis(500), status_rx.changed())
            .await
            .expect("status_rx should change to disconnected");
        assert!(matches!(
            status_rx.borrow_and_update().clone(),
            ConnectionStatus::Disconnected { .. }
        ));

        // 恢复连接 (reconnect 会将 should_fail 置 false)
        let _ = mock.reconnect(Duration::from_secs(0)).await;

        // 等待恢复
        tokio::time::timeout(Duration::from_millis(500), status_rx.changed())
            .await
            .expect("status_rx should change to connected");

        // 此时应在状态更新后恢复为 Connected
        // 注意：reconnect 后需要一次成功的 get_game_state 才会切换状态
        // 等状态发布
        tokio::time::timeout(Duration::from_millis(500), state_rx.changed())
            .await
            .ok();
        let state = state_rx.borrow_and_update().clone();
        assert!(state.is_some(), "After reconnect, state should be Some");

        cancel.store(true, Ordering::SeqCst);
    }
}
