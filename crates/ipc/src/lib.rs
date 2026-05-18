//! IPC 通信层 — 与 HsMod 通过命名管道通信

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::time::Duration;

pub const PIPE_NAME: &str = r"\\.\pipe\Hearthbuddy_IPC";

// ===== 协议类型 =====

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcRequest {
    #[serde(rename = "GetGameState")]
    GetGameState { seq: u32 },
    #[serde(rename = "PerformAction")]
    PerformAction { seq: u32, action: ActionCommand },
    #[serde(rename = "Ping")]
    Ping { seq: u32, timestamp: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionCommand {
    pub action_type: String,
    pub hand_index: Option<u32>,
    pub target_id: Option<i32>,
    pub position: Option<u32>,
    pub choice: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcResponse {
    #[serde(rename = "GameState")]
    GameState { seq: u32, state: GameStateData },
    #[serde(rename = "ActionResult")]
    ActionResult {
        seq: u32,
        success: bool,
        error: Option<String>,
    },
    #[serde(rename = "Pong")]
    Pong { seq: u32, timestamp: u64 },
    #[serde(rename = "Error")]
    Error { seq: u32, message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStateData {
    pub scene: String,
    pub is_own_turn: bool,
    pub turn: u32,
    pub own_mana: u32,
    pub own_max_mana: u32,
    pub own_hero: EntityData,
    pub enemy_hero: EntityData,
    pub own_hand: Vec<CardData>,
    pub own_minions: Vec<EntityData>,
    pub enemy_minions: Vec<EntityData>,
    pub own_hand_count: u32,
    pub enemy_hand_count: u32,
    pub own_deck_count: u32,
    pub enemy_deck_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityData {
    pub entity_id: i32,
    pub card_id: String,
    pub health: i32,
    pub attack: i32,
    pub armor: i32,
    pub has_taunt: bool,
    pub has_divine_shield: bool,
    pub has_stealth: bool,
    pub has_poisonous: bool,
    pub has_lifesteal: bool,
    pub is_exhausted: bool,
    pub num_attacks: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardData {
    pub entity_id: i32,
    pub card_id: String,
    pub cost: i32,
    pub attack: i32,
    pub health: i32,
    pub card_type: String,
}

// ===== IPC 客户端 =====

#[derive(Debug, thiserror::Error)]
pub enum IpcError {
    #[error("Connect: {0}")]
    ConnectionFailed(String),
    #[error("IO: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Disconnected")]
    Disconnected,
}

pub struct IpcClient {
    seq: u32,
    pipe: Pipe,
    recv_buf: Vec<u8>,
}

struct Pipe {
    handle: isize,
}
unsafe impl Send for Pipe {}

impl Read for Pipe {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        unsafe {
            let mut n: u32 = 0;
            if ReadFile(
                self.handle as *mut _,
                buf.as_mut_ptr() as *mut _,
                buf.len() as u32,
                &mut n,
                std::ptr::null_mut(),
            ) != 0
            {
                Ok(n as usize)
            } else {
                Err(std::io::Error::last_os_error())
            }
        }
    }
}

impl Write for Pipe {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        unsafe {
            let mut n: u32 = 0;
            if WriteFile(
                self.handle as *mut _,
                buf.as_ptr() as *const _,
                buf.len() as u32,
                &mut n,
                std::ptr::null_mut(),
            ) != 0
            {
                Ok(n as usize)
            } else {
                Err(std::io::Error::last_os_error())
            }
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Drop for Pipe {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle as *mut _);
        }
    }
}

impl IpcClient {
    pub fn connect(timeout: Duration) -> Result<Self, IpcError> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        let p: Vec<u16> = OsStr::new(PIPE_NAME).encode_wide().chain(Some(0)).collect();
        let n = (timeout.as_millis() / 200) as u32;
        for _ in 0..n {
            unsafe {
                let h = CreateFileW(
                    p.as_ptr(),
                    0xC0000000,
                    0,
                    std::ptr::null_mut(),
                    3,
                    0,
                    std::ptr::null_mut(),
                );
                if h != -1isize {
                    // Byte read mode to match server (0 = PIPE_READMODE_BYTE)
                    SetNamedPipeHandleState(h, &0u32, std::ptr::null(), std::ptr::null());
                    return Ok(Self {
                        seq: 0,
                        pipe: Pipe { handle: h },
                        recv_buf: Vec::with_capacity(4096),
                    });
                }
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
            let mut chunk = [0u8; 4096];
            match self.pipe.read(&mut chunk) {
                Ok(0) => return Err(IpcError::Disconnected),
                Ok(n) => {
                    if let Some(pos) = chunk[..n].iter().position(|&b| b == b'\n') {
                        self.recv_buf.extend_from_slice(&chunk[..pos]);
                        break;
                    }
                    self.recv_buf.extend_from_slice(&chunk[..n]);
                }
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
    fn CreateFileW(
        p: *const u16,
        a: u32,
        s: u32,
        sa: *mut u8,
        cd: u32,
        f: u32,
        t: *mut u8,
    ) -> isize;
    fn SetNamedPipeHandleState(h: isize, m: *const u32, a: *const u32, b: *const u32) -> i32;
    fn ReadFile(h: *mut u8, b: *mut u8, s: u32, r: *mut u32, o: *mut u8) -> i32;
    fn WriteFile(h: *mut u8, b: *const u8, s: u32, w: *mut u32, o: *mut u8) -> i32;
    fn CloseHandle(h: *mut u8) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ping_roundtrip() {
        let req = IpcRequest::Ping {
            seq: 1,
            timestamp: 123456789,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("Ping"));
        assert!(json.contains("123456789"));

        // Deserialize back
        let parsed: IpcRequest = serde_json::from_str(&json).unwrap();
        match parsed {
            IpcRequest::Ping { seq, timestamp } => {
                assert_eq!(seq, 1);
                assert_eq!(timestamp, 123456789);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_get_game_state_request() {
        let req = IpcRequest::GetGameState { seq: 42 };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: IpcRequest = serde_json::from_str(&json).unwrap();
        match parsed {
            IpcRequest::GetGameState { seq } => assert_eq!(seq, 42),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_perform_action_request() {
        let action = ActionCommand {
            action_type: "PlayCard".into(),
            hand_index: Some(2),
            target_id: Some(100),
            position: Some(3),
            choice: None,
        };
        let req = IpcRequest::PerformAction { seq: 7, action };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: IpcRequest = serde_json::from_str(&json).unwrap();
        match parsed {
            IpcRequest::PerformAction { seq, ref action } => {
                assert_eq!(seq, 7);
                assert_eq!(action.action_type, "PlayCard");
                assert_eq!(action.hand_index, Some(2));
                assert_eq!(action.target_id, Some(100));
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_pong_response() {
        let resp = IpcResponse::Pong {
            seq: 1,
            timestamp: 987654321,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: IpcResponse = serde_json::from_str(&json).unwrap();
        match parsed {
            IpcResponse::Pong { seq, timestamp } => {
                assert_eq!(seq, 1);
                assert_eq!(timestamp, 987654321);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_game_state_response() {
        let state = GameStateData {
            scene: "Gameplay".into(),
            is_own_turn: true,
            turn: 5,
            own_mana: 8,
            own_max_mana: 10,
            own_hero: EntityData {
                entity_id: 1,
                card_id: "HERO_01".into(),
                health: 30,
                attack: 0,
                armor: 2,
                has_taunt: false,
                has_divine_shield: false,
                has_stealth: false,
                has_poisonous: false,
                has_lifesteal: false,
                is_exhausted: false,
                num_attacks: 0,
            },
            enemy_hero: EntityData {
                entity_id: 2,
                card_id: "HERO_02".into(),
                health: 25,
                attack: 3,
                armor: 0,
                has_taunt: false,
                has_divine_shield: false,
                has_stealth: false,
                has_poisonous: false,
                has_lifesteal: false,
                is_exhausted: false,
                num_attacks: 1,
            },
            own_hand: vec![CardData {
                entity_id: 10,
                card_id: "CS2_118".into(),
                cost: 4,
                attack: 4,
                health: 5,
                card_type: "Minion".into(),
            }],
            own_minions: vec![],
            enemy_minions: vec![EntityData {
                entity_id: 20,
                card_id: "CS2_182".into(),
                health: 3,
                attack: 3,
                armor: 0,
                has_taunt: true,
                has_divine_shield: false,
                has_stealth: false,
                has_poisonous: false,
                has_lifesteal: false,
                is_exhausted: true,
                num_attacks: 0,
            }],
            own_hand_count: 1,
            enemy_hand_count: 3,
            own_deck_count: 20,
            enemy_deck_count: 18,
        };

        let resp = IpcResponse::GameState {
            seq: 1,
            state: state,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: IpcResponse = serde_json::from_str(&json).unwrap();

        match parsed {
            IpcResponse::GameState { seq, state } => {
                assert_eq!(seq, 1);
                assert_eq!(state.scene, "Gameplay");
                assert!(state.is_own_turn);
                assert_eq!(state.turn, 5);
                assert_eq!(state.own_mana, 8);
                assert_eq!(state.own_hero.entity_id, 1);
                assert_eq!(state.enemy_hero.health, 25);
                assert_eq!(state.enemy_minions.len(), 1);
                assert!(state.enemy_minions[0].has_taunt);
                assert_eq!(state.own_hand.len(), 1);
                assert_eq!(state.own_hand[0].card_id, "CS2_118");
                assert_eq!(state.enemy_hand_count, 3);
                assert_eq!(state.own_deck_count, 20);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_error_response() {
        let resp = IpcResponse::Error {
            seq: 5,
            message: "Not connected".into(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: IpcResponse = serde_json::from_str(&json).unwrap();
        match parsed {
            IpcResponse::Error { seq, message } => {
                assert_eq!(seq, 5);
                assert_eq!(message, "Not connected");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_action_result_response() {
        let resp = IpcResponse::ActionResult {
            seq: 3,
            success: true,
            error: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: IpcResponse = serde_json::from_str(&json).unwrap();
        match parsed {
            IpcResponse::ActionResult {
                seq,
                success,
                error,
            } => {
                assert_eq!(seq, 3);
                assert!(success);
                assert!(error.is_none());
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_json_tag_discrimination() {
        // Test that JSON "type" field correctly discriminates requests
        let ping_json = r#"{"type":"Ping","seq":1,"timestamp":0}"#;
        let req: IpcRequest = serde_json::from_str(ping_json).unwrap();
        assert!(matches!(req, IpcRequest::Ping { .. }));

        let gs_json = r#"{"type":"GetGameState","seq":2}"#;
        let req: IpcRequest = serde_json::from_str(gs_json).unwrap();
        assert!(matches!(req, IpcRequest::GetGameState { .. }));
    }

    #[test]
    fn test_entity_data_defaults() {
        let e = EntityData {
            entity_id: 0,
            card_id: String::new(),
            health: 0,
            attack: 0,
            armor: 0,
            has_taunt: false,
            has_divine_shield: false,
            has_stealth: false,
            has_poisonous: false,
            has_lifesteal: false,
            is_exhausted: false,
            num_attacks: 0,
        };
        let json = serde_json::to_string(&e).unwrap();
        let e2: EntityData = serde_json::from_str(&json).unwrap();
        assert_eq!(e.entity_id, e2.entity_id);
        assert!(!e2.has_taunt);
    }
}
