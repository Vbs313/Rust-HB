//! Win32 FFI 公共类型声明

#![allow(non_snake_case)]

// ===== Mouse =====

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MOUSEINPUT {
    pub dx: i32,
    pub dy: i32,
    pub mouseData: u32,
    pub dwFlags: u32,
    pub time: u32,
    pub dwExtraInfo: usize,
}

// ===== Keyboard =====

#[repr(C)]
#[derive(Clone, Copy)]
pub struct KEYBDINPUT {
    pub wVk: u16,
    pub wScan: u16,
    pub dwFlags: u32,
    pub time: u32,
    pub dwExtraInfo: usize,
}

// ===== INPUT Union =====

#[repr(C)]
pub union INPUT_UNION {
    pub mi: std::mem::ManuallyDrop<MOUSEINPUT>,
    pub ki: std::mem::ManuallyDrop<KEYBDINPUT>,
}

#[repr(C)]
pub struct INPUT {
    pub type_: u32,
    pub u: INPUT_UNION,
}

#[link(name = "user32")]
extern "system" {
    pub fn SendInput(cInputs: u32, pInputs: *const INPUT, cbSize: i32) -> u32;

}
