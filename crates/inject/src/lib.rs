//! hs-ipc-server v6 — Professional Mono embedding IPC server
//! Uses proper Mono API: mono_assembly_foreach + mono_assembly_get_image
#![allow(non_snake_case, dead_code)]

type HANDLE = isize; type BOOL = i32; type DWORD = u32; type LPVOID = isize; type LPCSTR = isize;
type HMODULE = isize; type LPDWORD = *mut u32;

const TRUE: BOOL = 1; const FALSE: BOOL = 0;
const PIPE_ACCESS_DUPLEX: DWORD = 3; const PIPE_WAIT: DWORD = 0;
const PIPE_UNLIMITED_INSTANCES: DWORD = 255; const INVALID_HANDLE: isize = -1;
const ERROR_PIPE_CONNECTED: DWORD = 535; const DLL_PROCESS_ATTACH: DWORD = 1;

// ===== Mono type aliases =====
type MonoDomain = isize; type MonoImage = isize; type MonoClass = isize; type MonoAssembly = isize;
type MonoMethod = isize; type MonoObject = isize;

extern "system" {
    fn CreateNamedPipeA(n: LPCSTR, m: DWORD, pm: DWORD, mi: DWORD, ob: DWORD, ib: DWORD, t: DWORD, s: *mut u8) -> HANDLE;
    fn ConnectNamedPipe(p: HANDLE, o: *mut u8) -> BOOL; fn DisconnectNamedPipe(p: HANDLE) -> BOOL;
    fn ReadFile(h: HANDLE, b: *mut u8, s: DWORD, r: LPDWORD, o: *mut u8) -> BOOL;
    fn WriteFile(h: HANDLE, b: *const u8, s: DWORD, w: LPDWORD, o: *mut u8) -> BOOL;
    fn CloseHandle(h: HANDLE) -> BOOL;
    fn CreateThread(s: *mut u8, st: DWORD, start: Option<unsafe extern "system" fn(LPVOID) -> DWORD>, p: LPVOID, f: DWORD, tid: LPDWORD) -> HANDLE;
    fn GetLastError() -> DWORD; fn Sleep(ms: DWORD);
    fn GetModuleHandleA(n: LPCSTR) -> HMODULE; fn GetProcAddress(m: HMODULE, n: LPCSTR) -> isize;
}

// Module snapshot
#[repr(C)] struct ME32W { dwSize: u32, id: u32, pid: u32, cnt: u32, proccnt: u32, base: LPVOID, size: u32, hMod: HANDLE, name: [u16; 256], path: [u16; 260] }
extern "system" { fn GetCurrentProcessId() -> DWORD; fn CreateToolhelp32Snapshot(f: DWORD, p: DWORD) -> HANDLE; fn Module32FirstW(s: HANDLE, e: *mut ME32W) -> BOOL; fn Module32NextW(s: HANDLE, e: *mut ME32W) -> BOOL; }

// ===== Global Mono API pointers =====
static mut FN_ROOT: isize = 0;      // mono_get_root_domain
static mut FN_CORLIB: isize = 0;    // mono_get_corlib
static mut FN_THREAD_ATTACH: isize = 0; // mono_thread_attach
static mut FN_CFN: isize = 0;       // mono_class_from_name
static mut FN_ASM_FOREACH: isize = 0; // mono_assembly_foreach
static mut FN_ASM_IMAGE: isize = 0;   // mono_assembly_get_image
static mut FN_IMG_NAME: isize = 0;    // mono_image_get_name
static mut FN_CLASS_NAME: isize = 0;  // mono_class_get_name
static mut FN_GET_METHOD: isize = 0;  // mono_class_get_method_from_name
static mut FN_INVOKE: isize = 0;      // mono_runtime_invoke
static mut MONO_READY: bool = false;
static mut ASM_CSHARP: MonoImage = 0;
static mut SCENE_MGR_CLASS: MonoClass = 0;

// ===== DllMain =====
#[no_mangle]
pub extern "system" fn DllMain(_h: HANDLE, reason: DWORD, _: LPVOID) -> BOOL {
    if reason == DLL_PROCESS_ATTACH {
        unsafe { CreateThread(std::ptr::null_mut(), 0, Some(ipc_thread), 0, 0, std::ptr::null_mut()); }
    }
    TRUE
}

