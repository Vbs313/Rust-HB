//! IPC 通信层 — 与 HsMod 通过命名管道通信

use serde::{Serialize, Deserialize};
use std::io::{Read, Write};
use std::time::Duration;

pub const PIPE_NAME: &str = r"\\.\pipe\Hearthbuddy_IPC";

// ===== 协议类型 =====

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcRequest {
    #[serde(rename = "GetGameState")] GetGameState { seq: u32 },
    #[serde(rename = "PerformAction")] PerformAction { seq: u32, action: ActionCommand },
    #[serde(rename = "Ping")] Ping { seq: u32, timestamp: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionCommand {
    pub action_type: String, pub hand_index: Option<u32>,
    pub target_id: Option<i32>, pub position: Option<u32>, pub choice: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcResponse {
    #[serde(rename = "GameState")] GameState { seq: u32, state: GameStateData },
    #[serde(rename = "ActionResult")] ActionResult { seq: u32, success: bool, error: Option<String> },
    #[serde(rename = "Pong")] Pong { seq: u32, timestamp: u64 },
    #[serde(rename = "Error")] Error { seq: u32, message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStateData {
    pub scene: String, pub is_own_turn: bool, pub turn: u32,
    pub own_mana: u32, pub own_max_mana: u32,
    pub own_hero: EntityData, pub enemy_hero: EntityData,
    pub own_hand: Vec<CardData>,
    pub own_minions: Vec<EntityData>, pub enemy_minions: Vec<EntityData>,
    pub own_hand_count: u32, pub enemy_hand_count: u32,
    pub own_deck_count: u32, pub enemy_deck_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityData {
    pub entity_id: i32, pub card_id: String, pub health: i32, pub attack: i32,
    pub armor: i32, pub has_taunt: bool, pub has_divine_shield: bool,
    pub has_stealth: bool, pub has_poisonous: bool, pub has_lifesteal: bool,
    pub is_exhausted: bool, pub num_attacks: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardData {
    pub entity_id: i32, pub card_id: String, pub cost: i32,
    pub attack: i32, pub health: i32, pub card_type: String,
}

// ===== IPC 客户端 =====

#[derive(Debug, thiserror::Error)]
pub enum IpcError {
    #[error("Connect: {0}")] ConnectionFailed(String),
    #[error("IO: {0}")] Io(#[from] std::io::Error),
    #[error("JSON: {0}")] Json(#[from] serde_json::Error),
    #[error("Disconnected")] Disconnected,
}

pub struct IpcClient {
    seq: u32,
    pipe: Pipe,
    recv_buf: Vec<u8>,
}

struct Pipe { handle: isize }
unsafe impl Send for Pipe {}

impl Read for Pipe {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        unsafe {
            let mut n: u32 = 0;
            if ReadFile(self.handle as *mut _, buf.as_mut_ptr() as *mut _, buf.len() as u32, &mut n, std::ptr::null_mut()) != 0 {
                Ok(n as usize)
            } else { Err(std::io::Error::last_os_error()) }
        }
    }
}

impl Write for Pipe {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        unsafe {
            let mut n: u32 = 0;
            if WriteFile(self.handle as *mut _, buf.as_ptr() as *const _, buf.len() as u32, &mut n, std::ptr::null_mut()) != 0 {
                Ok(n as usize)
            } else { Err(std::io::Error::last_os_error()) }
        }
    }
    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}

impl Drop for Pipe { fn drop(&mut self) { unsafe { CloseHandle(self.handle as *mut _); } } }

impl IpcClient {
    pub fn connect(timeout: Duration) -> Result<Self, IpcError> {
        use std::ffi::OsStr; use std::os::windows::ffi::OsStrExt;
        let p: Vec<u16> = OsStr::new(PIPE_NAME).encode_wide().chain(Some(0)).collect();
        let n = (timeout.as_millis() / 200) as u32;
        for _ in 0..n {
            unsafe {
                let h = CreateFileW(p.as_ptr(), 0xC0000000, 0, std::ptr::null_mut(), 3, 0, std::ptr::null_mut());
                if h != -1isize { SetNamedPipeHandleState(h, &1u32, std::ptr::null(), std::ptr::null());
                    return Ok(Self { seq: 0, pipe: Pipe { handle: h }, recv_buf: Vec::with_capacity(4096) }); }
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        Err(IpcError::ConnectionFailed(PIPE_NAME.into()))
    }

    /// 发送请求并读取响应（按行分割）
    pub fn request(&mut self, req: IpcRequest) -> Result<IpcResponse, IpcError> {
        self.seq += 1;
        let json = serde_json::to_string(&req)?;
        self.pipe.write_all(json.as_bytes())?;
        self.pipe.write_all(b"\n")?;
        self.pipe.flush()?;

        self.recv_buf.clear();
        loop {
            let mut b = [0u8; 1];
            match self.pipe.read(&mut b) {
                Ok(0) => return Err(IpcError::Disconnected),
                Ok(_) => { if b[0] == b'\n' { break; } self.recv_buf.push(b[0]); }
                Err(e) => return Err(IpcError::Io(e)),
            }
        }
        Ok(serde_json::from_slice(&self.recv_buf)?)
    }

    pub fn get_game_state(&mut self) -> Result<GameStateData, IpcError> {
        match self.request(IpcRequest::GetGameState { seq: 0 })? {
            IpcResponse::GameState { state, .. } => Ok(state),
            IpcResponse::Error { message, .. } => Err(IpcError::ConnectionFailed(message)),
            _ => Err(IpcError::ConnectionFailed("bad response".into())),
        }
    }

    pub fn perform_action(&mut self, action: ActionCommand) -> Result<bool, IpcError> {
        match self.request(IpcRequest::PerformAction { seq: 0, action })? {
            IpcResponse::ActionResult { success, .. } => Ok(success),
            IpcResponse::Error { message, .. } => Err(IpcError::ConnectionFailed(message)),
            _ => Err(IpcError::ConnectionFailed("bad response".into())),
        }
    }
}

extern "system" {
    fn CreateFileW(p: *const u16, a: u32, s: u32, sa: *mut u8, cd: u32, f: u32, t: *mut u8) -> isize;
    fn SetNamedPipeHandleState(h: isize, m: *const u32, a: *const u32, b: *const u32) -> i32;
    fn ReadFile(h: *mut u8, b: *mut u8, s: u32, r: *mut u32, o: *mut u8) -> i32;
    fn WriteFile(h: *mut u8, b: *const u8, s: u32, w: *mut u32, o: *mut u8) -> i32;
    fn CloseHandle(h: *mut u8) -> i32;
}
