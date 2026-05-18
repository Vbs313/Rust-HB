//! 键盘操作封装
//!
//! 基于 Windows SendInput API 实现键盘模拟（raw FFI）

use crate::ffi::{SendInput, INPUT, INPUT_UNION, KEYBDINPUT};
use crate::InputError;

const INPUT_KEYBOARD: u32 = 1;
const KEYEVENTF_EXTENDEDKEY: u32 = 0x0001;
const KEYEVENTF_KEYUP: u32 = 0x0002;
const KEYEVENTF_SCANCODE: u32 = 0x0008;

// ============================================================
// 按键定义
// ============================================================

/// 键盘按键
#[derive(Debug, Clone, Copy)]
pub enum Key {
    Return,
    Escape,
    Space,
    Tab,
    Backspace,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    Add,
    Subtract,
    Multiply,
    Divide,
    Up,
    Down,
    Left,
    Right,
    Shift,
    Control,
    Alt,
    Unknown(u16),
}

// ============================================================
// 键盘操作
// ============================================================

/// 按下按键
pub fn press_key(key: Key) -> Result<(), InputError> {
    send_key(&key, false)
}

/// 释放按键
pub fn release_key(key: Key) -> Result<(), InputError> {
    send_key(&key, true)
}

/// 敲击按键（按下+释放）
pub fn tap_key(key: Key) -> Result<(), InputError> {
    press_key(key)?;
    std::thread::sleep(std::time::Duration::from_millis(
        20 + rand::random::<u64>() % 30,
    ));
    release_key(key)
}

// ============================================================
// 内部实现
// ============================================================

/// 发送键盘事件
fn send_key(key: &Key, key_up: bool) -> Result<(), InputError> {
    let scan_code = key_to_scan_code(key);
    let mut flags = KEYEVENTF_SCANCODE;
    if key_up {
        flags |= KEYEVENTF_KEYUP;
    }

    let ki = KEYBDINPUT {
        wVk: 0,
        wScan: scan_code,
        dwFlags: flags,
        time: 0,
        dwExtraInfo: 0,
    };

    let input = INPUT {
        type_: INPUT_KEYBOARD,
        u: INPUT_UNION {
            ki: std::mem::ManuallyDrop::new(ki),
        },
    };

    unsafe {
        let sent = SendInput(1, &input, std::mem::size_of::<INPUT>() as i32);
        if sent == 0 {
            return Err(InputError::SimFailed("SendInput keyboard failed".into()));
        }
    }

    Ok(())
}