// ===== Mono initialization =====
unsafe fn init_mono() {
    if MONO_READY { return; }
    
    // Find mono.dll
    let mono_base = find_mono_module();
    if mono_base == 0 { return; }
    
    let get = |name: &str| -> isize {
        let s = std::ffi::CString::new(name).unwrap();
        GetProcAddress(mono_base, s.as_ptr() as isize)
    };
    
    FN_ROOT = get("mono_get_root_domain");
    FN_CORLIB = get("mono_get_corlib");
    FN_CFN = get("mono_class_from_name");
    FN_ASM_FOREACH = get("mono_assembly_foreach");
    FN_ASM_IMAGE = get("mono_assembly_get_image");
    FN_IMG_NAME = get("mono_image_get_name");
    FN_CLASS_NAME = get("mono_class_get_name");
    FN_GET_METHOD = get("mono_class_get_method_from_name");
    FN_INVOKE = get("mono_runtime_invoke");
    
    FN_THREAD_ATTACH = get("mono_thread_attach");
    if FN_ROOT != 0 && FN_CORLIB != 0 { MONO_READY = true; }
}

unsafe fn find_mono_module() -> HMODULE {
    for &name in &[c"mono.dll", c"mono-2.0-sgen.dll", c"monosgen-2.0.dll"] {
        let h = GetModuleHandleA(name.as_ptr() as isize);
        if h != 0 && GetProcAddress(h, c"mono_get_root_domain".as_ptr() as isize) != 0 { return h; }
    }
    // Module snapshot fallback
    let snap = CreateToolhelp32Snapshot(0x00000008, GetCurrentProcessId());
    if snap != INVALID_HANDLE {
        let mut me: ME32W = std::mem::zeroed();
        me.dwSize = std::mem::size_of::<ME32W>() as u32;
        if Module32FirstW(snap, &mut me) == TRUE { loop {
            let nm = String::from_utf16_lossy(&me.name).trim_matches('\0').to_lowercase();
            if nm.contains("mono") && GetProcAddress(me.base, c"mono_get_root_domain".as_ptr() as isize) != 0 {
                CloseHandle(snap); return me.base;
            }
            if Module32NextW(snap, &mut me) == FALSE { break; }
        } }
        CloseHandle(snap);
    }
    0
}

// ===== Assembly callback (extern "C" for mono_assembly_foreach) =====
static mut CALLBACK_RESULT: MonoImage = 0;

unsafe extern "C" fn on_assembly(asm_ptr: MonoAssembly, _user: *mut u8) {
    if CALLBACK_RESULT != 0 { return; } // already found
    if FN_ASM_IMAGE == 0 || FN_IMG_NAME == 0 { return; }
    
    let get_img: extern "C" fn(MonoAssembly) -> MonoImage = std::mem::transmute(FN_ASM_IMAGE);
    let get_name: extern "C" fn(MonoImage) -> *const i8 = std::mem::transmute(FN_IMG_NAME);
    
    let img = get_img(asm_ptr);
    if img == 0 { return; }
    let name_ptr = get_name(img);
    if name_ptr.is_null() { return; }
    let name = std::ffi::CStr::from_ptr(name_ptr).to_string_lossy().to_string();
    if name == "Assembly-CSharp" { CALLBACK_RESULT = img; }
}

/// Enumerate assemblies via mono_assembly_foreach to find Assembly-CSharp
unsafe fn find_asm_csharp() -> MonoImage {
    if CALLBACK_RESULT != 0 { return CALLBACK_RESULT; }
    if FN_ASM_FOREACH == 0 { return 0; }
    
    let foreach_fn: extern "C" fn(Option<unsafe extern "C" fn(MonoAssembly, *mut u8)>, *mut u8) = std::mem::transmute(FN_ASM_FOREACH);
    foreach_fn(Some(on_assembly), std::ptr::null_mut());
    
    CALLBACK_RESULT
}

/// Find a class by name from Assembly-CSharp
unsafe fn find_class(name: &str) -> MonoClass {
    if FN_CFN == 0 { return 0; }
    let cfn: extern "C" fn(MonoImage, *const i8, *const i8) -> MonoClass = std::mem::transmute(FN_CFN);
    let asm = if ASM_CSHARP != 0 { ASM_CSHARP } else { find_asm_csharp() };
    if asm == 0 { return 0; }
    ASM_CSHARP = asm;
    let cname = std::ffi::CString::new(name).unwrap();
    cfn(asm, c"".as_ptr() as *const i8, cname.as_ptr() as *const i8)
}

