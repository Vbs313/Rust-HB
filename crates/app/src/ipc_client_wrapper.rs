//! AsyncIpcClient — 异步 IPC 客户端包装器
//!
//! 将原生的同步 IpcClient 包装为 async 版本，
//! 通过 Arc<Mutex> 实现共享访问，spawn_blocking 避免阻塞 tokio runtime。
//!
//! 提供 IpcClientTrait 接口以支持测试 mock。

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use async_trait::async_trait;
use hb_ipc::{ActionCommand, GameStateData, IpcClient, IpcError};

/// IPC 客户端操作接口（支持 mock 测试）
#[async_trait]
pub trait IpcClientOps: Send + Sync {
    /// 读取游戏状态
    async fn get_game_state(&self) -> Result<GameStateData, IpcError>;
    /// 执行动作
    async fn perform_action(&self, action: ActionCommand) -> Result<bool, IpcError>;
    /// 重连
    async fn reconnect(&self, timeout: Duration) -> Result<(), IpcError>;
}

/// 获取 Mutex 锁（忽略 Poison 状态，继续使用）
fn lock_ipc(m: &Arc<Mutex<IpcClient>>) -> Result<MutexGuard<'_, IpcClient>, IpcError> {
    m.lock().map_err(|_| IpcError::ConnectionFailed("mutex poisoned".into()))
}

/// 异步 IPC 客户端
///
/// 内部使用 Arc<Mutex<IpcClient>> 实现 &self 方法的共享访问，
/// 所有阻塞操作通过 tokio::task::spawn_blocking 转移到阻塞线程池。
#[derive(Clone)]
pub struct AsyncIpcClient {
    inner: Arc<Mutex<IpcClient>>,
}

#[async_trait]
impl IpcClientOps for AsyncIpcClient {
    /// 异步连接（同步 connect 包装）
    async fn get_game_state(&self) -> Result<GameStateData, IpcError> {
        let inner = self.inner.clone();
        let ret = tokio::task::spawn_blocking(move || {
            let mut guard = lock_ipc(&inner)?;
            guard.get_game_state()
        })
        .await;
        ret.map_err(|e| IpcError::ConnectionFailed(e.to_string()))?
    }

    /// 异步执行动作
    async fn perform_action(&self, action: ActionCommand) -> Result<bool, IpcError> {
        let inner = self.inner.clone();
        let ret = tokio::task::spawn_blocking(move || {
            let mut guard = lock_ipc(&inner)?;
            guard.perform_action(action)
        })
        .await;
        ret.map_err(|e| IpcError::ConnectionFailed(e.to_string()))?
    }

    /// 异步重连（创建新连接替换内部状态）
    async fn reconnect(&self, timeout: Duration) -> Result<(), IpcError> {
        let ret = tokio::task::spawn_blocking(move || IpcClient::connect(timeout)).await;
        let new_ipc = ret.map_err(|e| IpcError::ConnectionFailed(e.to_string()))?;
        let mut guard = lock_ipc(&self.inner)?;
        *guard = new_ipc?;
        Ok(())
    }
}

impl AsyncIpcClient {
    /// 异步连接（同步 connect 包装）
    pub async fn connect(timeout: Duration) -> Result<Self, IpcError> {
        let ret = tokio::task::spawn_blocking(move || IpcClient::connect(timeout)).await;
        let ipc = ret.map_err(|e| IpcError::ConnectionFailed(e.to_string()))?;
        Ok(Self {
            inner: Arc::new(Mutex::new(ipc?)),
        })
    }
}

// ===== Mock IPC Client for Testing =====

#[cfg(test)]
pub mod mock {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;
    use hb_ipc::{ActionCommand, GameStateData, EntityData, IpcError};

    use super::IpcClientOps;

    /// 模拟 IPC 客户端 — 返回可配置的假数据
    #[derive(Clone)]
    pub struct MockIpcClient {
        pub state: Arc<std::sync::Mutex<GameStateData>>,
        pub call_count: Arc<AtomicUsize>,
        pub fail_count: Arc<AtomicUsize>,
        pub should_fail: Arc<std::sync::atomic::AtomicBool>,
    }

    impl MockIpcClient {
        pub fn new() -> Self {
            let state = GameStateData {
                scene: "Gameplay".into(),
                is_own_turn: true,
                turn: 5,
                own_mana: 8,
                own_max_mana: 10,
                own_hero: EntityData {
                    entity_id: 1, card_id: "HERO_01".into(),
                    health: 30, attack: 0, armor: 0,
                    has_taunt: false, has_divine_shield: false, has_stealth: false,
                    has_poisonous: false, has_lifesteal: false,
                    is_exhausted: false, num_attacks: 0,
                },
                enemy_hero: EntityData {
                    entity_id: 2, card_id: "HERO_02".into(),
                    health: 30, attack: 0, armor: 0,
                    has_taunt: false, has_divine_shield: false, has_stealth: false,
                    has_poisonous: false, has_lifesteal: false,
                    is_exhausted: false, num_attacks: 0,
                },
                own_hand: vec![], own_minions: vec![], enemy_minions: vec![],
                own_hand_count: 0, enemy_hand_count: 0,
                own_deck_count: 30, enemy_deck_count: 30,
            };
            Self {
                state: Arc::new(std::sync::Mutex::new(state)),
                call_count: Arc::new(AtomicUsize::new(0)),
                fail_count: Arc::new(AtomicUsize::new(0)),
                should_fail: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            }
        }

        /// 设置模拟返回的游戏状态
        pub fn set_state(&self, state: GameStateData) {
            *self.state.lock().unwrap() = state;
        }

        /// 切换为对手回合
        pub fn set_opponent_turn(&self) {
            let mut s = self.state.lock().unwrap();
            s.is_own_turn = false;
        }

        /// 切换为我方回合
        pub fn set_our_turn(&self) {
            let mut s = self.state.lock().unwrap();
            s.is_own_turn = true;
        }
    }

    #[async_trait]
    impl IpcClientOps for MockIpcClient {
        async fn get_game_state(&self) -> Result<GameStateData, IpcError> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            if self.should_fail.load(Ordering::SeqCst) {
                self.fail_count.fetch_add(1, Ordering::SeqCst);
                return Err(IpcError::Disconnected);
            }
            Ok(self.state.lock().unwrap().clone())
        }

        async fn perform_action(&self, _action: ActionCommand) -> Result<bool, IpcError> {
            Ok(true)
        }

        async fn reconnect(&self, _timeout: Duration) -> Result<(), IpcError> {
            self.should_fail.store(false, Ordering::SeqCst);
            Ok(())
        }
    }
}
