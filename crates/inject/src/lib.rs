//! Injected DLL v5e — Safe image scanner for Assembly-CSharp
#![allow(non_snake_case, dead_code)]

type HANDLE = isize; type BOOL = i32; type DWORD = u32; type LPVOID = isize; type LPCSTR = isize;
type HMODULE = isize; type LPDWORD = *mut u32; type FARPROC = isize;

const TRUE: BOOL = 1; const FALSE: BOOL = 0;
const PIPE_ACCESS_DUPLEX: DWORD = 3; const PIPE_WAIT: DWORD = 0;
const PIPE_UNLIMITED_INSTANCES: DWORD = 255; const INVALID_HANDLE: isize = -1;
const ERROR_PIPE_CONNECTED: DWORD = 535; const DLL_PROCESS_ATTACH: DWORD = 1;
const MEM_COMMIT: DWORD = 0x1000; const PAGE_READONLY: DWORD = 0x02; const PAGE_READWRITE: DWORD = 0x04;

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
    fn VirtualQuery(a: LPVOID, b: *mut MBI, l: DWORD) -> DWORD;
}

#[repr(C)] struct ME32W { dwSize: u32, id: u32, pid: u32, cnt: u32, proccnt: u32, base: LPVOID, size: u32, hMod: HANDLE, name: [u16; 256], path: [u16; 260] }
extern "system" { fn GetCurrentProcessId() -> DWORD; fn CreateToolhelp32Snapshot(f: DWORD, p: DWORD) -> HANDLE; fn Module32FirstW(s: HANDLE, e: *mut ME32W) -> BOOL; fn Module32NextW(s: HANDLE, e: *mut ME32W) -> BOOL; }

static mut G_ROOT: isize = 0; static mut G_CORLIB: isize = 0; static mut G_CFN: isize = 0;
static mut G_IMG_NAME: isize = 0; static mut G_INITED: bool = false;
static mut G_ASM_IMG: isize = 0; // cached Assembly-CSharp image

#[no_mangle]
pub extern "system" fn DllMain(_h: HANDLE, reason: DWORD, _: LPVOID) -> BOOL {
    if reason == DLL_PROCESS_ATTACH { unsafe { CreateThread(std::ptr::null_mut(), 0, Some(server_thread), 0, 0, std::ptr::null_mut()); } }
    TRUE
}

unsafe fn init() {
    if G_INITED { return; }
    G_INITED = true;
    // Find mono by name or module snapshot
    for &name in &[c"mono.dll", c"mono-2.0-sgen.dll", c"monosgen-2.0.dll"] {
        let h = GetModuleHandleA(name.as_ptr() as isize);
        if h != 0 {
            G_ROOT = GetProcAddress(h, c"mono_get_root_domain".as_ptr() as isize);
            G_CORLIB = GetProcAddress(h, c"mono_get_corlib".as_ptr() as isize);
            G_CFN = GetProcAddress(h, c"mono_class_from_name".as_ptr() as isize);
            G_IMG_NAME = GetProcAddress(h, c"mono_image_get_name".as_ptr() as isize);
            if G_ROOT != 0 { return; }
        }
    }
    // Snapshot
    let snap = CreateToolhelp32Snapshot(0x00000008, GetCurrentProcessId());
    if snap != INVALID_HANDLE {
        let mut me: ME32W = std::mem::zeroed();
        me.dwSize = std::mem::size_of::<ME32W>() as u32;
        if Module32FirstW(snap, &mut me) == TRUE { loop {
            let nm = String::from_utf16_lossy(&me.name).trim_matches('\0').to_lowercase();
            if nm.contains("mono") {
                G_ROOT = GetProcAddress(me.base, c"mono_get_root_domain".as_ptr() as isize);
                G_CORLIB = GetProcAddress(me.base, c"mono_get_corlib".as_ptr() as isize);
                G_CFN = GetProcAddress(me.base, c"mono_class_from_name".as_ptr() as isize);
                G_IMG_NAME = GetProcAddress(me.base, c"mono_image_get_name".as_ptr() as isize);
                if G_ROOT != 0 { CloseHandle(snap); return; }
            }
            if Module32NextW(snap, &mut me) == FALSE { break; }
        } }
        CloseHandle(snap);
    }
}