/// Call a static method returning int/enum (using -1 for any param count)
unsafe fn call_static_int(klass: MonoClass, method_name: &str) -> Option<i32> {
    if FN_GET_METHOD == 0 || FN_INVOKE == 0 { return None; }
    let get_m: extern "C" fn(MonoClass, *const i8, i32) -> MonoMethod = std::mem::transmute(FN_GET_METHOD);
    let invoke: extern "C" fn(MonoMethod, MonoObject, *mut MonoObject, *mut MonoObject) -> MonoObject = std::mem::transmute(FN_INVOKE);
    let mname = std::ffi::CString::new(method_name).unwrap();
    let method = get_m(klass, mname.as_ptr() as *const i8, -1);
    if method == 0 { return None; }
    let result = invoke(method, 0, std::ptr::null_mut(), std::ptr::null_mut());
    if result == 0 { return None; }
    Some(*((result + 4) as *const i32))
}

/// Call an instance method returning int/enum
unsafe fn call_instance_int(klass: MonoClass, instance: MonoObject, method_name: &str) -> Option<i32> {
    if FN_GET_METHOD == 0 || FN_INVOKE == 0 { return None; }
    let get_m: extern "C" fn(MonoClass, *const i8, i32) -> MonoMethod = std::mem::transmute(FN_GET_METHOD);
    let invoke: extern "C" fn(MonoMethod, MonoObject, *mut MonoObject, *mut MonoObject) -> MonoObject = std::mem::transmute(FN_INVOKE);
    let mname = std::ffi::CString::new(method_name).unwrap();
    let method = get_m(klass, mname.as_ptr() as *const i8, -1);
    if method == 0 { return None; }
    let result = invoke(method, instance, std::ptr::null_mut(), std::ptr::null_mut());
    if result == 0 { return None; }
    Some(*((result + 4) as *const i32))
}

/// Call a static method returning object reference
unsafe fn call_static_obj(klass: MonoClass, method_name: &str) -> MonoObject {
    if FN_GET_METHOD == 0 || FN_INVOKE == 0 { return 0; }
    let get_m: extern "C" fn(MonoClass, *const i8, i32) -> MonoMethod = std::mem::transmute(FN_GET_METHOD);
    let invoke: extern "C" fn(MonoMethod, MonoObject, *mut MonoObject, *mut MonoObject) -> MonoObject = std::mem::transmute(FN_INVOKE);
    let mname = std::ffi::CString::new(method_name).unwrap();
    let method = get_m(klass, mname.as_ptr() as *const i8, 0);
    if method == 0 { return 0; }
    invoke(method, 0, std::ptr::null_mut(), std::ptr::null_mut())
}

unsafe fn find_scene_mgr() -> MonoClass {
    if SCENE_MGR_CLASS != 0 { return SCENE_MGR_CLASS; }
    let cls = find_class("SceneMgr");
    if cls != 0 { SCENE_MGR_CLASS = cls; }
    cls
}

// ===== IPC Server =====
unsafe extern "system" fn ipc_thread(_: LPVOID) -> DWORD {
    Sleep(3000);
    init_mono();
    // Attach current thread to Mono domain
    if FN_THREAD_ATTACH != 0 && FN_ROOT != 0 {
        let root_fn: extern "C" fn() -> MonoDomain = std::mem::transmute(FN_ROOT);
        let attach: extern "C" fn(MonoDomain) -> *mut u8 = std::mem::transmute(FN_THREAD_ATTACH);
        let domain = root_fn();
        attach(domain);
    }
    Sleep(2000);
    if MONO_READY { find_scene_mgr(); } // pre-cache on startup
    
    let pipe_name = c"\\\\.\\pipe\\Hearthbuddy_IPC".as_ptr() as isize;
    loop {
        let pipe = CreateNamedPipeA(pipe_name, PIPE_ACCESS_DUPLEX, PIPE_WAIT, PIPE_UNLIMITED_INSTANCES, 4096, 4096, 0, std::ptr::null_mut());
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
    }
    0
}

