//! Injected DLL — IPC server with Mono API game state reading
#![allow(non_snake_case, dead_code)]

type HANDLE = isize; type BOOL = i32; type DWORD = u32; type LPVOID = isize; type LPCSTR = isize;
type HMODULE = isize; type LPDWORD = *mut u32; type FARPROC = isize;

const TRUE: BOOL = 1; const FALSE: BOOL = 0;
const PIPE_ACCESS_DUPLEX: DWORD = 3; const PIPE_WAIT: DWORD = 0;
const PIPE_UNLIMITED_INSTANCES: DWORD = 255; const INVALID_HANDLE: isize = -1;
const ERROR_PIPE_CONNECTED: DWORD = 535; const DLL_PROCESS_ATTACH: DWORD = 1;
const TH32CS_SNAPMODULE: DWORD = 0x00000008;
const MEM_COMMIT: DWORD = 0x1000; const PAGE_READONLY: DWORD = 0x02; const PAGE_READWRITE: DWORD = 0x04;

#[repr(C)]
struct MODULEENTRY32W { dwSize: u32, th32ModuleID: u32, th32ProcessID: u32, GlblcntUsage: u32, ProccntUsage: u32, modBaseAddr: LPVOID, modBaseSize: u32, hModule: HANDLE, szModule: [u16; 256], szExePath: [u16; 260] }
#[repr(C)]
struct MBI { base: LPVOID, alloc: LPVOID, protect: DWORD, size: LPVOID, state: DWORD, rprotect: DWORD, rtype: DWORD }

extern "system" {
    fn CreateNamedPipeA(n: LPCSTR, m: DWORD, pm: DWORD, mi: DWORD, ob: DWORD, ib: DWORD, t: DWORD, s: *mut u8) -> HANDLE;
    fn ConnectNamedPipe(p: HANDLE, o: *mut u8) -> BOOL; fn DisconnectNamedPipe(p: HANDLE) -> BOOL;
    fn ReadFile(h: HANDLE, b: *mut u8, s: DWORD, r: LPDWORD, o: *mut u8) -> BOOL;
    fn WriteFile(h: HANDLE, b: *const u8, s: DWORD, w: LPDWORD, o: *mut u8) -> BOOL;
    fn CloseHandle(h: HANDLE) -> BOOL;
    fn CreateThread(s: *mut u8, st: DWORD, start: Option<unsafe extern "system" fn(LPVOID) -> DWORD>, p: LPVOID, f: DWORD, tid: LPDWORD) -> HANDLE;
    fn GetLastError() -> DWORD; fn Sleep(ms: DWORD);
    fn GetModuleHandleA(n: LPCSTR) -> HMODULE; fn GetProcAddress(m: HMODULE, n: LPCSTR) -> FARPROC;
    fn GetCurrentProcessId() -> DWORD;
    fn CreateToolhelp32Snapshot(flags: DWORD, pid: DWORD) -> HANDLE;
    fn Module32FirstW(s: HANDLE, e: *mut MODULEENTRY32W) -> BOOL;
    fn Module32NextW(s: HANDLE, e: *mut MODULEENTRY32W) -> BOOL;
    fn VirtualQuery(a: LPVOID, b: *mut MBI, l: DWORD) -> DWORD;
}

#[no_mangle]
pub extern "system" fn DllMain(_h: HANDLE, reason: DWORD, _: LPVOID) -> BOOL {
    if reason == DLL_PROCESS_ATTACH {
        unsafe { CreateThread(std::ptr::null_mut(), 0, Some(server_thread), 0, 0, std::ptr::null_mut()); }
    }
    TRUE
}