// ===== Safe memory reader =====
unsafe fn read_u32(a: isize) -> Option<u32> {
    let mut m: MBI = std::mem::zeroed();
    if VirtualQuery(a as LPVOID, &mut m, 28) == 0 { return None; }
    if m.state != MEM_COMMIT { return None; }
    if (m.protect & (PAGE_READONLY | PAGE_READWRITE)) == 0 { return None; }
    Some(std::ptr::read_unaligned(a as *const u32))
}

// ===== Find Assembly-CSharp by scanning committed pages near corlib =====
unsafe fn find_asm() -> isize {
    let corlib_fn: extern "C" fn() -> isize = std::mem::transmute(G_CORLIB);
    let corlib = corlib_fn();
    let name_fn: extern "C" fn(isize) -> *const i8 = std::mem::transmute(G_IMG_NAME);
    
    // Scan around corlib ±4MB, step 64 bytes
    for delta in (0..0x400000i32).step_by(64) {
        for &sign in &[1i32, -1i32] {
            let addr = corlib + (delta * sign) as isize;
            if addr <= 0x10000 || addr >= 0x7FFF0000 { continue; }
            
            let np_val = match read_u32(addr + 0x1C) { Some(v) => v as isize, None => continue };
            if np_val < 0x10000 || np_val > 0x7FFFFFFF { continue; }
            
            // First check if the NAME STRING looks valid before calling any API
            // Read first few bytes of the supposed name
            let mut name_buf = [0u8; 20];
            let mut ok = true;
            for i in 0..5 {
                match read_u32(np_val + i as isize) {
                    Some(v) => { name_buf[i] = v as u8; if v as u8 == 0 { break; } }
                    None => { ok = false; break; }
                }
            }
            if !ok { continue; }
            let preview = String::from_utf8_lossy(&name_buf[..5]);
            // Must start with alphabetic chars to be a potential image name
            if preview.chars().next().map_or(true, |c| !c.is_ascii_alphabetic()) { continue; }
            
            // Now safely verify with mono_image_get_name
            let name_ptr = name_fn(addr);
            if name_ptr.is_null() { continue; }
            let name = std::ffi::CStr::from_ptr(name_ptr).to_string_lossy().to_string();
            if name == "Assembly-CSharp" { return addr; }
        }
    }
    0
}

// ===== Server thread =====
unsafe extern "system" fn server_thread(_: LPVOID) -> DWORD {
    Sleep(3000);
    init();
    Sleep(2000);
    
    let pn = c"\\\\.\\pipe\\Hearthbuddy_IPC".as_ptr() as isize;
    loop {
        let pipe = CreateNamedPipeA(pn, PIPE_ACCESS_DUPLEX, PIPE_WAIT, PIPE_UNLIMITED_INSTANCES, 4096, 4096, 0, std::ptr::null_mut());
        if pipe == INVALID_HANDLE { break; }
        let ok = ConnectNamedPipe(pipe, std::ptr::null_mut());
        if ok == FALSE && GetLastError() != ERROR_PIPE_CONNECTED { CloseHandle(pipe); continue; }
        let mut buf = [0u8; 4096]; let mut rd: DWORD = 0;
        loop {
            let r = ReadFile(pipe, buf.as_mut_ptr(), 4096, &mut rd, std::ptr::null_mut());
            if r == FALSE || rd == 0 { break; }
            let e = buf[..rd as usize].iter().position(|&b| b == b'\n').unwrap_or(rd as usize);
            let req = std::str::from_utf8(&buf[..e]).unwrap_or("");
            let resp = handle(req);
            if !resp.is_empty() { let mut w: DWORD = 0; WriteFile(pipe, resp.as_ptr(), resp.len() as DWORD, &mut w, std::ptr::null_mut()); }
        }
        DisconnectNamedPipe(pipe);
    } 0
}