fn handle(j: &str) -> String {
    let t = ext_str(j, "type").unwrap_or_default();
    let s = ext_u32(j, "seq").unwrap_or(0);
    match t.as_str() {
        "Ping" => { let ts = ext_u64(j, "timestamp").unwrap_or(0);
            format!("{{\"type\":\"Pong\",\"seq\":{s},\"timestamp\":{ts}}}\n") }
        "GetGameState" => { let st = read_gs(); format!("{{\"type\":\"GameState\",\"seq\":{s},\"state\":{st}}}\n") }
        _ => format!("{{\"type\":\"Error\",\"seq\":{s},\"message\":\"unknown\"}}\n")
    }
}

fn read_gs() -> String {
    unsafe {
        if !MONO_READY { return ph(); }
        let sm = if SCENE_MGR_CLASS != 0 { SCENE_MGR_CLASS } else { find_scene_mgr() };
        if sm == 0 { return r#"{"scene":"NoSM"}"#.into(); }
        
        let instance = call_static_obj(sm, "Get");
        if instance == 0 { return r#"{"scene":"NoGet"}"#.into(); }
        
        let scene = match call_instance_int(sm, instance, "GetMode") {
            Some(0) => "INVALID", Some(1) => "STARTUP", Some(2) => "LOGIN", Some(3) => "HUB",
            Some(4) => "GAMEPLAY", Some(5) => "COLLECTION", Some(6) => "ADVENTURE",
            Some(7) => "TAVERN_BRAWL", Some(8) => "ARENA", Some(9) => "DRAFT",
            Some(10) => "PACK_OPENING", Some(11) => "TOURNAMENT", Some(12) => "FRIENDLY",
            Some(13) => "FATAL_ERROR", Some(14) => "GAME_MODE", Some(15) => "BACON",
            Some(_) => "OTHER", None => "NoMode"
        };
        
        let mut turn = 0i32; let mut is_own = false;
        let gs_class = find_class("GameState");
        if gs_class != 0 {
            let gs = call_static_obj(gs_class, "Get");
            if gs != 0 {
                if let Some(t) = call_instance_int(gs_class, gs, "GetTurn") { turn = t; }
                if let Some(f) = call_instance_int(gs_class, gs, "IsFriendlySidePlayerTurn") { is_own = f > 0; }
            }
        }
        
        format!(r#"{{"scene":"{scene}","is_own_turn":{is_own},"turn":{turn}}}"#)
    }
}

fn ph() -> String {
    r#"{"scene":"Hub","is_own_turn":false,"turn":1,"own_mana":3,"own_max_mana":3,"own_hero":{"entity_id":1,"card_id":"HERO_01","health":30,"attack":0,"armor":0,"has_taunt":false,"has_divine_shield":false,"has_stealth":false,"has_poisonous":false,"has_lifesteal":false,"is_exhausted":false,"num_attacks":0},"enemy_hero":{"entity_id":2,"card_id":"HERO_02","health":30,"attack":0,"armor":0,"has_taunt":false,"has_divine_shield":false,"has_stealth":false,"has_poisonous":false,"has_lifesteal":false,"is_exhausted":false,"num_attacks":0},"own_hand":[{"entity_id":10,"card_id":"CS2_118","cost":4,"attack":4,"health":5,"card_type":"Minion"}],"own_minions":[{"entity_id":20,"card_id":"CS2_182","health":3,"attack":3,"armor":0,"has_taunt":true,"has_divine_shield":false,"has_stealth":false,"has_poisonous":false,"has_lifesteal":false,"is_exhausted":true,"num_attacks":0}],"enemy_minions":[],"own_hand_count":1,"enemy_hand_count":3,"own_deck_count":20,"enemy_deck_count":18}"#.to_string()
}

// ===== JSON helpers =====
fn ext_str(s: &str, k: &str) -> Option<String> { let q = format!("\"{k}\":\""); s.find(&q).and_then(|st| { let vs = st+q.len(); s[vs..].find('"').map(|e| s[vs..vs+e].to_string()) }) }
fn ext_u32(s: &str, k: &str) -> Option<u32> { let q = format!("\"{k}\":"); s.find(&q).and_then(|st| { let r=&s[st+q.len()..]; let e=r.find(|c:char|!c.is_ascii_digit()).unwrap_or(r.len()); r[..e].parse().ok() }) }
fn ext_u64(s: &str, k: &str) -> Option<u64> { let q = format!("\"{k}\":"); s.find(&q).and_then(|st| { let r=&s[st+q.len()..]; let e=r.find(|c:char|!c.is_ascii_digit()).unwrap_or(r.len()); r[..e].parse().ok() }) }