/// 按键 → 扫描码
fn key_to_scan_code(key: &Key) -> u16 {
    match key {
        Key::Return => 0x1C,
        Key::Escape => 0x01,
        Key::Space => 0x39,
        Key::Tab => 0x0F,
        Key::Backspace => 0x0E,
        Key::F1 => 0x3B,
        Key::F2 => 0x3C,
        Key::F3 => 0x3D,
        Key::F4 => 0x3E,
        Key::F5 => 0x3F,
        Key::F6 => 0x40,
        Key::F7 => 0x41,
        Key::F8 => 0x42,
        Key::F9 => 0x43,
        Key::F10 => 0x44,
        Key::F11 => 0x57,
        Key::F12 => 0x58,
        Key::Num1 => 0x02,
        Key::Num2 => 0x03,
        Key::Num3 => 0x04,
        Key::Num4 => 0x05,
        Key::Num5 => 0x06,
        Key::Num6 => 0x07,
        Key::Num7 => 0x08,
        Key::Num8 => 0x09,
        Key::Num9 => 0x0A,
        Key::Num0 => 0x0B,
        Key::A => 0x1E,
        Key::B => 0x30,
        Key::C => 0x2E,
        Key::D => 0x20,
        Key::E => 0x12,
        Key::F => 0x21,
        Key::G => 0x22,
        Key::H => 0x23,
        Key::I => 0x17,
        Key::J => 0x24,
        Key::K => 0x25,
        Key::L => 0x26,
        Key::M => 0x32,
        Key::N => 0x31,
        Key::O => 0x18,
        Key::P => 0x19,
        Key::Q => 0x10,
        Key::R => 0x13,
        Key::S => 0x1F,
        Key::T => 0x14,
        Key::U => 0x16,
        Key::V => 0x2F,
        Key::W => 0x11,
        Key::X => 0x2D,
        Key::Y => 0x15,
        Key::Z => 0x2C,
        Key::Numpad0 => 0x52,
        Key::Numpad1 => 0x4F,
        Key::Numpad2 => 0x50,
        Key::Numpad3 => 0x51,
        Key::Numpad4 => 0x4B,
        Key::Numpad5 => 0x4C,
        Key::Numpad6 => 0x4D,
        Key::Numpad7 => 0x47,
        Key::Numpad8 => 0x48,
        Key::Numpad9 => 0x49,
        Key::Add => 0x4E,
        Key::Subtract => 0x4A,
        Key::Multiply => 0x37,
        Key::Divide => 0x35,
        Key::Up => 0x48,
        Key::Down => 0x50,
        Key::Left => 0x4B,
        Key::Right => 0x4D,
        Key::Shift => 0x2A,
        Key::Control => 0x1D,
        Key::Alt => 0x38,
        Key::Unknown(sc) => *sc,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_scan_codes() {
        assert_eq!(key_to_scan_code(&Key::Return), 0x1C);
        assert_eq!(key_to_scan_code(&Key::Escape), 0x01);
        assert_eq!(key_to_scan_code(&Key::Tab), 0x0F);
        assert_eq!(key_to_scan_code(&Key::Space), 0x39);
        assert_eq!(key_to_scan_code(&Key::Backspace), 0x0E);
    }

    #[test]
    fn test_letter_scan_codes() {
        assert_eq!(key_to_scan_code(&Key::A), 0x1E);
        assert_eq!(key_to_scan_code(&Key::B), 0x30);
        assert_eq!(key_to_scan_code(&Key::Z), 0x2C);
        assert_eq!(key_to_scan_code(&Key::W), 0x11);
        assert_eq!(key_to_scan_code(&Key::S), 0x1F);
        assert_eq!(key_to_scan_code(&Key::D), 0x20);
    }

    #[test]
    fn test_number_scan_codes() {
        assert_eq!(key_to_scan_code(&Key::Num0), 0x0B);
        assert_eq!(key_to_scan_code(&Key::Num1), 0x02);
        assert_eq!(key_to_scan_code(&Key::Num9), 0x0A);
    }

    #[test]
    fn test_function_key_codes() {
        assert_eq!(key_to_scan_code(&Key::F1), 0x3B);
        assert_eq!(key_to_scan_code(&Key::F5), 0x3F);
        assert_eq!(key_to_scan_code(&Key::F12), 0x58);
    }

    #[test]
    fn test_numpad_codes() {
        assert_eq!(key_to_scan_code(&Key::Numpad0), 0x52);
        assert_eq!(key_to_scan_code(&Key::Numpad5), 0x4C);
        assert_eq!(key_to_scan_code(&Key::Add), 0x4E);
        assert_eq!(key_to_scan_code(&Key::Divide), 0x35);
    }

    #[test]
    fn test_arrow_keys() {
        assert_eq!(key_to_scan_code(&Key::Up), 0x48);
        assert_eq!(key_to_scan_code(&Key::Down), 0x50);
        assert_eq!(key_to_scan_code(&Key::Left), 0x4B);
        assert_eq!(key_to_scan_code(&Key::Right), 0x4D);
    }

    #[test]
    fn test_modifier_keys() {
        assert_eq!(key_to_scan_code(&Key::Shift), 0x2A);
        assert_eq!(key_to_scan_code(&Key::Control), 0x1D);
        assert_eq!(key_to_scan_code(&Key::Alt), 0x38);
    }

    #[test]
    fn test_unknown_key() {
        assert_eq!(key_to_scan_code(&Key::Unknown(0xFF)), 0xFF);
        assert_eq!(key_to_scan_code(&Key::Unknown(0xAB)), 0xAB);
    }

    #[test]
    fn test_all_keys_have_unique_codes() {
        // Quick sanity: scan codes should be mostly unique
        let keys = [
            Key::A,
            Key::B,
            Key::C,
            Key::D,
            Key::E,
            Key::F,
            Key::G,
            Key::H,
            Key::I,
            Key::J,
            Key::K,
            Key::L,
            Key::M,
            Key::N,
            Key::O,
            Key::P,
            Key::Q,
            Key::R,
            Key::S,
            Key::T,
            Key::U,
            Key::V,
            Key::W,
            Key::X,
            Key::Y,
            Key::Z,
            Key::Num0,
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
            Key::Num8,
            Key::Num9,
        ];
        let mut codes = std::collections::HashSet::new();
        for k in &keys {
            let sc = key_to_scan_code(k);
            assert!(codes.insert(sc), "Duplicate scan code 0x{sc:02X}");
        }
    }
}