fn handle(j: &str) -> String {
    let t = ext_str(j, "type").unwrap_or_default();
    let s = ext_u32(j, "seq").unwrap_or(0);
    match t.as_str() {
        "Ping" => { let ts = ext_u64(j, "timestamp").unwrap_or(0); format!("{{\"type\":\"Pong\",\"seq\":{s},\"timestamp\":{ts}}}\n") }
        "GetGameState" => { let st = read_gs(); format!("{{\"type\":\"GameState\",\"seq\":{s},\"state\":{st}}}\n") }
        _ => format!("{{\"type\":\"Error\",\"seq\":{s},\"message\":\"unknown\"}}\n")
    }
}

fn read_gs() -> String {
    unsafe {
        if G_ROOT == 0 { return ph(); }
        let rf: extern "C" fn() -> isize = std::mem::transmute(G_ROOT);
        let domain = rf();
        
        if G_ASM_IMG == 0 { G_ASM_IMG = find_asm(); }
        if G_ASM_IMG == 0 { return format!(r#"{{"scene":"ASM?"}}"#); }
        
        if G_CFN != 0 {
            let cfn: extern "C" fn(isize, *const i8, *const i8) -> isize = std::mem::transmute(G_CFN);
            let sm = cfn(G_ASM_IMG, c"".as_ptr() as *const i8, c"SceneMgr".as_ptr() as *const i8);
            if sm != 0 { return format!(r#"{{"scene":"OK","class":"SceneMgr"}}"#); }
            return format!(r#"{{"scene":"NoSceneMgr"}}"#);
        }
        format!(r#"{{"scene":"MonoOK","domain":"0x{domain:x}"}}"#)
    }
}

fn ph() -> String { r#"{"scene":"Hub","is_own_turn":false,"turn":1,"own_mana":3,"own_max_mana":3,"own_hero":{"entity_id":1,"card_id":"HERO_01","health":30,"attack":0,"armor":0,"has_taunt":false,"has_divine_shield":false,"has_stealth":false,"has_poisonous":false,"has_lifesteal":false,"is_exhausted":false,"num_attacks":0},"enemy_hero":{"entity_id":2,"card_id":"HERO_02","health":30,"attack":0,"armor":0,"has_taunt":false,"has_divine_shield":false,"has_stealth":false,"has_poisonous":false,"has_lifesteal":false,"is_exhausted":false,"num_attacks":0},"own_hand":[{"entity_id":10,"card_id":"CS2_118","cost":4,"attack":4,"health":5,"card_type":"Minion"}],"own_minions":[{"entity_id":20,"card_id":"CS2_182","health":3,"attack":3,"armor":0,"has_taunt":true,"has_divine_shield":false,"has_stealth":false,"has_poisonous":false,"has_lifesteal":false,"is_exhausted":true,"num_attacks":0}],"enemy_minions":[],"own_hand_count":1,"enemy_hand_count":3,"own_deck_count":20,"enemy_deck_count":18}"#.to_string() }

fn ext_str(s: &str, k: &str) -> Option<String> { let q = format!("\"{k}\":\""); s.find(&q).and_then(|st| { let vs = st+q.len(); s[vs..].find('"').map(|e| s[vs..vs+e].to_string()) }) }
fn ext_u32(s: &str, k: &str) -> Option<u32> { let q = format!("\"{k}\":"); s.find(&q).and_then(|st| { let r=&s[st+q.len()..]; let e=r.find(|c:char|!c.is_ascii_digit()).unwrap_or(r.len()); r[..e].parse().ok() }) }
fn ext_u64(s: &str, k: &str) -> Option<u64> { let q = format!("\"{k}\":"); s.find(&q).and_then(|st| { let r=&s[st+q.len()..]; let e=r.find(|c:char|!c.is_ascii_digit()).unwrap_or(r.len()); r[..e].parse().ok() }) }
