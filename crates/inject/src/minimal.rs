//! Minimal IPC DLL — no Mono init, just pipe server
#![allow(non_snake_case)]
type HANDLE=isize;type BOOL=i32;type DWORD=u32;type LPVOID=isize;type LPCSTR=isize;type LPDWORD=*mut u32;
const TRUE:BOOL=1;const FALSE:BOOL=0;const PIPE_ACCESS_DUPLEX:DWORD=3;const PIPE_WAIT:DWORD=0;
const PIPE_UNLIMITED_INSTANCES:DWORD=255;const INVALID_HANDLE:isize=-1;const ERROR_PIPE_CONNECTED:DWORD=535;
const DLL_PROCESS_ATTACH:DWORD=1;
extern"system"{
fn CreateNamedPipeA(n:LPCSTR,m:DWORD,pm:DWORD,mi:DWORD,ob:DWORD,ib:DWORD,t:DWORD,s:*mut u8)->HANDLE;
fn ConnectNamedPipe(p:HANDLE,o:*mut u8)->BOOL;fn DisconnectNamedPipe(p:HANDLE)->BOOL;
fn ReadFile(h:HANDLE,b:*mut u8,s:DWORD,r:LPDWORD,o:*mut u8)->BOOL;
fn WriteFile(h:HANDLE,b:*const u8,s:DWORD,w:LPDWORD,o:*mut u8)->BOOL;fn CloseHandle(h:HANDLE)->BOOL;
fn CreateThread(s:*mut u8,st:DWORD,start:Option<unsafe extern"system"fn(LPVOID)->DWORD>,p:LPVOID,f:DWORD,tid:LPDWORD)->HANDLE;
fn GetLastError()->DWORD;fn Sleep(ms:DWORD);}

#[no_mangle]pub extern"system"fn DllMain(_h:HANDLE,reason:DWORD,_:LPVOID)->BOOL{
if reason==DLL_PROCESS_ATTACH{unsafe{CreateThread(std::ptr::null_mut(),0,Some(thread),0,0,std::ptr::null_mut());}}TRUE}

unsafe extern"system"fn thread(_:LPVOID)->DWORD{
Sleep(5000);
loop{
let pipe=CreateNamedPipeA(c"\\\\.\\pipe\\Hearthbuddy_IPC".as_ptr()as isize,PIPE_ACCESS_DUPLEX,PIPE_WAIT,PIPE_UNLIMITED_INSTANCES,4096,4096,0,std::ptr::null_mut());
if pipe==INVALID_HANDLE{break;}
let ok=ConnectNamedPipe(pipe,std::ptr::null_mut());
if ok==FALSE&&GetLastError()!=ERROR_PIPE_CONNECTED{CloseHandle(pipe);continue;}
let mut buf=[0u8;4096];let mut rd:DWORD=0;
loop{
let r=ReadFile(pipe,buf.as_mut_ptr(),4096,&mut rd,std::ptr::null_mut());
if r==FALSE||rd==0{break;}
let e=buf[..rd as usize].iter().position(|&b|b==b'\n').unwrap_or(rd as usize);
let req=std::str::from_utf8(&buf[..e]).unwrap_or("");
let resp=handle(req);
if!resp.is_empty(){let mut w:DWORD=0;WriteFile(pipe,resp.as_ptr(),resp.len()as DWORD,&mut w,std::ptr::null_mut());}}
DisconnectNamedPipe(pipe);}0}

fn handle(j:&str)->String{
let t=ext_str(j,"type").unwrap_or_default();
let s=ext_u32(j,"seq").unwrap_or(0);
match t.as_str(){
"Ping"=>{let ts=ext_u64(j,"timestamp").unwrap_or(0);format!("{{\"type\":\"Pong\",\"seq\":{s},\"timestamp\":{ts}}}\n")}
"GetGameState"=>{format!("{{\"type\":\"GameState\",\"seq\":{s},\"state\":{{\"scene\":\"Hub\",\"is_own_turn\":false,\"turn\":0,\"own_mana\":0,\"own_max_mana\":0,\"own_hero\":{{\"entity_id\":1,\"card_id\":\"HERO_01\",\"health\":30,\"attack\":0,\"armor\":0,\"has_taunt\":false,\"has_divine_shield\":false,\"has_stealth\":false,\"has_poisonous\":false,\"has_lifesteal\":false,\"is_exhausted\":false,\"num_attacks\":0}},\"enemy_hero\":{{\"entity_id\":2,\"card_id\":\"HERO_02\",\"health\":30,\"attack\":0,\"armor\":0,\"has_taunt\":false,\"has_divine_shield\":false,\"has_stealth\":false,\"has_poisonous\":false,\"has_lifesteal\":false,\"is_exhausted\":false,\"num_attacks\":0}},\"own_hand\":[],\"own_minions\":[],\"enemy_minions\":[],\"own_hand_count\":0,\"enemy_hand_count\":0,\"own_deck_count\":30,\"enemy_deck_count\":30}}}}\n")}
_=>format!("{{\"type\":\"Error\",\"seq\":{s},\"message\":\"unknown\"}}\n")}}

fn ext_str(s:&str,k:&str)->Option<String>{let q=format!("\"{k}\":\"");s.find(&q).and_then(|st|{let vs=st+q.len();s[vs..].find('"').map(|e|s[vs..vs+e].to_string())})}
fn ext_u32(s:&str,k:&str)->Option<u32>{let q=format!("\"{k}\":");s.find(&q).and_then(|st|{let r=&s[st+q.len()..];let e=r.find(|c:char|!c.is_ascii_digit()).unwrap_or(r.len());r[..e].parse().ok()})}
fn ext_u64(s:&str,k:&str)->Option<u64>{let q=format!("\"{k}\":");s.find(&q).and_then(|st|{let r=&s[st+q.len()..];let e=r.find(|c:char|!c.is_ascii_digit()).unwrap_or(r.len());r[..e].parse().ok()})}
