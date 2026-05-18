//! Minimal injected DLL — IPC pipe server inside Hearthstone
//! v3: Simplified to avoid crashes. Mono will be added later.

#![allow(non_snake_case, dead_code)]

type HANDLE = isize;
type BOOL = i32;
type DWORD = u32;
type LPVOID = isize;
type LPDWORD = *mut u32;

const TRUE: BOOL = 1;
const FALSE: BOOL = 0;
const PIPE_ACCESS_DUPLEX: DWORD = 3;
const PIPE_TYPE_MESSAGE: DWORD = 4;
const PIPE_READMODE_MESSAGE: DWORD = 2;
const PIPE_WAIT: DWORD = 0;
const PIPE_UNLIMITED_INSTANCES: DWORD = 255;
const INVALID_HANDLE: isize = -1;
const ERROR_PIPE_CONNECTED: DWORD = 535;
const DLL_PROCESS_ATTACH: DWORD = 1;

extern "system" {
    fn CreateNamedPipeA(n: LPCSTR, m: DWORD, pm: DWORD, mi: DWORD, ob: DWORD, ib: DWORD, t: DWORD, s: *mut u8) -> HANDLE;
    fn ConnectNamedPipe(p: HANDLE, o: *mut u8) -> BOOL;
    fn DisconnectNamedPipe(p: HANDLE) -> BOOL;
    fn ReadFile(h: HANDLE, b: *mut u8, s: DWORD, r: LPDWORD, o: *mut u8) -> BOOL;
    fn WriteFile(h: HANDLE, b: *const u8, s: DWORD, w: LPDWORD, o: *mut u8) -> BOOL;
    fn CloseHandle(h: HANDLE) -> BOOL;
    fn CreateThread(s: *mut u8, st: DWORD, start: Option<unsafe extern "system" fn(LPVOID) -> DWORD>, p: LPVOID, f: DWORD, tid: LPDWORD) -> HANDLE;
    fn GetLastError() -> DWORD;
    fn Sleep(ms: DWORD);
}
type LPCSTR = isize;

#[no_mangle]
pub extern "system" fn DllMain(_h: HANDLE, reason: DWORD, _: LPVOID) -> BOOL {
    if reason == DLL_PROCESS_ATTACH {
        unsafe { CreateThread(std::ptr::null_mut(), 0, Some(server_thread), 0, 0, std::ptr::null_mut()); }
    }
    TRUE
}

unsafe extern "system" fn server_thread(_: LPVOID) -> DWORD {
    Sleep(5000);
    
    let pipe_name = c"\\\\.\\pipe\\Hearthbuddy_IPC".as_ptr() as isize;
    
    loop {
        let pipe = CreateNamedPipeA(pipe_name, PIPE_ACCESS_DUPLEX,
            PIPE_WAIT, // byte mode (matching Rust IPC client)
            PIPE_UNLIMITED_INSTANCES, 4096, 4096, 0, std::ptr::null_mut());
        if pipe == INVALID_HANDLE { break; }
        
        let ok = ConnectNamedPipe(pipe, std::ptr::null_mut());
        if ok == FALSE && GetLastError() != ERROR_PIPE_CONNECTED { CloseHandle(pipe); continue; }
        
        // Multi-request loop on the same connection
        let mut buf = [0u8; 4096];
        let mut read: DWORD = 0;
        loop {
            let r = ReadFile(pipe, buf.as_mut_ptr(), 4096, &mut read, std::ptr::null_mut());
            if r == FALSE || read == 0 { break; }
            // Find newline (handle multiple requests that arrived together)
            let mut pos = 0usize;
            while pos < read as usize {
                // Find end of this line
                let end = buf[pos..read as usize].iter().position(|&b| b == b'\n')
                    .map(|p| pos + p)
                    .unwrap_or(read as usize);
                let req = std::str::from_utf8(&buf[pos..end]).unwrap_or("");
                if !req.is_empty() {
                    let resp = handle(req);
                    if !resp.is_empty() {
                        let mut w: DWORD = 0;
                        WriteFile(pipe, resp.as_ptr(), resp.len() as DWORD, &mut w, std::ptr::null_mut());
                    }
                }
                pos = end + 1; // skip past newline
                if end >= read as usize { break; }
            }
        }
        DisconnectNamedPipe(pipe);
    }
    0
}

fn handle(json: &str) -> String {
    let t = extract_str(json, "type").unwrap_or_default();
    let seq = extract_u32(json, "seq").unwrap_or(0);
    match t.as_str() {
        "Ping" => {
            let ts = extract_u64(json, "timestamp").unwrap_or(0);
            format!("{{\"type\":\"Pong\",\"seq\":{seq},\"timestamp\":{ts}}}\n")
        }
        "GetGameState" => {
            // Full response matching Rust hb-ipc protocol
            format!("{{\"type\":\"GameState\",\"seq\":{seq},\"state\":{{\"scene\":\"Hub\",\"is_own_turn\":false,\"turn\":1,\"own_mana\":3,\"own_max_mana\":3,\"own_hero\":{{\"entity_id\":1,\"card_id\":\"HERO_01\",\"health\":30,\"attack\":0,\"armor\":0,\"has_taunt\":false,\"has_divine_shield\":false,\"has_stealth\":false,\"has_poisonous\":false,\"has_lifesteal\":false,\"is_exhausted\":false,\"num_attacks\":0}},\"enemy_hero\":{{\"entity_id\":2,\"card_id\":\"HERO_02\",\"health\":30,\"attack\":0,\"armor\":0,\"has_taunt\":false,\"has_divine_shield\":false,\"has_stealth\":false,\"has_poisonous\":false,\"has_lifesteal\":false,\"is_exhausted\":false,\"num_attacks\":0}},\"own_hand\":[{{\"entity_id\":10,\"card_id\":\"CS2_118\",\"cost\":4,\"attack\":4,\"health\":5,\"card_type\":\"Minion\"}}],\"own_minions\":[{{\"entity_id\":20,\"card_id\":\"CS2_182\",\"health\":3,\"attack\":3,\"armor\":0,\"has_taunt\":true,\"has_divine_shield\":false,\"has_stealth\":false,\"has_poisonous\":false,\"has_lifesteal\":false,\"is_exhausted\":true,\"num_attacks\":0}}],\"enemy_minions\":[],\"own_hand_count\":1,\"enemy_hand_count\":3,\"own_deck_count\":20,\"enemy_deck_count\":18}}}}\n")
        }
        _ => format!("{{\"type\":\"Error\",\"seq\":{seq},\"message\":\"unknown\"}}\n")
    }
}

fn extract_str(s: &str, key: &str) -> Option<String> {
    let q = format!("\"{key}\":\"");
    s.find(&q).and_then(|start| {
        let vs = start + q.len();
        s[vs..].find('"').map(|e| s[vs..vs+e].to_string())
    })
}
fn extract_u32(s: &str, key: &str) -> Option<u32> {
    let q = format!("\"{key}\":");
    s.find(&q).and_then(|start| {
        let r = &s[start+q.len()..];
        let e = r.find(|c: char| !c.is_ascii_digit()).unwrap_or(r.len());
        r[..e].parse().ok()
    })
}
fn extract_u64(s: &str, key: &str) -> Option<u64> {
    let q = format!("\"{key}\":");
    s.find(&q).and_then(|start| {
        let r = &s[start+q.len()..];
        let e = r.find(|c: char| !c.is_ascii_digit()).unwrap_or(r.len());
        r[..e].parse().ok()
    })
}
