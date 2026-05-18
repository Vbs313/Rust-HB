//! 任务管理器
//!
//! 对应 C# 版的 TaskManager.cs
//! 管理 Bot 的生命周期任务，使用 tokio 协程实现

use tokio::sync::mpsc;
use std::sync::Arc;
use parking_lot::Mutex;

/// 任务类型
pub enum Task {
    Gameplay,
    Dialog,
    Idle,
    Stop,
}

/// 任务管理器
pub struct TaskManager {
    sender: mpsc::Sender<Task>,
    running: Arc<Mutex<bool>>,
}

impl TaskManager {
    pub fn new() -> Self {
        let (tx, mut rx) = mpsc::channel::<Task>(100);
        let running = Arc::new(Mutex::new(false));

        let r = running.clone();
        tokio::spawn(async move {
            while let Some(task) = rx.recv().await {
                match task {
                    Task::Gameplay => {
                        tracing::debug!("Task: Gameplay");
                        // TODO: Bot main loop
                    }
                    Task::Dialog => {
                        tracing::debug!("Task: Dialog");
                        // TODO: Dialog handling
                    }
                    Task::Idle => {
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                    Task::Stop => break,
                }
            }
            *r.lock() = false;
        });

        Self { sender: tx, running }
    }

    pub async fn submit(&self, task: Task) {
        let _ = self.sender.send(task).await;
    }

    pub fn is_running(&self) -> bool {
        *self.running.lock()
    }
}