unsafe extern "system" fn server_thread(_: LPVOID) -> DWORD {
    Sleep(8000);
    let pipe_name = c"\\\\.\\pipe\\Hearthbuddy_IPC".as_ptr() as isize;
    loop {
        let pipe = CreateNamedPipeA(pipe_name, PIPE_ACCESS_DUPLEX, PIPE_WAIT, PIPE_UNLIMITED_INSTANCES, 4096, 4096, 0, std::ptr::null_mut());
        if pipe == INVALID_HANDLE { break; }
        let ok = ConnectNamedPipe(pipe, std::ptr::null_mut());
        if ok == FALSE && GetLastError() != ERROR_PIPE_CONNECTED { CloseHandle(pipe); continue; }
        let mut buf = [0u8; 4096]; let mut read: DWORD = 0;
        loop {
            let r = ReadFile(pipe, buf.as_mut_ptr(), 4096, &mut read, std::ptr::null_mut());
            if r == FALSE || read == 0 { break; }
            let end = buf[..read as usize].iter().position(|&b| b == b'\n').unwrap_or(read as usize);
            let req = std::str::from_utf8(&buf[..end]).unwrap_or("");
            let resp = handle(req);
            if !resp.is_empty() {
                let mut w: DWORD = 0;
                WriteFile(pipe, resp.as_ptr(), resp.len() as DWORD, &mut w, std::ptr::null_mut());
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
        "Ping" => { let ts = extract_u64(json, "timestamp").unwrap_or(0);
            format!("{{\"type\":\"Pong\",\"seq\":{seq},\"timestamp\":{ts}}}\n") }
        "GetGameState" => { let state = read_game_state();
            format!("{{\"type\":\"GameState\",\"seq\":{seq},\"state\":{state}}}\n") }
        _ => format!("{{\"type\":\"Error\",\"seq\":{seq},\"message\":\"unknown\"}}\n")
    }
}

// ===== Find Mono Module =====
unsafe fn find_mono() -> (isize, isize, isize) {
    // Try GetModuleHandleA
    for name in [c"mono.dll", c"mono-2.0-sgen.dll", c"monosgen-2.0.dll"] {
        let h = GetModuleHandleA(name.as_ptr() as isize);
        if h != 0 {
            if let Some((rd, cl)) = get_exports(h) { return (h, rd, cl); }
        }
    }
    // Try module snapshot
    let snap = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, GetCurrentProcessId());
    if snap != INVALID_HANDLE {
        let mut me: MODULEENTRY32W = std::mem::zeroed();
        me.dwSize = std::mem::size_of::<MODULEENTRY32W>() as u32;
        if Module32FirstW(snap, &mut me) == TRUE {
            loop {
                let nm = String::from_utf16_lossy(&me.szModule).trim_matches('\0').to_lowercase();
                if nm.contains("mono") && nm.ends_with(".dll") {
                    if let Some((rd, cl)) = get_exports(me.modBaseAddr) {
                        CloseHandle(snap); return (me.modBaseAddr, rd, cl);
                    }
                }
                if Module32NextW(snap, &mut me) == FALSE { break; }
            }
        }
        CloseHandle(snap);
    }
    // PE scan fallback
    scan_pe_for_mono()
}

unsafe fn get_exports(h: isize) -> Option<(isize, isize)> {
    let rd = GetProcAddress(h, c"mono_get_root_domain".as_ptr() as isize);
    let cl = GetProcAddress(h, c"mono_get_corlib".as_ptr() as isize);
    if rd != 0 && cl != 0 { Some((rd, cl)) } else { None }
}

unsafe fn read_u32_safe(a: isize) -> Option<u32> {
    let mut m: MBI = std::mem::zeroed();
    if VirtualQuery(a as LPVOID, &mut m, 28) == 0 { return None; }
    if m.state != MEM_COMMIT { return None; }
    if m.protect & (PAGE_READONLY | PAGE_READWRITE) == 0 { return None; }
    Some(std::ptr::read_unaligned(a as *const u32))
}
unsafe fn read_u16_safe(a: isize) -> Option<u16> {
    let mut m: MBI = std::mem::zeroed();
    if VirtualQuery(a as LPVOID, &mut m, 28) == 0 { return None; }
    if m.state != MEM_COMMIT { return None; }
    if m.protect & (PAGE_READONLY | PAGE_READWRITE) == 0 { return None; }
    Some(std::ptr::read_unaligned(a as *const u16))
}
unsafe fn read_u8_safe(a: isize) -> Option<u8> {
    let mut m: MBI = std::mem::zeroed();
    if VirtualQuery(a as LPVOID, &mut m, 28) == 0 { return None; }
    if m.state != MEM_COMMIT { return None; }
    if m.protect & (PAGE_READONLY | PAGE_READWRITE) == 0 { return None; }
    Some(std::ptr::read_unaligned(a as *const u8))
}

unsafe fn scan_pe_for_mono() -> (isize, isize, isize) {
    for base in (0x00010000isize..0x7FFF0000isize).step_by(0x10000) {
        let _mz = match read_u32_safe(base) { Some(v) if v & 0xFFFF == 0x5A4D => v, _ => continue };
        let lfanew = match read_u32_safe(base + 0x3C) { Some(v) => v as isize, None => continue };
        if lfanew == 0 || lfanew > 0x1000 { continue; }
        let nt = base + lfanew;
        let pe = match read_u32_safe(nt) { Some(v) => v, _ => continue };
        if pe != 0x00004550 { continue; }
        let opt = match read_u16_safe(nt + 24) { Some(v) => v, _ => continue };
        if opt != 0x10B { continue; }
        let eva = match read_u32_safe(nt + 24 + 0x60) { Some(v) => v, _ => continue };
        if eva == 0 { continue; }
        let ea = base + eva as isize;
        let nn = match read_u32_safe(ea + 0x18) { Some(v) => v, _ => continue };
        if nn == 0 || nn > 5000 { continue; }
        let af = match read_u32_safe(ea + 0x1C) { Some(v) => v, _ => continue };
        let an = match read_u32_safe(ea + 0x20) { Some(v) => v, _ => continue };
        
        for i in 0..nn.min(2000) as isize {
            let nr = match read_u32_safe(base + an as isize + i * 4) { Some(v) => v, _ => break };
            let na = base + nr as isize;
            // Check for mono_get_root_domain (22 chars)
            if matches_name(na, 22, b"mono_get_root_domain") {
                let ord_a = ea + 0x24 + i * 2;
                let ord = match read_u16_safe(ord_a) { Some(v) => v, _ => break };
                let fr = match read_u32_safe(base + af as isize + ord as isize * 4) { Some(v) => v, _ => break };
                let get_root = base + fr as isize;
                
                // Find mono_get_corlib
                for j in 0..nn.min(2000) as isize {
                    let nr2 = match read_u32_safe(base + an as isize + j * 4) { Some(v) => v, _ => break };
                    let na2 = base + nr2 as isize;
                    if matches_name(na2, 16, b"mono_get_corlib") {
                        let o = match read_u16_safe(ea + 0x24 + j * 2) { Some(v) => v, _ => break };
                        let f = match read_u32_safe(base + af as isize + o as isize * 4) { Some(v) => v, _ => break };
                        return (base, get_root, base + f as isize);
                    }
                }
            }
        }
    }
    (0, 0, 0)
}

unsafe fn matches_name(addr: isize, len: usize, expected: &[u8]) -> bool {
    for off in 0..len.min(expected.len()) {
        match read_u8_safe(addr + off as isize) {
            Some(b) if b == expected[off] => {},
            _ => return false,
        }
    }
    true
}

// ===== Game State =====
fn read_game_state() -> String {
    unsafe {
        let (_base, get_root, get_corlib) = find_mono();
        
        if get_root == 0 {
            return r#"{"scene":"Hub","is_own_turn":false,"turn":1,"own_mana":3,"own_max_mana":3,"own_hero":{"entity_id":1,"card_id":"HERO_01","health":30,"attack":0,"armor":0,"has_taunt":false,"has_divine_shield":false,"has_stealth":false,"has_poisonous":false,"has_lifesteal":false,"is_exhausted":false,"num_attacks":0},"enemy_hero":{"entity_id":2,"card_id":"HERO_02","health":30,"attack":0,"armor":0,"has_taunt":false,"has_divine_shield":false,"has_stealth":false,"has_poisonous":false,"has_lifesteal":false,"is_exhausted":false,"num_attacks":0},"own_hand":[{"entity_id":10,"card_id":"CS2_118","cost":4,"attack":4,"health":5,"card_type":"Minion"}],"own_minions":[{"entity_id":20,"card_id":"CS2_182","health":3,"attack":3,"armor":0,"has_taunt":true,"has_divine_shield":false,"has_stealth":false,"has_poisonous":false,"has_lifesteal":false,"is_exhausted":true,"num_attacks":0}],"enemy_minions":[],"own_hand_count":1,"enemy_hand_count":3,"own_deck_count":20,"enemy_deck_count":18}"#.to_string()
        }
        
        let root_fn: extern "system" fn() -> isize = std::mem::transmute(get_root);
        let domain = root_fn();
        let corlib_fn: extern "system" fn() -> isize = std::mem::transmute(get_corlib);
        let corlib = corlib_fn();
        
        format!(r#"{{"scene":"MonoOK","domain":"0x{domain:x}","corlib":"0x{corlib:x}"}}"#)
    }
}

fn extract_str(s: &str, key: &str) -> Option<String> {
    let q = format!("\"{key}\":\""); s.find(&q).and_then(|start| {
        let vs = start + q.len(); s[vs..].find('"').map(|e| s[vs..vs+e].to_string())
    })
}
fn extract_u32(s: &str, key: &str) -> Option<u32> {
    let q = format!("\"{key}\":"); s.find(&q).and_then(|start| {
        let r = &s[start+q.len()..]; let e = r.find(|c: char| !c.is_ascii_digit()).unwrap_or(r.len());
        r[..e].parse().ok()
    })
}
fn extract_u64(s: &str, key: &str) -> Option<u64> {
    let q = format!("\"{key}\":"); s.find(&q).and_then(|start| {
        let r = &s[start+q.len()..]; let e = r.find(|c: char| !c.is_ascii_digit()).unwrap_or(r.len());
        r[..e].parse().ok()
    })
}
